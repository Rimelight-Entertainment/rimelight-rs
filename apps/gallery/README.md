# Rimelight gallery

The component gallery, on Slint. It lists the design system's stories in a navy sidebar, grouped by section (Foundations,
Components, Templates, Shell), with a toolbar and the selected story in a frame. It is app-independent: the stories come
from `rimelight_ui`, `rimelight_shell` and `rimelight_design`, and the shell and palette stories use sample data
(`src/sample.rs`). An app can add its own stories (see below).

The window (`ui/gallery.slint`) and the stories (`ui/stories.slint`, `ui/component-stories.slint`,
`ui/shell-stories.slint`, `ui/foundations.slint`) are not compiled into Rust. `src/harness.rs` loads `ui/dev.slint` with
`slint-interpreter` at run time (decision 0044). The built-in story registry is `src/registry.rs`. `src/nav.rs` answers the
Select, Menu, ColorPicker, palette and shell callbacks with the rules in `rimelight_ui_kit` and `rimelight_shell`.

## Live reload

While the window is open, the gallery checks `ui/` (and the app's story folder, when given) every half second. On a change it
recompiles the markup and swaps in a new window on the same story with the same toolbar choices. A markup error is printed and
the old window stays. A `.slint` edit to a story or a component needs no cargo build. A change to Rust (the registry or the
wiring) still needs one.

Stories in Foundations: `foundations/tokens` (variant `focus`), `foundations/typography` (variants `weights`, `mono`),
`foundations/icons`, `foundations/spacing-and-radius`, `foundations/motion`, `foundations/about` (variant `sidebar-focus`).
Components, Templates and Shell hold the component stories and the shell stories.

Templates (ST5.4, the page templates in `rimelight_shell/ui/templates/`): `templates/list-page` (variants `loading`, `empty`,
`error`, `selected`), `templates/board-page` (variants `empty`, `error`, `move-refused`) and `templates/detail-page` (variants
`loading`, `empty`, `error`). Each story carries its own sample data. The board story has no drop target (its `column-at` answers
-1), so it shows the states but not the drag.

## Run

```
cargo run -p rimelight_gallery
cargo run -p rimelight_gallery -- --story foundations/typography#weights --theme dark --locale pt
cargo run -p rimelight_gallery -- --list-stories
cargo run -p rimelight_gallery -- --foundations --theme dark
cargo run -p rimelight_gallery -- --stories path/to/app/stories
```

Flags:

- `--stories <dir>`: an app's story folder (below). Without it only the built-in stories show.
- `--story <id[#variant]>`: the start story, for example `foundations/typography#mono`. App stories are accepted too.
- `--theme <light|dark|system>` (default `system`), `--locale <en|pt>` (default `en`).
- `--viewport <desktop|tablet|phone>` (default `desktop`): the story frame is full width, 768 px or 390 px.
- `--text-scale <100|125|150|200>` (default 100): scales type, line heights, icons and spacing bars.
- `--reduced-motion <on|off>` (default `off`): every motion duration becomes 0.
- `--chrome <on|off>` (default `on`): `off` draws only the story.
- `--size <WxH>` (default 1280x800).
- `--screenshot <out.png>`: capture one frame with `Window::take_snapshot` and exit.
- `--list-stories`: print every id. A variant follows its story. App stories follow the built-ins.
- `--foundations`: the foundations sheet (S1) instead of the gallery. Theme, locale, text scale and size apply.

Exit codes: 0 ok, 2 bad arguments or a bad story folder, 3 the window or capture failed, 4 the frame is blank (fewer than 8
distinct colours).

## App stories

An app points the gallery at a folder with `--stories <dir>`, or passes that folder to `rimelight_gallery::main_with`
(or `run`) from its own binary. The folder holds two files.

`stories.txt` lists one story per line, as `section/name|Title|variant1,variant2`. A blank line and a line starting with
`#` are skipped. The variants are optional. The section is one of `foundations`, `components`, `templates` or `shell`, and
the id must be unique. The sidebar lists every app story under "App stories", after the built-ins. Lines keep their order.
the first app story is at index `builtin-count`, the next one above it. The parser is `registry::parse_app_stories`.

```
# the app's stories
shell/inbox|Inbox|unread,archived
components/review-card|Review card
```

`stories.slint` is the app's markup. It must export this component:

```
export component AppStories inherits Rectangle {
    in property <int> story;     // 0 for the first line of stories.txt, 1 for the second, ...
    in property <bool> pt;       // true when the toolbar's locale is pt
    in property <int> variant;   // the selected variant of that story (0 is the default)
}
```

The app's markup imports the design system the same way the gallery does (`@rimelight_ui/...`, `@rimelight_design/...`,
`@rimelight_shell/...`). It can also import its own files next to `stories.slint`, through the path `@app_stories/...`.

`GalleryWindow` imports `AppStories` from the library path `@app_stories` and shows it for every story at or past
`builtin-count`. The harness maps `@app_stories` to the app folder, or to the empty stub `ui/no-app-stories` when no
folder is given, so there is one window either way. The slot sits in the story frame, right after the built-in `if root.story == N` chain.

## Captures

`cargo run -p rimelight_gallery -- --story <id[#variant]> --theme <light|dark> --size <WxH> --screenshot <out.png>` captures
one story. The capture is refused as blank when the frame has fewer than 8 distinct colours. Look at each capture before
keeping it: a blank, clipped or untranslated image is a failed capture.

## Keyboard

- Up and Down move the sidebar selection between stories (the sidebar has focus after a click, or with `sidebar-focus`).
- Tab moves through the toolbar chips in order: theme (Light, Dark, System), locale (en, pt), viewport, text scale, motion.
- Space or Return chooses the focused chip. The focus ring is drawn from `Tokens.focus-ring`.

The keyboard path is tested in `src/harness.rs` (`cargo test -p rimelight_gallery -- gallery_compiles`) with real key events.
The app-stories path is tested in the same test, with a folder written to the system temp directory.

## Adding a built-in story

Add a variant or a story to `STORIES` in `src/registry.rs`, add its component to `ui/stories.slint` (or
`ui/component-stories.slint`), and add its `if root.story == N` branch to `ui/gallery.slint` in the same order, then set
`builtin-count` in `ui/gallery.slint` to the new count. The registry test checks the branch count.

## Not built yet

- Android and web gallery builds.
- Visual regression comparison. Check a change by looking at the new capture.
