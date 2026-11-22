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
  - `crates/rimelight_ui`: Slint component library (markup in `ui/`, filled in ST2.3); placeholder for now.
  - `apps/`: empty; app crates that compile Slint come here.
  - `xtask/`: workspace task runner (`cargo xtask`); prints usage only for now.
  - `spikes/`: its own workspace, excluded from this one.
  - `vendor/`: excluded; added with the FFmpeg bindings when `rimelight_media` is extracted.
- Crates are extracted from rimelight-space in ST2.2 to ST2.7.
- Rules for agents and the layering are in `AGENTS.md`.
