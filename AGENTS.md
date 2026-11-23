# Agent rules for rimelight-rs

## How we work: build ONCE, at the very end (owner rule, overrides everything else)

Builds fill the owner's disk, lag the whole PC and used to take most of every session. So:

- **Write the features first, in big batches.** Many steps, across repos if needed, with no build, check, test,
  clippy, gate, measurement or screenshot in between.
- **Then run ONE build and test pass** to find what broke along the way, fix it all, and commit.
- **Never** build per step, per agent, per repo, before a push, after a pin bump, to "measure first", or to "just check".
  A plan document's per-step gate (`gate quick`, "gate per run", "measure first") does not override this.
- Subagents never run cargo, `vp`, npm builds or tests at all.
- Before the one build, if the disk has under 40 GB free, delete `debug/incremental` in the target folder.
- Never sit idle waiting on a build. If something must be built, it is the final pass.

Shared Rust crates for Rimelight's native apps. The plan these crates follow is `docs/plan.md` in rimelight-space
(sections 4 and 5, WS2). Read it before starting a step.

## Working rules

- Work on exactly one step at a time, by step ID (for example `ST2.3`). Do not expand scope.
- If a step cannot be done as written, or a task forces a choice the plan does not make, stop and report it. Do not
  silently deviate.
- Stated facts only: say what was checked and what was not.
- Owner dislikes dangling leftovers: when told to remove or move something, delete it completely and grep for references.

## Git rules

- Commit only when the task asks for it. One local commit per finished step; the message starts with the step ID.
- Stage only the files your step created or changed.
- Never `git commit --no-verify`. If the pre-commit hook fails, fix the reported problem and recommit.
- Never edit repo-wide tool config (`Cargo.toml` lints, `rust-toolchain.toml`, `.cargo/config.toml`, the hook) to get
  past a hook or a failing check. Fix the code, or stop and report.
- Push, open pull requests and create remote branches only when the owner says so.

## Verification

- Subagents never run cargo, in any form (build, check, test, clippy, fmt, `cargo run`, target checks). They write code,
  re-read it, leave it uncommitted and report the files they changed, and say what is unverified.
- The main session builds once per batch, with no subagent running: `cargo fmt --all`, then
  `cargo clippy --workspace --all-targets -- -D warnings`, then the tests (`cargo test --workspace`), then
  `cargo check --workspace --lib --target wasm32-unknown-unknown`.
- Every cargo command selects `--workspace` and narrows with target flags and test filters, never with `-p` or
  `--exclude` (cargo unifies features over the selected packages).
- Run at most one cargo build at a time. All builds share one target folder: the user environment variable
  `CARGO_TARGET_DIR` (`C:\Users\idant\RustRoverProjects\rimelight-space-target`, the same folder Space uses). Do not
  create a `target/` folder inside a worktree. Never move or copy build output by hand.
- Dry run of the hook, which runs no cargo: `RIMELIGHT_HOOK_DRY=1 sh .githooks/pre-commit`.
- Do not run `cargo xtask clean` routinely.
- `spikes/` is its own workspace. Do not change it from a library step.

## Layering (plan section 4)

1. Only app crates compile Slint. Library crates ship `.slint` markup and a `UI_DIR` constant; the app's `build.rs`
   registers them as library paths (`@rimelight_ui`, and so on).
2. Library Rust code never names generated Slint types. Glue is written against small traits, or against a
   `macro_rules!` the app expands inside its own crate (`wire_toasts!(AppWindow)` style).
3. Dependency direction: tokens, then ui, then ui_kit and shell, then domain crates, then the app. Nothing points
   backwards. `rimelight_ui` has no network, file, FFmpeg or protocol dependencies.
4. No app code in `rimelight_ui`. Components are generic; anything that knows about one product lives in the shell or
   in that product.
5. Shared crates are pinned by git tag in every consumer. `rimelight_design` is pinned by git rev in
   `[workspace.dependencies]` and bumped deliberately.

## Code rules

- The workspace forbids `unsafe_code`. Any `unsafe` needed later (platform crates) goes in one audited crate with the
  lint relaxed locally and a written reason.
- Library crates opt in to the workspace lints with `[lints] workspace = true`.
- New UI tests go into `crates/rimelight_ui/tests/ui/<name>.rs` as a module of the single `ui` test binary. Never add a
  new top-level `tests/*.rs` file: each one links all of Slint again. Test-only markup goes in a test-markup crate, not a
  `slint!` macro in a test.
