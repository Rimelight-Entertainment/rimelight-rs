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
  - `crates/rimelight_ui`: Slint component library (ST2.3, not yet compiled): 65 component files (74 exported
    components), typography, icons, and bundled fonts. Markup only; the app crates compile it. The fonts are Noto Sans
    and JetBrains Mono, licensed under SIL OFL 1.1 (see `crates/rimelight_ui/ui/fonts/README.md`).
  - `crates/rimelight_ui/tests/ui`: the one `ui` test binary for the components (ST2.7, not yet run): 26 test modules
    moved from Space, with their markup in `tests/ui/markup/`, plus the icon-file checks in `src/lib.rs`. The decision
    table is `docs/st2.7-tests.md`.
  - `crates/rimelight_ui_test_slint`: test-only crate (not published) that compiles `rimelight_ui`'s test markup, one
    module per file, for the ui test binary.
  - `apps/ui_check`: compile check. Its `ui/main.slint` places every exported component of `rimelight_ui` in one
    window, and its headless test creates that window. Not shipped.
  - `apps/`: other app crates that compile Slint come here.
  - `xtask/`: workspace task runner (`cargo xtask`); prints usage only for now.
  - `spikes/`: its own workspace, excluded from this one.
  - `vendor/`: excluded; added with the FFmpeg bindings when `rimelight_media` is extracted.
  - `crates/rimelight_ui_kit`: shared Slint glue (ST2.4, not yet compiled): the plain rules of the toast queue, colour parsing, table, tree, display, select, menu and theme, plus the `macro_rules!` an app expands to wire them to its generated types (`wire_select!`, `wire_menu!`, `wire_color_hex!`, `wire_theme!`). Depends on `slint` only.
  - `crates/rimelight_shell`: app shell markup (rail, sidebar, status bar, tab bar, more drawer, split pane, command palette, quick action, shortcuts dialog, connection state, About), ST2.5a. Its Rust models are designed in `docs/st2.5b-shell-model.md`.
- Crates are extracted from rimelight-space in ST2.2 to ST2.7.
- Rules for agents and the layering are in `AGENTS.md`.
