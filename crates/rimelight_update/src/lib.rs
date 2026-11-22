//! rimelight_update: self-update of installed Rimelight apps with Velopack (decision 0047), with the feed as configuration.
//!
//! One [`Updater`] serves both apps:
//!
//! - [`Feed::GithubReleases`]: a repository's GitHub Releases (Rimelight Media).
//! - [`Feed::Http`]: a Velopack feed over HTTP(S), such as the Rimelight Space website route (R2 behind a Worker).
//! - [`Feed::Location`]: a local folder or any address, for testing a release (Velopack's auto source).
//!
//! The flows the apps use:
//!
//! - [`Updater::check_and_download`] checks and downloads without restarting (Media: the top bar offers "Restart to
//!   update", or the update applies when the app quits, see [`Updater::apply_on_exit`]).
//! - [`Updater::check_and_apply`] checks, downloads, applies and restarts, and fails when the feed's minimum version
//!   (see [`Config::minimum_version_channel`]) is above this copy (Space: the start-up gate, the app does not open).
//! - [`Updater::watch`] re-checks the minimum version while the app runs and updates at once when this copy is below it
//!   (Space: every 30 minutes, [`DEFAULT_WATCH_INTERVAL`]).
//!
//! A copy that is not installed (a build from source, `cargo run`) has no Velopack manager: [`Updater::new`] returns
//! `None`, and the app treats that as "updates off". Nothing here names a Slint type. The start-up window of Rimelight
//! Space is the [`update_window_gate!`] macro, which the app expands with its own window type.
//!
//! Desktop only: on wasm and Android the crate is empty (Velopack does not build there).
#![cfg(not(any(target_arch = "wasm32", target_os = "android")))]

use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

/// The update an [`Outcome::Ready`] carries (velopack's type, re-exported so apps need no direct velopack dependency).
pub use velopack::UpdateInfo;
use velopack::download::download_url_as_string;
use velopack::sources::{AutoSource, FileSource, GithubSource, HttpSource};
use velopack::{UpdateCheck, UpdateManager, VelopackApp};

/// How often a running app re-checks the minimum version (Space's `watch`).
pub const DEFAULT_WATCH_INTERVAL: Duration = Duration::from_secs(30 * 60);

/// Where updates come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Feed {
    /// The releases of a GitHub repository, `owner/name`. Its releases page is [`Feed::releases_page`].
    GithubReleases { repo: String },
    /// A Velopack feed over HTTP(S): a folder URL with Velopack's files and, optionally, `min-version.<channel>.txt`.
    Http { url: String },
    /// A local folder or any address Velopack's auto source reads (for testing a release before it is published).
    Location { location: String },
}

impl Feed {
    /// The GitHub releases web page, for "see the release notes" links. None for the other feeds.
    pub fn releases_page(&self) -> Option<String> {
        match self {
            Feed::GithubReleases { repo } => Some(format!("https://github.com/{repo}/releases")),
            Feed::Http { .. } | Feed::Location { .. } => None,
        }
    }
}

/// The configuration of one updater. The app reads any override (an environment variable) and puts the result here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub feed: Feed,
    /// `Some(channel)`: [`Updater::check_and_apply`] and [`Updater::watch`] also read the feed's
    /// `min-version.<channel>.txt` (one version line) and block a copy below it. Only an [`Feed::Http`] feed carries
    /// that file; with another feed the check finds no minimum and never blocks. `None`: no minimum version (Media).
    pub minimum_version_channel: Option<String>,
}

/// What the update window shows, one step of a check (see [`Updater::check_and_apply`]).
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Checking,
    /// Percent, 0 to 100.
    Downloading(i16),
    Applying,
}

/// The report a check calls for each step. `Sync` so the download thread can call it too.
pub type Report<'a> = &'a (dyn Fn(Step) + Sync);

/// The result of [`Updater::check_and_download`].
pub enum Outcome {
    /// This copy is current (or the feed had nothing newer).
    UpToDate,
    /// Downloaded and waiting for a restart: apply it with [`Updater::apply_now`] or [`Updater::apply_on_exit`].
    Ready(Box<UpdateInfo>),
    /// The check or the download failed; the text is for the person.
    Failed(String),
}

