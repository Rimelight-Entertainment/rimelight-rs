# rimelight-rs

Shared Rust crates for Rimelight's native apps (Rimelight Space, Rimelight Media, idantity-native): Slint
components, shell, glue, media playback, picture decoding, platform calls and self-update. MIT licensed (decision D2).

The design tokens are not here. They live in the web monorepo (`Rimelight-Entertainment/rimelight`,
`packages/design`), which generates the `rimelight_design` crate that these crates depend on (decision D1).

The plan these crates follow is `docs/plan.md` in rimelight-space.

## Status

- `spikes/slint-cross-crate`: proof that a library can ship Slint markup and Rust glue while only the app compiles
  Slint (ST2.1, passed 2026-11-22).
- Crates are extracted from rimelight-space in ST2.2 to ST2.7.
