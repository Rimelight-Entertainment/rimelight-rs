//! The component gallery: a navy sidebar of stories grouped by section, a toolbar with theme, locale, viewport, text-scale
//! and reduced-motion switches, and the selected story in a frame. It is app-independent: its built-in stories are the
//! design system's own (`rimelight_ui`, `rimelight_shell`, `rimelight_design`), and its shell and palette stories use the
//! sample data in [`sample`]. An app can add stories of its own (below).
//!
//! The windows are not compiled into Rust. [`harness`] loads `ui/dev.slint` on the Slint interpreter at run time and
//! reloads it when a file under `ui/` changes (decision 0044). The built-in story registry is [`registry`].
//!
//! # App stories
//!
//! An app points the gallery at a folder with `--stories <dir>` (or passes it to [`main_with`]). The folder holds:
//!
//! - `stories.txt`: one story per non-empty line that does not start with `#`, as `section/name|Title|variant1,variant2`.
//!   The variants are optional. The section is one of `foundations`, `components`, `templates` or `shell`. The sidebar
//!   lists the app stories under "App stories", after the built-ins. [`registry::parse_app_stories`] documents the errors.
//! - `stories.slint`: the app's markup. It must export a component with this contract:
//!
//! ```text
//! export component AppStories inherits Rectangle {
//!     in property <int> story;      // the app story's index: 0 for the first line of stories.txt
//!     in property <int> variant;    // the selected variant of that story, 0 for the default
//!     in property <bool> pt;        // true when the toolbar's locale is pt
//! }
//! ```
//!
//! `GalleryWindow` imports `AppStories` from the library path `@app_stories` and shows it for every story at or past
//! `builtin-count`. The harness maps `@app_stories` to the app folder, or to the empty stub `ui/no-app-stories` when no
//! folder is given, so there is one window either way. The app's markup imports the design system as usual (`@rimelight_ui/...`,
//! `@rimelight_design/...`). Hot reload watches the app folder too.
//!
//! # Wrappers
//!
//! An app binary can own its own flags. Parse them from `std::env::args()` first, then hand the rest to [`run`] (the
//! full argument list, program name first), or to [`main_with`], which reads the process arguments itself. Pass the
//! app's own stories folder as `default_stories`, so `--stories` is not needed.
//!
//! # Flags
//!
//! All optional unless noted:
//!
//! - `--stories <dir>`: the app's story folder (see above). Without it, `default_stories` is used.
//! - `--story <id[#variant]>`: start on a story, for example `foundations/typography#weights` (default: the first).
//! - `--theme <light|dark|system>` (default system), `--locale <en|pt>` (default en),
//!   `--viewport <desktop|tablet|phone>` (default desktop), `--text-scale <100|125|150|200>` (default 100),
//!   `--reduced-motion <on|off>` (default off), `--chrome <on|off>` (default on).
//! - `--size <WxH>` window size in logical pixels (default 1280x800).
//! - `--screenshot <out.png>`: render one frame headlessly and exit (no window is shown). The frame is refused as
//!   blank when it has fewer than 8 distinct colours (exit 4).
//! - `--list-stories`: print every selectable id, one per line, and exit. App stories follow the built-ins.
//! - `--foundations`: show the foundations sheet (S1) instead of the gallery. Theme, locale, text scale and size apply.
//!
//! Exit codes: 0 ok, 2 bad arguments or a bad stories folder, 3 the window or the capture failed, 4 the captured image is
//! blank.
//!
//! Keyboard: Up and Down move through the sidebar stories; Tab moves through the toolbar chips; Space or Return
//! chooses the focused chip. The focus ring is drawn from `Tokens.focus-ring`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub mod harness;
pub mod nav;
pub mod registry;
pub mod sample;
pub mod values;

use harness::{CaptureError, Options, Viewport};
use rimelight_ui_kit::theme::{Mode, TextScale};

/// The usage line, printed after a bad argument.
pub const USAGE: &str = "usage: rimelight_gallery [--stories <dir>] [--story <id[#variant]>] [--theme light|dark|system] \
[--locale en|pt] [--viewport desktop|tablet|phone] [--text-scale 100|125|150|200] [--reduced-motion on|off] \
[--chrome on|off] [--size WxH] [--screenshot <png>] [--list-stories] [--foundations]";

/// A parsed command line. `story` is resolved once the app stories are known, so `--story` may come before `--stories`.
#[derive(Debug)]
pub struct Cli {
    /// The toolbar, size and viewport choices. `selection` and the app stories are filled in by [`run`].
    pub options: Options,
    /// The `--story` value, not yet resolved.
    pub story: Option<String>,
    /// The `--stories` folder, if given.
    pub stories: Option<PathBuf>,
    /// The `--screenshot` file, if given: one frame is captured and the program exits.
    pub screenshot: Option<PathBuf>,
    /// `--list-stories`.
    pub list: bool,
}