/// Runs the Velopack install, update and uninstall hooks. Call it first in `main`: during a hook it does its work and
/// exits the process. `hooks` adds the app's own callbacks to the builder (Media: after install, after update, first run,
/// before uninstall); a plain app passes `|app| app`.
pub fn init(hooks: impl FnOnce(VelopackApp) -> VelopackApp) {
    hooks(VelopackApp::build()).run();
}

/// The installed version, or None when this copy is not installed.
pub fn installed_version() -> Option<String> {
    UpdateManager::new(FileSource::new("."), None, None)
        .ok()
        .map(|m| m.get_current_version_as_string())
}

/// The update manager of an installed copy, with its feed.
pub struct Updater {
    manager: UpdateManager,
    config: Config,
}

impl Updater {
    /// The updater for `config`, or None when this copy is not installed (or the feed cannot be opened).
    pub fn new(config: Config) -> Option<Self> {
        let manager = match &config.feed {
            Feed::GithubReleases { repo } => UpdateManager::new(
                GithubSource::new(&format!("https://github.com/{repo}"), None, false),
                None,
                None,
            ),
            Feed::Http { url } => UpdateManager::new(HttpSource::new(url.clone()), None, None),
            Feed::Location { location } => {
                UpdateManager::new(AutoSource::new(location), None, None)
            }
        }
        .ok()?;
        Some(Self { manager, config })
    }

    /// The configuration this updater was made with.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// The version this copy runs.
    pub fn current_version(&self) -> String {
        self.manager.get_current_version_as_string()
    }

    /// Checks and downloads without restarting (blocking; call it from a worker thread). `report` gets the percent.
    pub fn check_and_download(&self, report: Report<'_>) -> Outcome {
        match self.manager.check_for_updates() {
            Ok(UpdateCheck::UpdateAvailable(info)) => match self.download(&info, report) {
                Ok(()) => Outcome::Ready(info),
                Err(message) => Outcome::Failed(message),
            },
            Ok(_) => Outcome::UpToDate,
            Err(e) => Outcome::Failed(format!("Could not check for updates: {e}")),
        }
    }

    /// Applies a downloaded update and restarts into it. Returns only on failure.
    pub fn apply_now(&self, info: &UpdateInfo) -> Result<(), String> {
        self.manager
            .apply_updates_and_restart(&info.TargetFullRelease)
            .map_err(|e| format!("Could not install the update: {e}"))
    }

    /// Applies a downloaded update after this process exits, without restarting (the app quits; the next start is new).
    pub fn apply_on_exit(&self, info: &UpdateInfo) {
        let _ = self.manager.wait_exit_then_apply_updates(
            &info.TargetFullRelease,
            true,
            false,
            Vec::<String>::new(),
        );
    }

    /// Start-up flow (blocking): checks, and when a newer version exists downloads it, reports each step, applies it and
    /// restarts. `Ok` means this version may start. With a minimum version configured, a copy below it is an `Err` naming
    /// the minimum, when no newer version is published either.
    pub fn check_and_apply(&self, report: Report<'_>) -> Result<(), String> {
        report(Step::Checking);
        let check = self
            .manager
            .check_for_updates()
            .map_err(|e| format!("Could not check for updates: {e}"))?;
        match check {
            UpdateCheck::UpdateAvailable(update) => self.download_and_apply(&update, report),
            UpdateCheck::NoUpdateAvailable | UpdateCheck::RemoteIsEmpty => {
                match self.below_minimum() {
                    Some(minimum) => {
                        let current = self.current_version();
                        Err(format!(
                            "Version {current} is no longer supported (the minimum is {minimum}) and no newer version is published yet."
                        ))
                    }
                    None => Ok(()),
                }
            }
        }
    }

    /// The feed's minimum version when this copy is below it; None when there is no minimum, it is not reachable, is not
    /// a version, or this copy is not below it. A version that does not parse never blocks.
    pub fn below_minimum(&self) -> Option<String> {
        let channel = self.config.minimum_version_channel.as_deref()?;
        let Feed::Http { url } = &self.config.feed else {
            return None;
        };
        let minimum = fetch_minimum(url, channel)?;
        is_below_minimum(&self.current_version(), &minimum).then_some(minimum)
    }

