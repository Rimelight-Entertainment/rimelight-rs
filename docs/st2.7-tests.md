# ST2.7: decision table for the component UI tests

Source: `rimelight-space/crates/space_ui/tests/ui/` (65 test modules plus `main.rs`, 36 markup files in `markup/`).

Rule (the ST2.7 brief): a module moves to rimelight-rs only when (a) its markup imports only `@rimelight_ui/...`,
`@rimelight_design/...`, `@rimelight_shell/...` and `std-widgets.slint`, and (b) its Rust uses nothing from `space_ui`
except items that now live in `rimelight_ui_kit`, plus slint, i_slint_backend_testing, the test markup crate and the
helpers in `main.rs`. Everything else stays in Space.

Checked by grep of each file's imports and paths. Nothing was compiled. "Markup" is the file a module loads through
`rimelight_ui_test_slint::<name>`.

## Summary

- Moves: 26 modules, with their 26 markup files.
- Stays: 38 modules, with the other 10 markup files.
- Helper: `support.rs` (headless backend setup). Copied to rimelight-rs, and kept in Space for the modules that stay.
- Total: 65 test modules (26 + 38 + support). `main.rs` is the harness, so 66 files with it.

## Decision table

| Module | Decision | Reason |
|---|---|---|
| actions | moves | Markup: Button, IconButton, ButtonGroup, Link (rimelight_ui). Rust: slint and `support` only. |
| admin | stays | Rust uses space_ui (`pages::admin`, `slint_ui`, `theme`). |
| admin_users | stays | Rust uses space_ui (`slint_ui`, `slint_ui::calendar`, `theme`). |
| app_shell | stays | Rust uses space_ui app shell (`slint_ui::app_shell`, `slint_ui::shell`). Uses rimelight_nav. |
| back_forward | stays | Rust uses space_ui app shell and sidebar rows. Uses rimelight_nav. |
| back_overlays | stays | Rust uses space_ui app shell. Uses rimelight_nav. |
| banner | moves | Markup: Banner (rimelight_ui). Rust: slint and `support` only. |
| canvas | stays | Rust uses space_ui canvas state and `slint_ui`. Uses rimelight_client, rimelight_proto, serde_json. |
| choice | moves | Markup: Checkbox, Input, RadioGroup, Switch, Slider (rimelight_ui). Rust: slint and `support` only. |
| color_picker | moves | Markup: ColorPicker (rimelight_ui). Rust uses `rimelight_ui_kit::color`. |
| combobox | moves | Markup: Combobox, SelectOption, SelectNav (rimelight_ui). Rust uses `rimelight_ui_kit::select`. |
| company | stays | Rust uses space_ui (`slint_ui` tables and `app_shell`). |
| connection_lost | stays | Rust uses space_ui (`AppShell`, `ConnectionState`, `app_shell`). No markup. |
| context_menu | moves | Markup: ContextMenu, MenuNav, Icons (rimelight_ui). Rust uses `rimelight_ui_kit::menu`. |
| copy_link | stays | Rust uses space_ui app shell, `history::link_for` and `shell`. Uses rimelight_nav. |
| date_b6 | stays | Markup is allowed (CalendarGrid, DatePicker, DateRangePicker, TimePicker), but the Rust uses `space_ui::slint_ui::calendar`. The calendar rules are not in `rimelight_ui_kit`. |
| departments | stays | Rust uses space_ui (`DeptsTable`, `PageLoadState`, `app_shell`). Uses rimelight_client, rimelight_proto. |
| dialog | moves | Markup: Button, Dialog, ConfirmDialog (rimelight_ui). Rust: slint and `support` only. |
| display | moves | Markup: Badge, Tag, AvatarStack, Progress (rimelight_ui), Tokens (rimelight_design). Rust uses `rimelight_ui_kit::display`. |
| employees | stays | Rust uses space_ui (`EmployeesTable`, `app_shell`). Uses rimelight_client, rimelight_proto. |
| feedback | moves | Markup: EmptyState, ErrorState, Icons (rimelight_ui). Rust: slint and `support` only. |
| field | moves | Markup: Field, Form, Input, NumberInput (rimelight_ui). Rust: slint and `support` only. |
| issue | stays | Rust uses space_ui (`pages::projects::issue`, `PageLoadState`, `app_shell`). Uses rimelight_client, rimelight_proto, serde_json. |
| issues | stays | Rust uses space_ui (`PageLoadState`, `app_shell`). Uses rimelight_client, rimelight_proto. |
| keyboard | moves | Markup imports only `std-widgets.slint` (Button, LineEdit). Rust: slint and `support` only. |
| locations | stays | Rust uses space_ui (`LocationsState`, `PageLoadState`, `AppShell`). Uses rimelight_client, rimelight_proto. |
| menu | moves | Markup: Button, Menu, MenuItem, Icons (rimelight_ui). Rust uses `rimelight_ui_kit::menu`. |
| more_drawer | stays | Markup is allowed (`@rimelight_shell` MoreDrawer, RailItem, JobItem), but the Rust uses space_ui (`app_shell`, `JobItem`). Uses rimelight_nav. |
| new_issue | stays | Rust uses space_ui (`app_shell`, `projects`). Uses rimelight_proto. |
| number_input | moves | Markup: NumberInput, Input (rimelight_ui). Rust: slint and `support` only. |
| page_header | stays | Rust uses space_ui (`app_shell`, `MenuItem`, `projects`). Uses rimelight_nav. |
| page_tabs | stays | Rust uses space_ui app shell and `shell`. Uses rimelight_nav. |
| pane_headers | stays | Rust uses space_ui app shell, `history` and `shell`. Uses rimelight_nav. |
| persistence | stays | Rust uses space_ui (`app_shell`, `workspace`). |
| phone_workspaces | stays | Rust uses space_ui (`AppShell`, `app_shell`). |
| pointer_drag | stays | Rust uses space_ui (`app_shell`, `shell`, `TileGroup`). Uses rimelight_nav. |
| popover | moves | Markup: Button, Input, Popover (rimelight_ui). Rust: slint and `support` only. |
| popover_place | moves | Markup: Popover (rimelight_ui). Rust: `support` only. |
| profile | stays | Markup imports `ui/pages/company/profile.slint` (a Space page). Rust uses space_ui, rimelight_client, rimelight_proto. |
| project_tree | stays | Rust uses space_ui (`app_shell`, `palette`, `projects`, `shell`). Uses rimelight_nav. |
| quick_action | stays | Rust uses space_ui (`AppShell`, `app_shell`). Uses rimelight_nav. |
| rail_badge | stays | Markup is allowed (`@rimelight_shell` Rail, RailItem, RailBadge), but the Rust uses space_ui `shell::rail_badges`. |
| select | moves | Markup: Select, SelectOption, SelectNav (rimelight_ui). Rust uses `rimelight_ui_kit::select`. |
| sheet | moves | Markup: Button, Sheet (rimelight_ui), Tokens (rimelight_design). Rust: slint and `support` only. |
| shell | stays | Rust uses space_ui (`AppShell`, `Icons`, `palette`, `projects`, `shell`, `status`), `palette_forms`, `workspace_ui`, rimelight_nav. Its markup (shell_1, shell_2, shell_3) is allowed but stays with it. |
| shell_chrome | stays | Markup imports `ui/components/profile-menu.slint` (Space-only). Rust uses space_ui `app_shell`. |
| shortcuts | stays | Rust uses space_ui (`AppShell`, `app_shell`). No markup. |
| sign_in | stays | Markup imports `ui/components/profile-menu.slint` (Space-only). Rust uses space_ui (`AppShell`, `SessionPhase`, `app_shell`). |
| status_bar | stays | Markup is allowed (`@rimelight_shell` StatusBar, JobsMenu, JobItem), but the Rust uses space_ui (`JobItem`, `app_shell`). |
| strip_keyboard | stays | Rust uses space_ui app shell, `shell` and `AppShell`. Uses rimelight_nav. |
| support | helper, copied and kept | Headless backend setup used by every module. Copied to rimelight-rs; Space keeps its copy for the modules that stay. |
| tab_drag | stays | Rust uses space_ui app shell and `shell`. Uses rimelight_nav. |
| team | stays | Rust uses space_ui (`PageLoadState`, `app_shell`). Uses rimelight_client, rimelight_proto. |
| teams | stays | Rust uses space_ui (`TeamsTable`, `PageLoadState`, `app_shell`). Uses rimelight_client, rimelight_proto. |
| text_fields | moves | Markup: Input, InputSize (rimelight_ui). Rust: slint and `support` only. |
| textarea | moves | Markup: Textarea (rimelight_ui). Rust: slint and `support` only. |
| tier2_b1 | moves | Markup: Card, Section, StatusDot, PresenceDot, Avatar, Breadcrumbs, Badge (rimelight_ui). Rust: slint and `support` only. |
| tier2_b2 | moves | Markup: Tabs, Accordion, Stepper, Pagination (rimelight_ui). Rust: slint and `support` only. |
| tier2_b3 | moves | Markup: Toolbar, FilterBar, Tree, ScrollArea, ResizablePanel (rimelight_ui). Rust uses `rimelight_ui_kit::tree`. |
| tier3_b4 | moves | Markup: DescriptionList, Stat, Timeline, ActivityFeed, Badge (rimelight_ui). Rust: slint and `support` only. |
| tier3_b5 | moves | Markup: List, Table (rimelight_ui). Rust uses `rimelight_ui_kit::table`. |
| toast | moves | Markup: Button, ToastStack (rimelight_ui). Rust uses `rimelight_ui_kit::toast` and `i_slint_backend_testing`. |
| tooltip | moves | Markup: IconButton, Icons (rimelight_ui). Rust: slint and `i_slint_backend_testing` only. |
| tiles_ui | stays | Rust uses space_ui (`AppShell`, `TileGroup`, `app_shell`, `shell`). Uses rimelight_nav. |
| workspaces | stays | Rust uses space_ui (`AppShell`, `MenuKind`, `app_shell`, `shell`). Uses rimelight_nav. |

