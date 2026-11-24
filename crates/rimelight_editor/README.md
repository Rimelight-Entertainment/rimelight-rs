# rimelight_editor

The shared Slint block editor (decision 0049, CMS.5; design `rimelight-space/docs/cms-editor-design.md`, sections 2, 3,
6, 7, 8, 9 and 10). One editor for wiki, docs, blog, franchise content and screenplays, used by Space and later by
idantity-native.

## What is in the crate

- `ui/`: the editor markup. `block-editor.slint` exports `BlockEditor` (the component the app places) and the global
  `EditorBridge` (every property and callback the editor shares with Rust). The rest are its parts:
  `editor-row.slint` (one block row: transparent `TextInput` over `StyledText`, handle or phone grip, presence bar,
  remote carets, comment badge), `insert-menu.slint` (the `/` menu), `mention-picker.slint` (the `@` picker),
  `emoji-picker.slint` (the `:` picker), `block-menu.slint` (the block actions), `link-prompt.slint` (Ctrl+K),
  `locale-switcher.slint` (the language chips), `comment-panel.slint` (threads, reply, resolve, show),
  `inspector.slint` (registry fields), `format-toolbar.slint`, `web-block.slint` (the place of a web-only block) and
  `types.slint` (the structs and the `KeyOutcome` enum). Markup imports the shared components and tokens from
  `@rimelight_ui/...` and `@rimelight_design/...`.
- `src/controller.rs`: `EditorController`, plain Rust. It owns the `rimelight_content::editor::EditorDoc` and the editor
  state (locale, focus, block selection, the menus, the link prompt, the comment panel, the inspector). Every user action
  is a method that returns `ControllerEffect`s for the app to act on. The keyboard map, the markdown shortcuts and the
  registry-filtered insert menu live here.
- `src/emoji.rs`: the built-in emoji table the `:` picker searches (shortcode and emoji).
- `src/view.rs`: `RowView` (what the page shows per block), `to_markdown` (rich text to the markdown `StyledText`
  draws, with user text escaped and mentions as `[@Title](mention:<id>)` links), and the heading sizes.
- `src/layout.rs`: parley geometry: `caret_rect` (remote carets), `line_edges` (is the caret on the first or last visual
  line) and `offset_at` (the byte offset nearest an x position on a line, for Up and Down).
- `src/webview.rs`: the `WebViewHost` trait and `embed_url`.
- `src/lib.rs`: `UI_DIR`, `HostEvent`, `hex_rgb`, and the `wire_block_editor!` macro.

## Layering

This is a library crate (AGENTS.md, layering 1 and 2):

- It ships markup and `UI_DIR`. It never compiles Slint.
- Its Rust never names a generated Slint type. The crate has no `slint` dependency.
- The app's `build.rs` registers `UI_DIR` as the library path `rimelight_editor`, next to `rimelight_ui` and
  `rimelight_design`.
- The glue to the window is `wire_block_editor!`, which the app expands in its own crate.
- All editor state and callbacks live in the global `EditorBridge`, not on `BlockEditor`. `BlockEditor` binds to the
  bridge itself, so the app's window needs no bindings to it.

## How an app wires it

1. Register the library path in `build.rs`, import the component and the global in the app's markup, and re-export the
   global from the root file so Rust can name it (`window.global::<EditorBridge>()` needs it exported from the root):

   ```slint
   import { BlockEditor, EditorBridge } from "@rimelight_editor/block-editor.slint";

   export { EditorBridge }

   export component AppShell inherits Window {
       // ... the app's own chrome ...
       BlockEditor { }
   }
   ```

   `BlockEditor` takes no bindings. Its width and height come from its parent, like any element.

2. Build the controller and expand the macro, with the app's window type and the generated `EditorBridge` name first,
   then the generated types of the app's build:

   ```rust
   rimelight_editor::wire_block_editor!(
       AppShell, EditorBridge,
       EditorRow, RemoteCaret, InsertItem, MentionItem, InspectorField, KeyOutcome,
       EmojiItem, BlockMenuItem, LocaleItem, CommentItem, PresenceItem
   );

   let controller = Rc::new(RefCell::new(EditorController::new(doc, "en")));
   let binding = bind_block_editor(&window, controller.clone(), move |event| {
       // HostEvent::Effect(effect):
       //   SyncNeeded sends SyncSession::outgoing; MentionSearch runs the entry search and calls set_mention_results;
       //   CommentRequested loads the threads and calls set_comments; Comment { request } calls the comments API
       //   (Submit, Resolve, Reopen) and then set_comments; LocaleChanged may load per-locale data; Error shows a toast.
       // HostEvent::WebGeometry { key, x, y, w, h, url }: calls WebViewHost::show.
   });
   ```

   `EditorController::new` builds the insert menu from the registry (`insert_catalogue`), so the app passes no menu.
   Keep the returned `Rc<BlockEditorBinding>` alive while the window is shown.

   One editor is live per window at a time, because `EditorBridge` is one global per window. An app with several open
   pages binds once per window, then calls `binding.swap_controller(page_controller.clone())` when the active tab
   changes. That redraws the editor from the new controller.

