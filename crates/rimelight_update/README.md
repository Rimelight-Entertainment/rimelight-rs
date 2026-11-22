# rimelight_update

Self-update of installed Rimelight apps with Velopack (decision 0047). One `Updater`, fed by configuration.

## The feed

`Config { feed, minimum_version_channel }` where `feed` is one of:

- `Feed::GithubReleases { repo: "owner/name" }`: Rimelight Media (GitHub Releases).
- `Feed::Http { url }`: Rimelight Space (its website route for Velopack's files).
- `Feed::Location { location }`: a local folder or address, for testing a release.

Environment overrides (such as Space's `RIMELIGHT_UPDATE_URL`) are read by the app, which then builds the `Feed`.

`Updater::new(config)` returns `None` for a copy that is not installed (a `cargo run` build). The app treats that as
"updates off".

## Flows

| Method | Used by | What it does |
|---|---|---|
| `check_and_download(&report)` | Media | Checks and downloads. Returns `Outcome::{UpToDate, Ready, Failed}`. Then `apply_now` (restart) or `apply_on_exit` (quit). |
| `check_and_apply(&report)` | Space start-up gate | Checks, downloads, reports, applies and restarts. `Err` when the feed's minimum version is above this copy. |
| `below_minimum()` | Space | The minimum, when this copy is below it. Only with `minimum_version_channel` and an `Http` feed. |
| `watch(self: Arc<Self>, interval)` | Space | Re-checks the minimum on a thread and updates at once when below it. `DEFAULT_WATCH_INTERVAL` is 30 minutes. |

`installed_version()` returns the installed version or `None`. `init(hooks)` runs the Velopack install, update and
uninstall hooks. It must be the first call in `main`. `hooks` adds the app's own callbacks, and `|app| app` adds none.

Media's `Feed::releases_page()` gives the release notes link.

## Slint window (Space)

Nothing in the crate names a generated type. The start-up window is the macro `update_window_gate!($window, $product)`, which the
app expands in its own crate. It defines `start_up_gate(Option<Arc<Updater>>) -> bool`. The window type needs the
properties `status`, `progress` and `failed`, and the callbacks `retry` and `quit`.

## Minimum version

With `minimum_version_channel: Some("stable")` and an `Http` feed, the feed's `min-version.stable.txt` (one `x.y.z` line)
is read. A copy below it can't start, or is updated at once while running. An unparsable file or version never blocks.