## Moved: 26 test modules and 26 markup files

actions, banner, choice, color_picker, combobox, context_menu, dialog, display, feedback, field, keyboard, menu,
number_input, popover, popover_place, select, sheet, text_fields, textarea, tier2_b1, tier2_b2, tier2_b3, tier3_b4,
tier3_b5, toast, tooltip. Each `.rs` is in `crates/rimelight_ui/tests/ui/`, each `.slint` in `tests/ui/markup/`.

## Markup that stays in Space (10 files)

date_b6, more_drawer, profile, rail_badge, shell_1, shell_2, shell_3, shell_chrome, sign_in, status_bar.

## Environment notes

- `SLINT_EMIT_DEBUG_INFO=1` (ElementHandle lookups): no moved test needs it. No test in either tree uses ElementHandle any
  more (the four that did now read AppShell test properties, per the Space decision on debug info), and no build sets it.
- The moved tests that wire callbacks (combobox, context_menu, menu, select) keep a local `wire_nav` over
  `rimelight_ui_kit`. None calls Space's `glue::` module, so the kit macros (`wire_select!`, `wire_menu!`,
  `wire_color_hex!`) are not needed in the harness.

## Icon tests

Space's `crates/space_ui/src/slint_ui/assets.rs` and `crates/space_ui/src/icon.rs` checked the icon pair.

