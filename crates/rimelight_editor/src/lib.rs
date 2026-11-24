//! Rimelight's Slint block editor: the editor markup (`ui/`), the plain-Rust controller that turns user actions into
//! document edits, the view and layout rules, and the `wire_block_editor!` macro an app expands to connect them.
//!
//! The design is `rimelight-space/docs/cms-editor-design.md` (sections 2, 3, 6, 7, 8, 9 and 10). The document model it
//! drives is `rimelight_content::editor` (`crates/rimelight_content/EDITOR_API.md`).
//!
//! Layering (AGENTS.md). This is a library crate: it ships `.slint` markup and [`UI_DIR`], and it never compiles Slint.
//! Nothing here names a generated Slint type. The app's `build.rs` registers [`UI_DIR`] as the library path
//! `rimelight_editor`, and the app's own markup imports `@rimelight_editor/block-editor.slint`. The Rust glue is
//! [`wire_block_editor!`], which the app expands in its own crate, next to its generated window type.

pub mod controller;
pub mod emoji;
pub mod layout;
pub mod view;
pub mod webview;

pub use controller::{
    BlockMenuChoice, CommentPanelView, CommentRequest, CommentRow, CommentView, ControllerEffect,
    EditorController, EmojiChoice, FieldKind, FieldSpec, InsertChoice, InspectorFieldView, Key,
    KeyInput, KeyResult, LinkPromptView, LocaleChoice, MentionChoice, Mods, PresenceView,
    RemoteCaretView, insert_catalogue,
};
pub use rimelight_content::sync::RemoteCursor;
pub use view::{RowView, font_size, to_markdown};
pub use webview::{WebViewHost, embed_url};

/// Absolute path of the editor's `.slint` markup folder, for an app's `build.rs` to register as the library path
/// `rimelight_editor`.
pub const UI_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/ui");

/// What the editor tells its host beyond the window update: every controller effect (sync, mention search, comment and
/// locale changes, errors), and the box and URL of a web-only block that needs a native web view.
#[derive(Clone, Debug, PartialEq)]
pub enum HostEvent {
    Effect(ControllerEffect),
    /// A web-only block reports its absolute box in logical pixels. The host calls [`WebViewHost::show`] with it.
    WebGeometry {
        key: String,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        url: String,
    },
}

/// Parses `#rrggbb` (or `rrggbb`) into its channels.
pub fn hex_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let digits = hex.strip_prefix('#').unwrap_or(hex);
    if digits.len() != 6 || !digits.is_ascii() {
        return None;
    }
    let channel = |at: usize| u8::from_str_radix(&digits[at..at + 2], 16).ok();
    Some((channel(0)?, channel(2)?, channel(4)?))
}

