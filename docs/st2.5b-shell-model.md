# ST2.5b: the shell's Rust models and the app frame (design, answered)

**Status:** answered by the owner, 2026-11-22; ready to build. ST2.5a (the shell markup) is done.

## Where things stand

ST2.5a moved the shell **markup** to `crates/rimelight_shell`:
- rail, sidebar, status bar and jobs menu;
- tab bar, more drawer, split pane;
- command palette, quick action, shortcuts dialog;
- connection state and the About screen.

The **Rust** that fills those components still lives in Space, because almost all of it reads Space's route manifest
(`rimelight_nav`):

| Space module | Lines | Tie to Space |
|---|---|---|
| `slint_ui/tiles.rs` (tab groups, splits, drag rules, layout persistence) | 1,300 | One call: the Home page title as the default tab title |
| `slint_ui/tile_drop.rs` (drop zones while dragging a tab) | 441 | None besides `tiles.rs` |
| `slint_ui/workspace.rs` (named workspaces of tiles) | 1,413 | `Area::Home` as the default, and the Home title |
| `slint_ui/palette.rs` (command palette rows) | 93 | Rows come from the manifest's pages (`pages()`, `format_route`) |
| `slint_ui/shell.rs` (rail and sidebar rows, badges, statuses) | 490 | Built from `Area`, `Page`, `Status`, `sections_in`, `pages_in`, `parse` |
| `ui/app-shell.slint` (the window: frame, panes, overlays) | 2,989 | Hosts Space's pages, sign-in, profile menu and quick-action items |
| `slint_ui/app_shell.rs` (the driver) | 3,606 | Wires everything, with 20 references to Space pages |

## Proposal

1. **A route trait in `rimelight_shell` (Rust side).** Apps describe their navigation; the shared models consume it:

   ```rust
   pub trait Routes {
       /// Rail entries, in order: id, label, icon name, optional badge count.
       fn areas(&self) -> Vec<AreaInfo>;
       /// Sidebar tree for one area: headings with pages (route, title, icon, optional status).
       fn sidebar(&self, area: &str) -> Vec<SidebarSection>;
       /// Every page the palette can open (route, title, area, keywords).
       fn pages(&self) -> Vec<PageInfo>;
       /// Title, icon and area for an open route, or None if the route is unknown.
       fn describe(&self, route: &str) -> Option<RouteInfo>;
       /// The route new tabs and empty groups fall back to.
       fn home(&self) -> &str;
   }
   ```

   Space implements it over `rimelight_nav` in a few dozen lines. Media and idantity implement it over their own page
   lists.
2. **Move the models in this order, because each step is independent and testable:**
   1. `tiles.rs` and `tile_drop.rs`. The only change is that the Home title comes from `Routes::home()`/`describe()`.
   2. `workspace.rs`. It loses `Area::Home` in favour of `Routes::home()`.
   3. `palette.rs`. Rows come from `Routes::pages()`.
   4. `shell.rs`. Rail and sidebar rows come from `areas()` and `sidebar()`. Status words stay Space's (`Status`), mapped
      by the app.

   The wiring that names generated Slint types becomes kit-style macros (`wire_tiles!`, `wire_palette!`), as in ST2.4.
3. **The window stays per app for now.** Slint components accept only one `@children` slot, so a generic `AppFrame`
   that hosts an app's pages, overlays and menus would need either one large component with many properties, or a
   split into frame pieces (`ShellRail`, `ShellBody`, `ShellStatus`) that each app arranges itself.

   Recommendation: the **frame pieces**. Each app keeps a short `app-shell.slint` that places the shared pieces and its
   own pages. Space's 2,989-line file shrinks as pieces move out. A single `AppFrame` would force one layout on Media,
   which has no rail or sidebar.

## Owner answers (2026-11-22)

All four recommendations accepted:
1. Media uses only the status bar, the shortcuts dialog and the About screen.
2. Frame pieces, not one `AppFrame`.
3. The profile menu and sign-in screen move to `rimelight_shell` as `AccountMenu` and `SignInScreen`.
4. The sidebar's "Projects" label becomes a property, in step 4.

## Questions for the owner (as asked)

1. **Does Rimelight Media use the shell at all?**
   - Its layout is a top toolbar, an edit row, a stage and a status bar, with no rail, sidebar or tabs.
   - Recommendation: Media uses only the status bar, the shortcuts dialog and the About screen from `rimelight_shell`.
   - The rail, sidebar, tiles and workspaces are for Space and idantity.
2. **Frame pieces or one `AppFrame` component?** Recommendation: frame pieces (point 3).
3. **Where do the profile menu and the sign-in screen go?**
   - Both are Space-flavoured today: account, presence, Auth0 sign-in states.
   - idantity also signs in through Auth0 (`@rimelight/auth`).
   - Recommendation: both move to `rimelight_shell` with neutral names (`AccountMenu`, `SignInScreen`), and
     product-specific rows come in through properties.
4. **The sidebar's project switcher has a literal "Projects" label.** Turn it into a property? Recommendation: yes, as
   part of step 4 above.

## Gate (per moved model)

- Its unit tests move with it and pass in rimelight-rs.
- Space builds against the pinned rev with no visible change: clippy, `--lib`, `--test ui`, and the host, wasm32 and
  Android checks.