    /// Re-checks the minimum version every `interval` on a thread and, when this copy is below it, downloads and restarts
    /// into the newest version at once. An update that is not required waits for the next start.
    pub fn watch(self: Arc<Self>, interval: Duration) {
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(interval);
                if self.below_minimum().is_none() {
                    continue;
                }
                if let Ok(UpdateCheck::UpdateAvailable(update)) = self.manager.check_for_updates()
                    && let Err(message) = self.download_and_apply(&update, &|_: Step| {})
                {
                    eprintln!("rimelight_update: forced update failed: {message}");
                }
            }
        });
    }

    /// Downloads `info`, reporting percent, and returns the message on failure.
    fn download(&self, info: &UpdateInfo, report: Report<'_>) -> Result<(), String> {
        let (tx, rx) = mpsc::channel::<i16>();
        std::thread::scope(|scope| {
            // Forwards each percent until `download_updates` drops its sender (it holds `tx` until it returns).
            scope.spawn(move || {
                for percent in rx {
                    report(Step::Downloading(percent));
                }
            });
            self.manager
                .download_updates(info, Some(tx))
                .map_err(|e| format!("Could not download the update: {e}"))
        })
    }

    /// Downloads, reports `Applying`, and restarts into `update`. Returns only on failure.
    fn download_and_apply(&self, update: &UpdateInfo, report: Report<'_>) -> Result<(), String> {
        self.download(update, report)?;
        report(Step::Applying);
        self.apply_now(update)
    }
}

/// The minimum version the feed at `feed` asks for on `channel`, or None when the file is missing or not a version.
fn fetch_minimum(feed: &str, channel: &str) -> Option<String> {
    let url = format!("{}/min-version.{channel}.txt", feed.trim_end_matches('/'));
    let text = download_url_as_string(&url, None).ok()?;
    let line = text.lines().next()?.trim().to_string();
    parse_version(&line).map(|_| line)
}

/// The three numbers of a version such as `1.2.3` (a `-pre` or `+build` suffix is ignored). `None` when it is not one.
fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    let core = text.trim().split(['-', '+']).next()?;
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

/// True when `current` is older than `minimum`. A version on either side that does not parse never blocks.
fn is_below_minimum(current: &str, minimum: &str) -> bool {
    match (parse_version(current), parse_version(minimum)) {
        (Some(current), Some(minimum)) => current < minimum,
        _ => false,
    }
}

