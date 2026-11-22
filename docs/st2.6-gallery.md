# ST2.6: the gallery (design, answered)

**Status:** built, 2026-11-22 (see Result at the end).

## What the gallery is today

The gallery lives in Space as `apps/gallery`.
- It loads the story markup (`space_ui/ui/dev.slint`, `gallery.slint`, `component-stories.slint`,
  `shell-stories.slint`, `stories.slint`, `foundations.slint`) on the Slint interpreter, with hot reload
  (decision 0044).
- It has a story registry (`registry.rs`, about 100 stories) and switches for theme, locale, viewport, text scale and
  reduced motion.
- It does headless captures (`--story <id> --out <png>`).

**The markup is almost all shared.** Every story imports only `@rimelight_ui`, `@rimelight_shell` and
`@rimelight_design`, except one: the kanban column story (`components/kanban-column.slint`, which is Space-only
because it shows issue cards).

**The Rust side has four ties to Space:**

| Tie | Where | Generic replacement |
|---|---|---|
| `space_ui::theme::ThemeMode` | `harness.rs`, `main.rs` | `rimelight_ui_kit::theme::Mode` |
| The Select, Menu and ColorHex wiring, through Space's `glue` modules (Space's generated types) | `nav.rs` | Kit rules applied to interpreter `Value`s directly (no generated types needed in an interpreter app) |
| `shell_capture`: `--story shell/<state>` renders Space's whole `AppShell` | `main.rs` | None. It is Space's app, not a story |
| One test compares the rail to `rimelight_nav::Area::ALL` | `harness.rs:706` | Compare to the shell story's own rows |

## Proposal

1. **Move the gallery to rimelight-rs `apps/gallery`**, with every story except kanban, the foundations sheet, the
   avatar sample asset and the registry. Swap the three replaceable ties (table rows 1, 2 and 4). It then shows
   `rimelight_ui` and `rimelight_shell` for every app, and Media and idantity can use it.
2. **Whole-app captures stay in Space** as a capture mode of the desktop app (`rimelight_desktop --capture
   shell/<state> --out <png>`) or as an `xtask screenshot shell/<state>` command. `shell_capture.rs` already lives in
   `space_ui`, so only the command-line entry moves.
3. **The kanban story:** Space keeps one small `ui/space-stories.slint`. The shared gallery has no way to load an
   app's extra stories, so either
   - (a) the shared gallery gains a `--stories <dir>` flag that adds an app's own story file, or
   - (b) Space's kanban story is dropped and kanban is checked in the running app.

## Owner answers (2026-11-22)

Both recommendations accepted:
1. Whole-app screenshots move to `cargo xtask screenshot shell/<state>` in Space.
2. The shared gallery gains a `--stories <dir>` flag for an app's own stories; Space keeps kanban there.

## Questions for the owner (as asked)

1. **Where should whole-app screenshots live:** a desktop-app flag or an `xtask` command? Recommendation: `cargo xtask
   screenshot shell/<state>`, which matches the existing `xtask screenshot windows|android`.
2. **App-specific stories:** (a) a `--stories` flag, so Space (kanban, issue card, profile menu, sign-in) and later
   Media and idantity can add their own stories to the shared gallery, or (b) shared stories only? Recommendation:
   (a).

## Gate

- The gallery's own tests pass in rimelight-rs, and the headless captures of a story, the foundations sheet and the
  About screen work.
- Space builds without `apps/gallery`, and its whole-shell capture works through the chosen command.

## Result (2026-11-22)

- **The gallery is `rimelight-rs/apps/gallery`** (`rimelight_gallery`, a lib and a bin), with 67 built-in stories, the
  foundations sheet and hot reload. It names no app: Select, Menu and ColorHex run the kit rules on interpreter values,
  and the shell and palette stories use sample data (`src/sample.rs`).
- **App stories:** `--stories <dir>` adds a folder with `stories.txt` (`section/name|Title|variants`) and
  `stories.slint` exporting `AppStories` (`story`, `variant`, `pt`). `gallery.slint` always imports
  `AppStories` from `@app_stories`. Without the flag, that path maps to an empty stub (`ui/no-app-stories`). A first
  try generated a derived window instead, but the interpreter does not expose a base window's properties on a derived
  one, so it was dropped.
- **Space:** `apps/gallery` is now `space_gallery`. It is the shared gallery with Space's stories
  (`crates/space_ui/ui/stories`, the kanban board) as the default folder, plus the whole-app capture
  `--story shell/<state>`. The capture also runs as `cargo xtask screenshot shell/<state> --out <png>`.
- Checked by eye: the shared gallery's rail and sidebar, palette, button and foundations captures, plus Space's kanban
  story and the `desktop-default` whole-app capture.
