# rimelight-rs

Shared Rust crates for Rimelight's native apps (Rimelight Space, Rimelight Media, idantity-native): Slint
components, shell, glue, media playback, picture decoding, platform calls and self-update. MIT licensed (decision D2).

The design tokens are not here. They live in the web monorepo (`Rimelight-Entertainment/rimelight`,
`packages/design`), which generates the `rimelight_design` crate that these crates depend on (decision D1).

The plan these crates follow is `docs/plan.md` in rimelight-space.

## Status

- `spikes/slint-cross-crate`: proof that a library can ship Slint markup and Rust glue while only the app compiles
  Slint (ST2.1, passed 2026-11-22).
- Workspace (ST2.0, set up 2026-11-22; not yet built or checked with cargo):
  - `crates/rimelight_ui`: Slint component library (ST2.3, WS4): 68 component files (81 exported
    components), typography, icons, and bundled fonts. Markup only; the app crates compile it. The fonts are Noto Sans
    and JetBrains Mono, licensed under SIL OFL 1.1 (see `crates/rimelight_ui/ui/fonts/README.md`).
  - `crates/rimelight_ui/tests/ui`: the one `ui` test binary for the components (ST2.7, not yet run): 26 test modules
    moved from Space, with their markup in `tests/ui/markup/`, plus the icon-file checks in `src/lib.rs`. The decision
    table is `docs/st2.7-tests.md`.
  - `crates/rimelight_ui_test_slint`: test-only crate (not published) that compiles `rimelight_ui`'s test markup, one
    module per file, for the ui test binary.
  - `apps/ui_check`: compile check. Its `ui/main.slint` places every exported component of `rimelight_ui` in one
    window, and its headless test creates that window. Not shipped.
  - `apps/gallery`: the component gallery (`rimelight_gallery`): the design system's stories on the Slint interpreter,
    app-independent, with sample data for the shell. An app adds its own stories from a folder with `--stories <dir>`.
    Not shipped.
  - `apps/`: other app crates that compile Slint come here.
  - `xtask/`: workspace task runner (`cargo xtask`); prints usage only for now.
  - `spikes/`: its own workspace, excluded from this one.
  - `vendor/`: excluded; holds the vendored `ffmpeg-sys-next` (see FFmpeg below).
  - `crates/rimelight_ui_kit`: shared Slint glue (ST2.4, not yet compiled): the plain rules of the toast queue, colour parsing, table, tree, display, select, menu and theme, plus the `macro_rules!` an app expands to wire them to its generated types (`wire_select!`, `wire_menu!`, `wire_color_hex!`, `wire_theme!`). Depends on `slint` only.
  - `crates/rimelight_shell`: app shell markup (rail, sidebar, status bar, tab bar, more drawer, split pane, command palette, quick action, shortcuts dialog, connection state, About, account menu, sign-in screen), ST2.5a. Plus the plain-Rust shell models (page tabs, history, tile groups, drop zones, workspaces and their saving, palette filter, badge text), generic over the app's `Routes`, ST2.5b. See `docs/st2.5b-shell-model.md`.
- Crates are extracted from rimelight-space in ST2.2 to ST2.7.
- Rules for agents and the layering are in `AGENTS.md`.

## FFmpeg

One FFmpeg setup serves both Rimelight Space and Rimelight Media:

- `vendor/ffmpeg-sys-next`: the vendored low-level bindings (checked in, Windows x64 only), with the build script that
  links the FFmpeg libraries an app enables and copies their DLLs. The "Rimelight changes" section of its
  `README.upstream.md` lists what differs from upstream.
- `tools/ffmpeg/`: `build.sh` and `options.txt` build the trimmed, LGPL, shared FFmpeg for Windows x64; `pin.txt` names
  the build the apps use (release, asset, SHA-256, URL).
- `tools/get-ffmpeg.ps1 -Dest <folder>`: downloads the pinned build, checks it and unpacks it to `<folder>/current`.
- `.github/workflows/ffmpeg.yml`: builds `tools/ffmpeg/` and publishes the zip as a release asset.
- `THIRD-PARTY-NOTICES.md`: the FFmpeg (LGPL) and dav1d (BSD) notices for apps that ship the build.

Apps depend on the vendored bindings through the workspace's `[patch.crates-io]`:

```toml
[patch.crates-io]
ffmpeg-sys-next = { git = "https://github.com/Rimelight-Entertainment/rimelight-rs.git", rev = "..." }
```

and use `ffmpeg-next` with only the features they need:

```toml
ffmpeg-next = { version = "...", default-features = false, features = ["codec", "format", "software-resampling", "software-scaling"] }
```

Add `device` or `filter` only when the app uses them; the build script then links avdevice or avfilter too.