/// Expands to the start-up update window for an app (Rimelight Space, REL.3). `$window` is the app's Slint window type
/// with the properties `status`, `progress`, `failed` and the callbacks `retry` and `quit` (see its `.slint` file).
///
/// The expansion defines `start_up_gate(updater) -> bool`: with no updater (a copy that is not installed) it returns
/// `true` at once; otherwise it shows `$window`, runs [`Updater::check_and_apply`] on a worker thread, and returns `true`
/// when the check passed, `false` when the person quit after a failure. It also defines the private helpers it needs.
///
/// Use it in the app crate, which must depend on `slint` (the expansion names `::slint`):
///
/// ```ignore
/// rimelight_update::update_window_gate!(rimelight_update_slint::UpdateWindow, "Rimelight Space");
/// ```
#[macro_export]
macro_rules! update_window_gate {
    ($window:path, $product:expr $(,)?) => {
        /// Shows one step of the check in the update window.
        fn __update_show(ui: &$window, step: &$crate::Step) {
            ui.set_failed(false);
            match step {
                $crate::Step::Checking => {
                    ui.set_status("Checking for updates".into());
                    ui.set_progress(-1.0);
                }
                $crate::Step::Downloading(percent) => {
                    ui.set_status("Downloading an update".into());
                    ui.set_progress(f32::from(*percent) / 100.0);
                }
                $crate::Step::Applying => {
                    ui.set_status(
                        format!("Installing the update, {} restarts in a moment", $product).into(),
                    );
                    ui.set_progress(-1.0);
                }
            }
        }

        /// Shows a failure in the update window, with Try again and Quit.
        fn __update_show_failure(ui: &$window, message: ::std::string::String) {
            ui.set_status(message.as_str().into());
            ui.set_failed(true);
        }

        /// One start-up check on a worker thread. On success it sets `passed` and hides the window.
        fn __update_start(
            ui: ::slint::Weak<$window>,
            updater: ::std::sync::Arc<$crate::Updater>,
            passed: ::std::sync::Arc<::std::sync::atomic::AtomicBool>,
        ) {
            ::std::thread::spawn(move || {
                let steps = ui.clone();
                let report = move |step: $crate::Step| {
                    let _ =
                        steps.upgrade_in_event_loop(move |window| __update_show(&window, &step));
                };
                match updater.check_and_apply(&report) {
                    Ok(()) => {
                        passed.store(true, ::std::sync::atomic::Ordering::SeqCst);
                        let _ = ui.upgrade_in_event_loop(|window| {
                            let _ = ::slint::ComponentHandle::hide(&window);
                        });
                    }
                    Err(message) => {
                        let _ = ui.upgrade_in_event_loop(move |window| {
                            __update_show_failure(&window, message)
                        });
                    }
                }
            });
        }

        /// Runs the start-up gate. See the macro's docs. `updater` is None for a copy that is not installed.
        pub fn start_up_gate(
            updater: ::std::option::Option<::std::sync::Arc<$crate::Updater>>,
        ) -> bool {
            use ::slint::ComponentHandle as _;
            let Some(updater) = updater else {
                return true;
            };
            let window = match <$window>::new() {
                Ok(window) => window,
                Err(e) => {
                    // No window to show the failure in. Starting unchecked would break the rule that the check passes first.
                    eprintln!("rimelight_update: the update window could not open: {e}");
                    return false;
                }
            };
            let passed = ::std::sync::Arc::new(::std::sync::atomic::AtomicBool::new(false));
            {
                let weak = window.as_weak();
                let updater = ::std::sync::Arc::clone(&updater);
                let passed = ::std::sync::Arc::clone(&passed);
                window.on_retry(move || {
                    if let Some(window) = weak.upgrade() {
                        __update_show(&window, &$crate::Step::Checking);
                    }
                    __update_start(
                        weak.clone(),
                        ::std::sync::Arc::clone(&updater),
                        ::std::sync::Arc::clone(&passed),
                    );
                });
            }
            {
                let weak = window.as_weak();
                window.on_quit(move || {
                    if let Some(window) = weak.upgrade() {
                        let _ = ::slint::ComponentHandle::hide(&window);
                    }
                });
            }
            __update_start(window.as_weak(), updater, ::std::sync::Arc::clone(&passed));
            if let Err(e) = window.run() {
                eprintln!("rimelight_update: the update window failed: {e}");
                return false;
            }
            passed.load(::std::sync::atomic::Ordering::SeqCst)
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_and_suffixed_versions() {
        assert_eq!(parse_version("1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version(" 0.10.0\n"), Some((0, 10, 0)));
        assert_eq!(parse_version("2.0.1-beta.1+abc"), Some((2, 0, 1)));
    }

    #[test]
    fn rejects_things_that_are_not_versions() {
        assert_eq!(parse_version(""), None);
        assert_eq!(parse_version("1.2"), None);
        assert_eq!(parse_version("1.2.3.4"), None);
        assert_eq!(parse_version("<html>404</html>"), None);
    }

    #[test]
    fn compares_numbers_not_text() {
        assert!(is_below_minimum("0.9.0", "0.10.0"));
        assert!(!is_below_minimum("0.10.0", "0.9.0"));
        assert!(!is_below_minimum("1.0.0", "1.0.0"));
        assert!(is_below_minimum("1.0.0", "1.0.1"));
    }

    #[test]
    fn an_unparsable_side_never_blocks() {
        assert!(!is_below_minimum("dev", "1.0.0"));
        assert!(!is_below_minimum("1.0.0", "soon"));
    }

    #[test]
    fn github_feed_has_a_releases_page_and_the_others_do_not() {
        let github = Feed::GithubReleases {
            repo: "owner/name".into(),
        };
        assert_eq!(
            github.releases_page().as_deref(),
            Some("https://github.com/owner/name/releases")
        );
        assert_eq!(
            Feed::Http {
                url: "https://example.invalid/releases/win".into()
            }
            .releases_page(),
            None
        );
    }
}