/// Expands, in the app crate, the glue between [`EditorController`] and the `EditorBridge` global that
/// `block-editor.slint` declares. The editor's state lives in the global, so an app places `BlockEditor {}` anywhere in
/// its window with no bindings, and this glue reaches it through the window.
///
/// Arguments, in order:
/// - `$window`: the app's top-level window type (`export component AppShell inherits Window { ... }`).
/// - `$bridge`: the generated `EditorBridge` global, as the app's root markup re-exports it (`export { EditorBridge }`).
/// - `$row`: `EditorRow`. `$caret`: `RemoteCaret`. `$insert`: `InsertItem`. `$mention`: `MentionItem`.
/// - `$field`: `InspectorField`. `$outcome`: `KeyOutcome`. `$emoji`: `EmojiItem`. `$block_menu`: `BlockMenuItem`.
/// - `$locale`: `LocaleItem`. `$comment`: `CommentItem`. `$presence`: `PresenceItem`.
///
/// Expansion defines `BlockEditorBinding` (returned by `bind_block_editor`), `bind_block_editor(window, controller,
/// on_host)`, and `BlockEditorBinding::{refresh, set_remote_cursors, swap_controller}`. The window keeps its own
/// properties and callbacks; this wires every callback of `EditorBridge` to the controller, and pushes the controller's
/// state back into the global after each one. Only rows whose view changed are written back, so typing updates the
/// edited row alone.
///
/// One editor is live per window at a time: the bridge is one global per window, so a second `bind_block_editor` on
/// the same window would take over its callbacks. An app with several open pages binds once per window and calls
/// `swap_controller` when the active page changes.
///
/// UNSURE, to confirm at the first build: `ModelRc::new(Rc<VecModel<_>>)` (a shared model handle),
/// `StyledText::from_markdown` returning a `Result`, `Weak<Window>` from `window.as_weak()`, and
/// `ComponentHandle::global::<G>()` on the app's window type.
#[macro_export]
macro_rules! wire_block_editor {
    ($window:path, $bridge:path, $row:path, $caret:path, $insert:path, $mention:path, $field:path, $outcome:path,
     $emoji:path, $block_menu:path, $locale:path, $comment:path, $presence:path $(,)?) => {
        #[allow(unused_imports)]
        use ::slint::ComponentHandle as _;

        /// The state the editor is bound to: the controller the bridge currently drives, the rows model the bridge
        /// draws, the views last written to it, and the host's hook.
        pub struct BlockEditorBinding {
            controller:
                ::std::cell::RefCell<::std::rc::Rc<::std::cell::RefCell<$crate::EditorController>>>,
            rows: ::std::rc::Rc<::slint::VecModel<$row>>,
            shown: ::std::cell::RefCell<::std::vec::Vec<$crate::RowView>>,
            window: ::slint::Weak<$window>,
            on_host: ::std::boxed::Box<dyn ::std::ops::Fn($crate::HostEvent)>,
        }

        fn colour_of(hex: &str) -> ::slint::Color {
            $crate::hex_rgb(hex).map_or_else(::slint::Color::default, |(r, g, b)| {
                ::slint::Color::from_rgb_u8(r, g, b)
            })
        }

        fn editor_row(view: &$crate::RowView) -> $row {
            let (presence, colour, name) = match &view.presence {
                Some((hex, name)) => (
                    true,
                    colour_of(hex),
                    ::slint::SharedString::from(name.as_str()),
                ),
                None => (
                    false,
                    ::slint::Color::default(),
                    ::slint::SharedString::new(),
                ),
            };
            $row {
                id: ::slint::SharedString::from(view.id.as_str()),
                block_type: ::slint::SharedString::from(view.block_type.as_str()),
                depth: view.depth as i32,
                index: view.index as i32,
                text: ::slint::SharedString::from(view.text.as_str()),
                markdown: ::slint::StyledText::from_markdown(&view.markdown)
                    .unwrap_or_else(|_| ::slint::StyledText::from_plain_text(&view.markdown)),
                inline: view.inline,
                font_size: view.font_size,
                heading_level: view.heading_level.map_or(0, i32::from),
                locked: view.locked,
                restricted: view.restricted,
                web_only: view.web_only,
                web_url: ::slint::SharedString::from(view.web_url.as_str()),
                placeholder: ::slint::SharedString::from(view.placeholder.as_str()),
                selected: view.selected,
                presence,
                presence_colour: colour,
                presence_name: name,
                fallback: view.fallback,
                comment_count: view.comment_count as i32,
            }
        }

        fn caret_row(caret: &$crate::RemoteCaretView) -> $caret {
            $caret {
                row: caret.row as i32,
                x: caret.x,
                y: caret.y,
                height: caret.height,
                colour: colour_of(&caret.colour),
                name: ::slint::SharedString::from(caret.name.as_str()),
            }
        }

        fn insert_row(choice: &$crate::InsertChoice) -> $insert {
            $insert {
                block_type: ::slint::SharedString::from(choice.block_type.as_str()),
                label: ::slint::SharedString::from(choice.label.as_str()),
                icon_key: ::slint::SharedString::from(choice.icon_key.as_str()),
                category: ::slint::SharedString::from(choice.category.as_str()),
            }
        }

        fn mention_row(choice: &$crate::MentionChoice) -> $mention {
            $mention {
                entry_id: ::slint::SharedString::from(choice.entry_id.as_str()),
                collection: ::slint::SharedString::from(choice.collection.as_str()),
                slug: ::slint::SharedString::from(choice.slug.as_str()),
                title: ::slint::SharedString::from(choice.title.as_str()),
            }
        }

        fn emoji_row(choice: &$crate::EmojiChoice) -> $emoji {
            $emoji {
                emoji: ::slint::SharedString::from(choice.emoji.as_str()),
                shortcode: ::slint::SharedString::from(choice.shortcode.as_str()),
            }
        }

        fn block_menu_row(choice: &$crate::BlockMenuChoice) -> $block_menu {
            $block_menu {
                action: ::slint::SharedString::from(choice.action.as_str()),
                label: ::slint::SharedString::from(choice.label.as_str()),
            }
        }

        fn locale_row(choice: &$crate::LocaleChoice) -> $locale {
            $locale {
                code: ::slint::SharedString::from(choice.code.as_str()),
                label: ::slint::SharedString::from(choice.label.as_str()),
                present: choice.present,
                active: choice.active,
            }
        }

        fn comment_row(row: &$crate::CommentRow) -> $comment {
            $comment {
                id: ::slint::SharedString::from(row.id.as_str()),
                block_id: ::slint::SharedString::from(row.block_id.as_str()),
                depth: row.depth as i32,
                author: ::slint::SharedString::from(row.author.as_str()),
                body: ::slint::SharedString::from(row.body.as_str()),
                created: ::slint::SharedString::from(row.created.as_str()),
                resolved: row.resolved,
            }
        }

        fn presence_row(view: &$crate::PresenceView) -> $presence {
            $presence {
                name: ::slint::SharedString::from(view.name.as_str()),
                initial: ::slint::SharedString::from(view.initial.as_str()),
                colour: colour_of(&view.colour),
            }
        }

        fn field_row(field: &$crate::InspectorFieldView) -> $field {
            $field {
                key: ::slint::SharedString::from(field.key.as_str()),
                label: ::slint::SharedString::from(field.label.as_str()),
                kind: ::slint::SharedString::from(field.kind),
                value: ::slint::SharedString::from(field.value.as_str()),
                options: ::slint::ModelRc::new(::slint::VecModel::from(
                    field
                        .options
                        .iter()
                        .map(|o| ::slint::SharedString::from(o.as_str()))
                        .collect::<::std::vec::Vec<_>>(),
                )),
                read_only: field.read_only,
            }
        }

        /// A `usize` row index from the bridge's `int`, clamped at zero.
        fn row_index(row: i32) -> usize {
            row.max(0) as usize
        }

        fn model<T: ::std::clone::Clone + 'static>(
            items: ::std::vec::Vec<T>,
        ) -> ::slint::ModelRc<T> {
            ::slint::ModelRc::new(::slint::VecModel::from(items))
        }

        impl BlockEditorBinding {
            /// The controller the bridge currently drives.
            fn current(&self) -> ::std::rc::Rc<::std::cell::RefCell<$crate::EditorController>> {
                self.controller.borrow().clone()
            }

            /// Points the bridge at another controller, for an app whose active page changed. Redraws the window's
            /// editor from it. The rows model is reused: only rows whose view differs are written.
            pub fn swap_controller(
                &self,
                controller: ::std::rc::Rc<::std::cell::RefCell<$crate::EditorController>>,
            ) {
                *self.controller.borrow_mut() = controller;
                self.refresh();
            }

            /// Pushes the controller's state to the window's `EditorBridge`: rows (only those that changed), carets,
            /// menus, panels, the locale switcher, the presence strip and the inspector.
            pub fn refresh(&self) {
                use ::slint::Model as _;
                let Some(window) = self.window.upgrade() else {
                    return;
                };
                let bridge = window.global::<$bridge>();
                let current = self.current();
                let controller = current.borrow();
                let views = controller.row_views();
                {
                    let shown = self.shown.borrow();
                    for (at, view) in views.iter().enumerate() {
                        if shown.get(at) == Some(view) {
                            continue;
                        }
                        let row = editor_row(view);
                        if at < self.rows.row_count() {
                            self.rows.set_row_data(at, row);
                        } else {
                            self.rows.push(row);
                        }
                    }
                }
                while self.rows.row_count() > views.len() {
                    let last = self.rows.row_count() - 1;
                    self.rows.remove(last);
                }
                *self.shown.borrow_mut() = views;

                bridge.set_carets(model(
                    controller.caret_views().iter().map(caret_row).collect(),
                ));
                bridge.set_locales(model(
                    controller.locale_choices().iter().map(locale_row).collect(),
                ));
                bridge.set_presence(model(
                    controller
                        .presence_views()
                        .iter()
                        .map(presence_row)
                        .collect(),
                ));

                bridge.set_insert_open(controller.insert_menu_open());
                bridge.set_insert_items(model(
                    controller
                        .insert_menu_items()
                        .iter()
                        .map(insert_row)
                        .collect(),
                ));
                bridge.set_insert_selected(controller.insert_menu_selected() as i32);

                bridge.set_mention_open(controller.mention_menu_open());
                bridge.set_mention_items(model(
                    controller
                        .mention_menu_items()
                        .iter()
                        .map(mention_row)
                        .collect(),
                ));
                bridge.set_mention_selected(controller.mention_menu_selected() as i32);

                bridge.set_emoji_open(controller.emoji_menu_open());
                bridge.set_emoji_items(model(
                    controller
                        .emoji_menu_items()
                        .iter()
                        .map(emoji_row)
                        .collect(),
                ));
                bridge.set_emoji_selected(controller.emoji_menu_selected() as i32);

                bridge.set_block_menu_open(controller.block_menu_open());
                bridge.set_block_menu_items(model(
                    controller
                        .block_menu_items()
                        .iter()
                        .map(block_menu_row)
                        .collect(),
                ));
                bridge.set_block_menu_selected(controller.block_menu_selected() as i32);

                match controller.link_prompt() {
                    Some(link) => {
                        bridge.set_link_open(true);
                        bridge.set_link_url(::slint::SharedString::from(link.url.as_str()));
                        bridge.set_link_new_tab(link.new_tab);
                        bridge.set_link_has_link(link.has_link);
                    }
                    None => bridge.set_link_open(false),
                }

                match controller.comment_panel() {
                    Some(panel) => {
                        bridge.set_comments_open(true);
                        bridge.set_comment_scope(::slint::SharedString::from(panel.scope.as_str()));
                        bridge.set_comment_block(::slint::SharedString::from(panel.block.as_str()));
                        bridge.set_comments(model(panel.rows.iter().map(comment_row).collect()));
                    }
                    None => bridge.set_comments_open(false),
                }

                bridge.set_inspector_fields(model(
                    controller
                        .inspector_fields()
                        .iter()
                        .map(field_row)
                        .collect(),
                ));
            }

            /// Updates the window from a list of effects, then tells the host about each one.
            fn apply(&self, effects: ::std::vec::Vec<$crate::ControllerEffect>) {
                self.refresh();
                if let Some(window) = self.window.upgrade() {
                    let bridge = window.global::<$bridge>();
                    for effect in &effects {
                        if let $crate::ControllerEffect::Focus { row, anchor, head } = effect {
                            bridge.set_focus_row(*row as i32);
                            bridge.set_focus_anchor(*anchor as i32);
                            bridge.set_focus_head(*head as i32);
                            bridge.set_focus_serial(bridge.get_focus_serial() + 1);
                        }
                    }
                }
                for effect in effects {
                    (self.on_host)($crate::HostEvent::Effect(effect));
                }
            }

            /// The co-editors' cursors, from the room's presence (`SyncSession::remote_cursors`). Redraws the carets,
            /// the colour bars and the presence strip.
            pub fn set_remote_cursors(&self, cursors: ::std::vec::Vec<$crate::RemoteCursor>) {
                let _ = self.current().borrow_mut().set_remote_cursors(cursors);
                self.refresh();
            }
        }

        /// Wires the window's `EditorBridge` to `controller`. `on_host` receives every effect the controller returns,
        /// and each web-only block's geometry. It is called after the window has been updated. Keep the returned
        /// binding alive as long as the window is shown. One editor is live per window: a second bind on the same
        /// window takes over its callbacks.
        pub fn bind_block_editor(
            window: &$window,
            controller: ::std::rc::Rc<::std::cell::RefCell<$crate::EditorController>>,
            on_host: impl ::std::ops::Fn($crate::HostEvent) + 'static,
        ) -> ::std::rc::Rc<BlockEditorBinding> {
            let binding = ::std::rc::Rc::new(BlockEditorBinding {
                controller: ::std::cell::RefCell::new(controller),
                rows: ::std::rc::Rc::new(::slint::VecModel::default()),
                shown: ::std::cell::RefCell::new(::std::vec::Vec::new()),
                window: window.as_weak(),
                on_host: ::std::boxed::Box::new(on_host),
            });
            let bridge = window.global::<$bridge>();
            bridge.set_rows(::slint::ModelRc::new(binding.rows.clone()));

            let b = binding.clone();
            bridge.on_text_edited(move |row, text| {
                let effects = b
                    .current()
                    .borrow_mut()
                    .text_edited(row_index(row), text.as_str());
                b.apply(effects);
            });

            let b = binding.clone();
            bridge.on_key(move |row, event| {
                let mods = $crate::Mods {
                    control: event.modifiers.control,
                    shift: event.modifiers.shift,
                    alt: event.modifiers.alt,
                    meta: event.modifiers.meta,
                };
                let input = $crate::KeyInput::from_slint(event.text.as_str(), mods);
                let result = b.current().borrow_mut().key(row_index(row), input);
                let outcome = if result.handled {
                    <$outcome>::Handled
                } else {
                    <$outcome>::Ignored
                };
                b.apply(result.effects);
                outcome
            });

            let b = binding.clone();
            bridge.on_caret_moved(move |row, anchor, head| {
                let effects = b.current().borrow_mut().caret_moved(
                    row_index(row),
                    anchor.max(0) as usize,
                    head.max(0) as usize,
                );
                b.apply(effects);
            });

            let b = binding.clone();
            bridge.on_handle_clicked(move |row| {
                let effects = b.current().borrow_mut().handle_clicked(row_index(row));
                b.apply(effects);
            });

            let b = binding.clone();
            bridge.on_long_pressed(move |row| {
                let effects = b.current().borrow_mut().long_pressed(row_index(row));
                b.apply(effects);
            });

            let b = binding.clone();
            bridge.on_drag_dropped(move |from, to, depth| {
                let effects = b.current().borrow_mut().drag_dropped(
                    row_index(from),
                    row_index(to),
                    row_index(depth),
                );
                b.apply(effects);
            });

            let b = binding.clone();
            bridge.on_insert_menu_chosen(move |block_type| {
                let effects = b
                    .current()
                    .borrow_mut()
                    .insert_menu_chosen(block_type.as_str());
                b.apply(effects);
            });

            let b = binding.clone();
            bridge.on_insert_menu_dismissed(move || {
                let effects = b.current().borrow_mut().insert_menu_dismissed();
                b.apply(effects);
            });

            let b = binding.clone();
            bridge.on_mention_query(move |text| {
                let effects = b.current().borrow_mut().mention_query(text.as_str());
                b.apply(effects);
            });

            let b = binding.clone();
            bridge.on_mention_chosen(move |index| {
                let effects = b.current().borrow_mut().mention_chosen(row_index(index));
                b.apply(effects);
            });

            let b = binding.clone();
            bridge.on_mention_dismissed(move || {
                let effects = b.current().borrow_mut().mention_dismissed();
                b.apply(effects);
            });

            let b = binding.clone();
            bridge.on_emoji_chosen(move |index| {
                let effects = b.current().borrow_mut().emoji_chosen(row_index(index));
                b.apply(effects);
            });

            let b = binding.clone();
            bridge.on_emoji_dismissed(move || {
                let effects = b.current().borrow_mut().emoji_dismissed();
                b.apply(effects);
            });

            let b = binding.clone();
            bridge.on_block_menu_chosen(move |action| {
                let effects = b.current().borrow_mut().block_menu_chosen(action.as_str());
                b.apply(effects);
            });

            let b = binding.clone();
            bridge.on_block_menu_dismissed(move || {
                let effects = b.current().borrow_mut().block_menu_dismissed();
                b.apply(effects);
            });

            let b = binding.clone();
            bridge.on_toolbar(move |action| {
                let effects = b.current().borrow_mut().toolbar(action.as_str());
                b.apply(effects);
            });

            let b = binding.clone();
            bridge.on_link_submitted(move |url, new_tab| {
                let effects = b.current().borrow_mut().link_submit(url.as_str(), new_tab);
                b.apply(effects);
            });

            let b = binding.clone();
            bridge.on_link_removed(move || {
                let effects = b.current().borrow_mut().link_remove();
                b.apply(effects);
            });

            let b = binding.clone();
            bridge.on_link_dismissed(move || {
                let effects = b.current().borrow_mut().link_dismissed();
                b.apply(effects);
            });

            let b = binding.clone();
            bridge.on_locale_chosen(move |code| {
                let effects = b.current().borrow_mut().locale_chosen(code.as_str());
                b.apply(effects);
            });

            let b = binding.clone();
            bridge.on_comment_submit(move |block_id, parent_id, body| {
                let effects = b.current().borrow_mut().comment_submit(
                    block_id.as_str(),
                    parent_id.as_str(),
                    body.as_str(),
                );
                b.apply(effects);
            });

            let b = binding.clone();
            bridge.on_comment_resolve(move |id| {
                let effects = b.current().borrow_mut().comment_resolve(id.as_str());
                b.apply(effects);
            });

            let b = binding.clone();
            bridge.on_comment_reopen(move |id| {
                let effects = b.current().borrow_mut().comment_reopen(id.as_str());
                b.apply(effects);
            });

            let b = binding.clone();
            bridge.on_comment_jump(move |block_id| {
                let effects = b.current().borrow_mut().comment_jump(block_id.as_str());
                b.apply(effects);
            });

            let b = binding.clone();
            bridge.on_comment_scope_changed(move |scope| {
                let effects = b
                    .current()
                    .borrow_mut()
                    .comment_scope_changed(scope.as_str());
                b.apply(effects);
            });

            let b = binding.clone();
            bridge.on_comment_closed(move || {
                let effects = b.current().borrow_mut().comment_closed();
                b.apply(effects);
            });

            let b = binding.clone();
            bridge.on_inspector_changed(move |key, value| {
                let effects = b
                    .current()
                    .borrow_mut()
                    .inspector_changed(key.as_str(), value.as_str());
                b.apply(effects);
            });

            let b = binding.clone();
            bridge.on_web_block_geometry(move |key, x, y, w, h, url| {
                (b.on_host)($crate::HostEvent::WebGeometry {
                    key: key.to_string(),
                    x,
                    y,
                    w,
                    h,
                    url: url.to_string(),
                });
            });

            binding.refresh();
            binding
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_colours_parse_with_or_without_the_hash() {
        assert_eq!(hex_rgb("#3b82f6"), Some((0x3b, 0x82, 0xf6)));
        assert_eq!(hex_rgb("3b82f6"), Some((0x3b, 0x82, 0xf6)));
        assert_eq!(hex_rgb("#fff"), None);
        assert_eq!(hex_rgb("#zzzzzz"), None);
    }
}