3. Pass the editor's inputs as they change: `binding.set_remote_cursors(cursors)` from the room's presence (it redraws
   the carets, the colour bars and the presence strip), `controller.refresh()` after a co-editor's update,
   `controller.set_layout_width(...)` from the text column, `controller.set_field_specs(...)` from the registry,
   `controller.set_web_base(...)` for web-only blocks, and `controller.set_comments(...)` from the comments API. After a
   mention search returns, call `controller.set_mention_results(...)` and then `binding.refresh()`.
4. Set `EditorBridge.compact` for phones and `EditorBridge.keyboard-height` while the on-screen keyboard is shown. From
   Rust: `window.global::<EditorBridge>().set_compact(true)`.

Rows are updated in place, and only rows whose view changed are written back, so the focused text box keeps its focus
while the document changes around it, and typing touches only the edited row.

## Keyboard (design section 6)

Implemented in `EditorController::key`: Enter (split), Shift+Enter (left to the TextInput), Backspace and Delete (merge,
or outdent an empty list item), Tab and Shift+Tab, Up and Down at the first or last visual line (moves to the neighbour,
keeping the x position), Alt+Shift+Up and Down (move block), Ctrl+D, Ctrl+B, I, U, E, Shift+X, Ctrl+K (link), Ctrl+Alt+M
(comment), Ctrl+Z and Ctrl+Shift+Z or Ctrl+Y, Escape (block mode). `/`, `@` and `:` open the insert, mention and emoji
menus. Markdown shortcuts at the start of a paragraph: `#` to `#####` (section), `-` or `*` (bulleted list), `1.` (numbered
list), `>` (quote), three backticks (code), `---` (divider).

## Locales

`EditorDoc::locales()` lists the locales with text, and the switcher also offers `pt`, `es` and `pt-BR`. Switching
re-renders the rows in that locale. A row with no text in the locale shows the `en` text greyed and read-only; the first
character typed creates the locale's text (the `en` text is copied in first).

## Comments

Comments are app data. The controller keeps the page's `CommentView`s (set by the app), shows the open count as a badge
on each block, and builds the panel's threads for the page or the focused block. Submit, resolve and reopen are
`ControllerEffect::Comment { request }` for the app to send. "Show" focuses the anchored block.

## Native web views

Web-only blocks (live preview, API documentation) render in a native web view that only the app can create. The editor
does not own one. It reports each web-only block's box and URL as `HostEvent::WebGeometry`, and the app implements
`WebViewHost` (`show`, `hide`, `hide_all`). Call `hide_all` while a menu is open (a child window draws over Slint), and
show the block's snapshot in its place (`WebBlock.show-snapshot`). `embed_url(base, block_json)` builds the embed route
`{base}/embed/block#<base64url of the block JSON>`.

## Open

- Nothing here has been compiled. The first build is the check. Every API this crate calls that was written from memory
  is marked `// UNSURE:` in the source: parley 0.11 (`ranged_builder`, `break_all_lines`, `align`, `Cursor::from_point`,
  `Cursor::index`, `Layout::len`, `Layout::get`, `Line::metrics`), Slint 1.18 (`Timer`, `PointerEvent.button`,
  `PointerEventButton`, `ListView` row heights, `Inspector`'s `width` override at the instance, `Math.round`), and
  `ModelRc::new(Rc<VecModel<_>>)`, `StyledText::from_markdown`.
- The link prompt's open-in-new-tab switch is shown but not stored: `rimelight_content` link spans carry only a URL.
- A section's `#` count does not set its depth yet; every `#` marker makes a section at the current nesting.
- The insert menu maps icon keys to icons for four keys only (`minus`, `chevron-right`, `search`, else a document icon).
  The registry's other icons need entries in the icon set and a case in `insert-menu.slint`.
- The block menu and the emoji and mention pickers open at a fixed place (under the header). Anchoring them to the caret's
  row needs the row's geometry from Slint.
- The phone virtual keyboard is an app-set `keyboard-height`. Slint 1.18's `virtual-keyboard-position` and
  `virtual-keyboard-size` are not used (not verified to exist on `Window`).
- The drag drop target is estimated from a fixed row pitch (`EditorBridge.drag-pitch`), not from each row's geometry.
- The global's properties are `in-out` (not `in`), so Slint's own markup can write them too. UNSURE: whether a global
  property declared `in` would also give Rust a setter in Slint 1.18; `in-out` is used so the setters exist.
- Comments on deleted blocks are not shown as "orphaned" yet; the panel only lists comments on the scope's block.
- The emoji table is a fixed list of about 200 common emoji; the full set is a follow-up.
- Each caret and line measurement builds a parley layout. Layout contexts are cached per thread, but a page with many
  remote carets still lays out each caret's text on every refresh.