- Moved to rimelight-rs (`crates/rimelight_ui/src/lib.rs`, module `icon_files`): every icon named by `ui/icons.slint` has
  a file in `assets/icons`, and the LICENSE is ISC. They read the folder relative to `CARGO_MANIFEST_DIR`.
- Kept in Space, because they need Space's `IconName` or `rimelight_nav`: `the_icon_markup_names_exactly_the_mapped_icons`
  (`assets.rs`: icons.slint against `IconName` and the two lists) and `every_page_icon_is_a_sidebar_icon` (`assets.rs`).
- Deleted from Space: `every_icon_has_a_file`, `slint_only_icons_have_files`, `the_icon_licence_is_bundled`, the file loop
  in the sidebar test, `IconName::asset_path` and its test `every_mapping_resolves_to_a_bundled_file` (`icon.rs`). The
  files they checked are now in rimelight-rs.
- Space's `crates/space_ui/assets/icons` folder is deleted. Nothing in Space reads it at build or run time: the markup uses
  the shared `ui/icons.slint` from rimelight_ui, which is the only `@image-url` to that folder, and no `include_bytes`,
  `include_str` or path read touches it. `assets/stories/` stays (the gallery's avatar sample).

## Not verified

Nothing was compiled or run (no cargo, by brief). Open risks: the dev-dependency cycle (rimelight_ui dev-depends on
rimelight_ui_test_slint, which build-depends on rimelight_ui); `Cargo.lock` in rimelight-rs is not updated; the moved
tests and markup have not been built in their new crate; `rimelight_design` is a git dependency, so the moved markup's
`@rimelight_design` import resolves only after the pinned rev is fetched.