/// Parses the argument list, program name first. Errors name the bad flag or value.
pub fn parse(args: &[String]) -> Result<Cli, String> {
    let mut cli = Cli {
        options: Options::default(),
        story: None,
        stories: None,
        screenshot: None,
        list: false,
    };
    let mut iter = args.iter().skip(1);
    while let Some(flag) = iter.next() {
        if flag == "--list-stories" {
            cli.list = true;
            continue;
        }
        if flag == "--foundations" {
            cli.options.foundations = true;
            continue;
        }
        let value = iter
            .next()
            .ok_or_else(|| format!("{flag} needs a value"))?
            .as_str();
        match flag.as_str() {
            "--stories" => cli.stories = Some(PathBuf::from(value)),
            "--story" => cli.story = Some(value.to_string()),
            "--theme" => {
                cli.options.theme = match value {
                    "light" => Mode::Light,
                    "dark" => Mode::Dark,
                    "system" => Mode::System,
                    other => return Err(format!("unknown theme {other:?}")),
                };
            }
            "--locale" => {
                cli.options.portuguese = match value {
                    "en" => false,
                    "pt" => true,
                    other => return Err(format!("unknown locale {other:?}")),
                };
            }
            "--viewport" => {
                cli.options.viewport =
                    Viewport::parse(value).ok_or_else(|| format!("unknown viewport {value:?}"))?;
            }
            "--text-scale" => {
                let percent: u32 = value
                    .parse()
                    .map_err(|_| "text scale must be a number".to_string())?;
                cli.options.scale = TextScale::from_percent(percent)
                    .ok_or_else(|| "text scale must be 100, 125, 150 or 200".to_string())?;
            }
            "--reduced-motion" => {
                cli.options.reduced_motion = on_off(value, "reduced-motion")?;
            }
            "--chrome" => {
                cli.options.chrome = on_off(value, "chrome")?;
            }
            "--size" => {
                let (w, h) = value
                    .split_once('x')
                    .ok_or_else(|| "size must be WxH".to_string())?;
                let w: u32 = w.parse().map_err(|_| "bad width".to_string())?;
                let h: u32 = h.parse().map_err(|_| "bad height".to_string())?;
                if w < 100 || h < 100 {
                    return Err("size must be at least 100x100".to_string());
                }
                cli.options.size = (w, h);
            }
            "--screenshot" => cli.screenshot = Some(PathBuf::from(value)),
            other => return Err(format!("unknown flag {other}")),
        }
    }
    Ok(cli)
}

fn on_off(value: &str, flag: &str) -> Result<bool, String> {
    match value {
        "on" => Ok(true),
        "off" => Ok(false),
        other => Err(format!("--{flag} must be on or off, got {other:?}")),
    }
}

/// Runs the gallery with the full argument list (program name first). `default_stories` is the app's story folder when
/// `--stories` is absent. Returns the process exit code.
pub fn run(args: &[String], default_stories: Option<&Path>) -> ExitCode {
    let cli = match parse(args) {
        Ok(cli) => cli,
        Err(message) => {
            eprintln!("rimelight_gallery: {message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let mut options = cli.options;
    options.app_dir = cli
        .stories
        .or_else(|| default_stories.map(Path::to_path_buf));
    if let Some(dir) = &options.app_dir {
        match registry::load_app_stories(dir) {
            Ok(apps) => options.apps = apps,
            Err(message) => {
                eprintln!("rimelight_gallery: {message}");
                return ExitCode::from(2);
            }
        }
    }
    if let Some(id) = &cli.story {
        match registry::resolve(id, &options.apps) {
            Ok(selection) => options.selection = selection,
            Err(message) => {
                eprintln!("rimelight_gallery: --story: {message}");
                return ExitCode::from(2);
            }
        }
    }

    if cli.list {
        for line in registry::listing(&options.apps) {
            println!("{line}");
        }
        return ExitCode::SUCCESS;
    }

    let Some(out) = cli.screenshot else {
        return match harness::run(&options) {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => {
                eprintln!("rimelight_gallery: {message}");
                ExitCode::from(3)
            }
        };
    };

    if let Err(message) = harness::install_headless() {
        eprintln!("rimelight_gallery: {message}");
        return ExitCode::from(3);
    }
    match harness::capture(&options, &out) {
        Ok(line) => {
            println!("{line}");
            ExitCode::SUCCESS
        }
        Err(CaptureError::Blank(message)) => {
            eprintln!("rimelight_gallery: {message}");
            ExitCode::from(4)
        }
        Err(CaptureError::Failed(message)) => {
            eprintln!("rimelight_gallery: {message}");
            ExitCode::from(3)
        }
    }
}

/// Runs the gallery with the process arguments. See [`run`].
pub fn main_with(default_stories: Option<&Path>) -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    run(&args, default_stories)
}
