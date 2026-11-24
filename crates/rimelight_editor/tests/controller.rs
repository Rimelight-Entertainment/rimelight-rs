//! Controller rules on small pages: the key map, the markdown shortcuts (with the registry's block names), the insert
//! menu filtered by the registry, the emoji picker, the link prompt, the locale fallback, Up and Down across blocks,
//! comments, the block menu, markdown escaping and the embed URL. The pages are built through the editor API, so the
//! tests do not depend on the import format.

use loro::LoroDoc;
use rimelight_content::editor::{EditorDoc, MarkKind, Mention, Span, TextValue};
use rimelight_editor::{
    CommentRequest, CommentView, ControllerEffect, EditorController, Key, KeyInput, Mods,
    embed_url, to_markdown,
};
use serde_json::json;

/// A page with one paragraph per entry of `paragraphs` (an empty string leaves the paragraph empty).
fn page(paragraphs: &[&str]) -> EditorController {
    let mut doc = EditorDoc::from_doc(LoroDoc::new());
    let mut after: Option<String> = None;
    for text in paragraphs {
        let id = doc
            .insert_block(None, after.as_deref(), "ParagraphBlock")
            .expect("insert paragraph");
        if !text.is_empty() {
            doc.replace_text(&id, "text", "en", text)
                .expect("fill paragraph");
        }
        after = Some(id);
    }
    EditorController::new(doc, "en")
}

fn key(name: Key) -> KeyInput {
    KeyInput {
        key: name,
        mods: Mods::default(),
    }
}

fn with(name: Key, mods: Mods) -> KeyInput {
    KeyInput { key: name, mods }
}

fn ctrl() -> Mods {
    Mods {
        control: true,
        ..Mods::default()
    }
}

fn ctrl_alt() -> Mods {
    Mods {
        control: true,
        alt: true,
        ..Mods::default()
    }
}

fn text_in(c: &EditorController, row: usize, locale: &str) -> String {
    let id = c.rows()[row].id.clone();
    c.doc().text(&id, "text", locale).plain
}

fn focus_row(effects: &[ControllerEffect]) -> Option<usize> {
    effects.iter().find_map(|e| match e {
        ControllerEffect::Focus { row, .. } => Some(*row),
        _ => None,
    })
}

/// Types `typed` into row `row` as the first key-by-key edit would: the caret moves to the end, then the text changes.
fn type_into(c: &mut EditorController, row: usize, typed: &str) {
    let end = typed.len();
    c.caret_moved(row, end, end);
    c.text_edited(row, typed);
}

#[test]
fn enter_splits_the_block_at_the_caret() {
    let mut c = page(&["hello world"]);
    c.caret_moved(0, 5, 5);
    let result = c.key(0, key(Key::Enter));
    assert!(result.handled);
    assert_eq!(c.rows().len(), 2);
    assert_eq!(text_in(&c, 0, "en"), "hello");
    assert_eq!(text_in(&c, 1, "en"), " world");
    assert_eq!(focus_row(&result.effects), Some(1));
}

#[test]
fn shift_enter_is_left_to_the_text_input() {
    let mut c = page(&["hello"]);
    c.caret_moved(0, 2, 2);
    let shift = Mods {
        shift: true,
        ..Mods::default()
    };
    let result = c.key(0, with(Key::Enter, shift));
    assert!(!result.handled);
    assert_eq!(c.rows().len(), 1);
}

#[test]
fn backspace_at_the_start_merges_into_the_previous_block() {
    let mut c = page(&["one", "two"]);
    c.caret_moved(1, 0, 0);
    let result = c.key(1, key(Key::Backspace));
    assert!(result.handled);
    assert_eq!(c.rows().len(), 1);
    assert_eq!(text_in(&c, 0, "en"), "onetwo");
}

#[test]
fn backspace_inside_text_is_left_to_the_text_input() {
    let mut c = page(&["one"]);
    c.caret_moved(0, 2, 2);
    assert!(!c.key(0, key(Key::Backspace)).handled);
}

#[test]
fn tab_indents_and_shift_tab_outdents_a_list_item() {
    // Paragraphs do not nest (registry); Tab indents list items, sections and steps (design section 6).
    let mut doc = EditorDoc::from_doc(LoroDoc::new());
    let list = doc
        .insert_block(None, None, "UnorderedListBlock")
        .expect("list");
    let first = doc.rows("en")[1].id.clone();
    let second = doc
        .insert_block(Some(&list), Some(&first), "ListItemBlock")
        .expect("second item");
    let mut c = EditorController::new(doc, "en");
    let depth_of =
        |c: &EditorController, id: &str| c.rows().iter().find(|r| r.id == id).expect("row").depth;
    let row_of =
        |c: &EditorController, id: &str| c.rows().iter().position(|r| r.id == id).expect("row");
    let before = depth_of(&c, &second);
    let row = row_of(&c, &second);
    c.caret_moved(row, 0, 0);
    assert!(c.key(row, key(Key::Tab)).handled);
    assert!(
        depth_of(&c, &second) > before,
        "Tab nests the item under the one above"
    );
    let shift = Mods {
        shift: true,
        ..Mods::default()
    };
    let row = row_of(&c, &second);
    c.caret_moved(row, 0, 0);
    assert!(c.key(row, with(Key::Tab, shift)).handled);
    assert_eq!(depth_of(&c, &second), before);
}

#[test]
fn ctrl_b_bolds_the_selection() {
    let mut c = page(&["hello"]);
    c.caret_moved(0, 0, 5);
    assert!(
        c.key(
            0,
            KeyInput {
                key: Key::Char('b'),
                mods: ctrl()
            }
        )
        .handled
    );
    let id = c.rows()[0].id.clone();
    let value = c.doc().text(&id, "text", "en");
    assert!(
        value
            .spans
            .iter()
            .any(|s| s.marks.contains(&MarkKind::Bold))
    );
    assert_eq!(to_markdown(&value), "**hello**");
}

#[test]
fn up_on_the_first_line_moves_to_the_previous_block() {
    let mut c = page(&["alpha", "beta"]);
    c.caret_moved(1, 2, 2);
    let result = c.key(1, key(Key::Up));
    assert!(result.handled);
    // The column is kept: the caret lands near x = 2 of "alpha" (exact offset depends on the glyph advances).
    assert_eq!(focus_row(&result.effects), Some(0));
}

#[test]
fn down_on_the_last_line_moves_to_the_next_block() {
    let mut c = page(&["alpha", "beta"]);
    c.caret_moved(0, 5, 5);
    let result = c.key(0, key(Key::Down));
    assert!(result.handled);
    assert_eq!(focus_row(&result.effects), Some(1));
}

#[test]
fn up_inside_a_multi_line_block_is_left_to_the_text_input() {
    let mut c = page(&["line one\nline two"]);
    c.caret_moved(0, 12, 12);
    assert!(!c.key(0, key(Key::Up)).handled);
}

#[test]
fn slash_opens_the_insert_menu_and_enter_inserts_the_first_match() {
    let mut c = page(&[""]);
    c.caret_moved(0, 1, 1);
    c.text_edited(0, "/");
    assert!(c.insert_menu_open());
    assert!(c.insert_menu_items().len() > 2);

    type_into(&mut c, 0, "/hr");
    let result = c.key(0, key(Key::Enter));
    assert!(result.handled);
    // The empty paragraph held only "/hr", so it turns into the chosen block.
    assert_eq!(c.rows()[0].block_type, "DividerBlock");
    assert!(!c.insert_menu_open());
}

#[test]
fn the_insert_menu_filters_by_label_and_keywords() {
    let mut c = page(&[""]);
    c.caret_moved(0, 1, 1);
    c.text_edited(0, "/");
    // "hr" is a keyword of Divider only.
    type_into(&mut c, 0, "/hr");
    let labels: Vec<String> = c.insert_menu_items().into_iter().map(|i| i.label).collect();
    assert_eq!(labels, vec!["Divider".to_string()]);

    type_into(&mut c, 0, "/bullet");
    let labels: Vec<String> = c.insert_menu_items().into_iter().map(|i| i.label).collect();
    assert_eq!(labels, vec!["Bulleted list".to_string()]);
}

#[test]
fn the_root_menu_offers_web_blocks_and_not_nested_only_blocks() {
    let mut c = page(&[""]);
    c.caret_moved(0, 1, 1);
    c.text_edited(0, "/");
    let types: Vec<String> = c
        .insert_menu_items()
        .into_iter()
        .map(|i| i.block_type)
        .collect();
    assert!(types.contains(&"LivePreviewBlock".to_string()));
    assert!(types.contains(&"ApiDocumentationBlock".to_string()));
    assert!(types.contains(&"ParagraphBlock".to_string()));
    for nested in [
        "ListItemBlock",
        "TabItemBlock",
        "StepItemBlock",
        "CardBlock",
        "SceneBlock",
        "DialogueBlock",
    ] {
        assert!(
            !types.contains(&nested.to_string()),
            "{nested} must not be offered at the root"
        );
    }
}

#[test]
fn inside_a_callout_the_menu_offers_only_what_the_callout_allows() {
    let mut doc = EditorDoc::from_doc(LoroDoc::new());
    let callout = doc
        .insert_block(None, None, "CalloutBlock")
        .expect("callout");
    // The callout starts with one paragraph (its initial child); this is the second.
    doc.insert_block(Some(callout.as_str()), None, "ParagraphBlock")
        .expect("child");
    let mut c = EditorController::new(doc, "en");
    // Row 1 is the callout's child paragraph (row 0 is the callout itself).
    c.caret_moved(1, 0, 0);
    c.text_edited(1, "/");
    let types: Vec<String> = c
        .insert_menu_items()
        .into_iter()
        .map(|i| i.block_type)
        .collect();
    assert!(
        types.contains(&"ParagraphBlock".to_string()),
        "offered: {types:?}"
    );
    assert!(types.contains(&"CodeBlock".to_string()));
    assert!(types.contains(&"UnorderedListBlock".to_string()));
    assert!(!types.contains(&"DividerBlock".to_string()));
    assert!(!types.contains(&"SectionBlock".to_string()));
}

#[test]
fn a_space_after_the_slash_closes_the_insert_menu() {
    let mut c = page(&[""]);
    c.caret_moved(0, 1, 1);
    c.text_edited(0, "/");
    c.caret_moved(0, 2, 2);
    c.text_edited(0, "/ ");
    assert!(!c.insert_menu_open());
}

#[test]
fn hash_space_turns_an_empty_paragraph_into_a_section() {
    let mut c = page(&[""]);
    c.caret_moved(0, 0, 0);
    c.text_edited(0, "# ");
    assert_eq!(c.rows()[0].block_type, "SectionBlock");
}

#[test]
fn any_hash_marker_makes_a_section() {
    // The hash count does not set the depth yet: a section's heading level comes from its nesting (see README).
    let mut c = page(&[""]);
    c.caret_moved(0, 0, 0);
    c.text_edited(0, "### ");
    assert_eq!(c.rows()[0].block_type, "SectionBlock");
    assert_eq!(c.rows()[0].heading_level, Some(2));
}

#[test]
fn each_markdown_shortcut_uses_its_registry_block() {
    let cases: [(&str, &str); 6] = [
        ("> ", "QuoteBlock"),
        ("```", "CodeBlock"),
        ("---", "DividerBlock"),
        ("* ", "UnorderedListBlock"),
        ("1. ", "OrderedListBlock"),
        ("## ", "SectionBlock"),
    ];
    for (marker, expected) in cases {
        let mut c = page(&[""]);
        c.caret_moved(0, 0, 0);
        c.text_edited(0, marker);
        let types: Vec<&str> = c.rows().iter().map(|r| r.block_type.as_str()).collect();
        assert!(
            types.contains(&expected),
            "{marker:?} gave {types:?}, expected {expected}"
        );
    }
}

#[test]
fn dash_space_starts_a_list_with_a_list_item() {
    let mut c = page(&["one", ""]);
    c.caret_moved(1, 0, 0);
    c.text_edited(1, "- ");
    let rows = c.rows();
    assert!(rows.iter().any(|r| r.block_type == "UnorderedListBlock"));
    let item = rows
        .iter()
        .find(|r| r.block_type == "ListItemBlock")
        .expect("a list item");
    assert_eq!(item.depth, 1);
}

#[test]
fn a_code_fence_says_nothing_once_the_block_is_code() {
    let mut c = page(&[""]);
    c.caret_moved(0, 0, 0);
    c.text_edited(0, "```");
    // The paragraph became a code block. Shortcuts only run on paragraphs, so a fence typed in code stays text.
    assert_eq!(c.rows()[0].block_type, "CodeBlock");
    c.caret_moved(0, 3, 3);
    c.text_edited(0, "```x");
    assert_eq!(c.rows()[0].block_type, "CodeBlock");
}

#[test]
fn a_colon_opens_the_emoji_picker_and_enter_inserts_the_match() {
    let mut c = page(&[""]);
    c.caret_moved(0, 1, 1);
    c.text_edited(0, ":");
    assert!(c.emoji_menu_open());
    type_into(&mut c, 0, ":sm");
    let items = c.emoji_menu_items();
    assert_eq!(items.first().map(|e| e.shortcode.as_str()), Some("smile"));

    let result = c.key(0, key(Key::Enter));
    assert!(result.handled);
    assert!(!c.emoji_menu_open());
    assert_eq!(text_in(&c, 0, "en"), "😄");
}

#[test]
fn a_colon_in_the_middle_of_a_word_does_not_open_the_picker() {
    let mut c = page(&["12"]);
    c.caret_moved(0, 2, 2);
    c.text_edited(0, "12:");
    assert!(!c.emoji_menu_open());
}

#[test]
fn ctrl_k_links_the_selection_and_remove_clears_it() {
    let mut c = page(&["hello"]);
    c.caret_moved(0, 0, 5);
    assert!(
        c.key(
            0,
            KeyInput {
                key: Key::Char('k'),
                mods: ctrl()
            }
        )
        .handled
    );
    assert!(c.link_prompt().is_some());
    c.link_submit("https://example.test", false);
    assert!(c.link_prompt().is_none());

    let id = c.rows()[0].id.clone();
    let has_link = |c: &EditorController| {
        c.doc()
            .text(&id, "text", "en")
            .spans
            .iter()
            .any(|s| s.link.as_deref() == Some("https://example.test"))
    };
    assert!(has_link(&c));

    // Reopening on the linked text shows its URL; Remove link clears it.
    c.caret_moved(0, 0, 5);
    c.key(
        0,
        KeyInput {
            key: Key::Char('k'),
            mods: ctrl(),
        },
    );
    let prompt = c.link_prompt().expect("prompt");
    assert!(prompt.has_link);
    assert_eq!(prompt.url, "https://example.test");
    c.link_remove();
    assert!(!has_link(&c));
}

#[test]
fn ctrl_k_without_a_selection_says_so() {
    let mut c = page(&["hello"]);
    c.caret_moved(0, 2, 2);
    let result = c.key(
        0,
        KeyInput {
            key: Key::Char('k'),
            mods: ctrl(),
        },
    );
    assert!(
        result
            .effects
            .iter()
            .any(|e| matches!(e, ControllerEffect::Error(_)))
    );
    assert!(c.link_prompt().is_none());
}

#[test]
fn a_locale_without_text_shows_the_english_text_until_typed_in() {
    let mut c = page(&["hello"]);
    let offered = c.locale_choices();
    assert!(offered.iter().any(|l| l.code == "pt" && !l.present));
    assert!(
        offered
            .iter()
            .any(|l| l.code == "en" && l.present && l.active)
    );

    c.locale_chosen("pt");
    let view = c.row_views().remove(0);
    assert!(view.fallback);
    assert_eq!(view.text, "hello");

    // The first key creates the locale's text: the English copy, then the typed character.
    let result = c.key(
        0,
        KeyInput {
            key: Key::Char('x'),
            mods: Mods::default(),
        },
    );
    assert!(result.handled);
    assert_eq!(text_in(&c, 0, "pt"), "hellox");
    assert!(!c.row_views()[0].fallback);
    assert_eq!(text_in(&c, 0, "en"), "hello");
}

#[test]
fn enter_in_a_fallback_row_does_nothing() {
    let mut c = page(&["hello"]);
    c.locale_chosen("es");
    let result = c.key(0, key(Key::Enter));
    assert!(result.handled);
    assert_eq!(c.rows().len(), 1);
    assert_eq!(text_in(&c, 0, "es"), "");
}

#[test]
fn ctrl_alt_m_opens_the_comment_panel_for_the_block() {
    let mut c = page(&["note"]);
    c.caret_moved(0, 0, 0);
    let result = c.key(
        0,
        KeyInput {
            key: Key::Char('m'),
            mods: ctrl_alt(),
        },
    );
    assert!(result.handled);
    let id = c.rows()[0].id.clone();
    assert!(
        result
            .effects
            .contains(&ControllerEffect::CommentRequested {
                ids: vec![id.clone()]
            })
    );
    let panel = c.comment_panel().expect("panel");
    assert_eq!(panel.scope, id);
}

#[test]
fn comments_show_as_threads_with_a_badge_and_resolve_is_a_request() {
    let mut c = page(&["note"]);
    let id = c.rows()[0].id.clone();
    c.comment_scope_changed(&id);
    c.set_comments(vec![
        CommentView {
            id: "c1".into(),
            block_id: id.clone(),
            parent_id: None,
            author: "Ana".into(),
            body: "Check this".into(),
            created_at: "today".into(),
            resolved: false,
        },
        CommentView {
            id: "c2".into(),
            block_id: id.clone(),
            parent_id: Some("c1".into()),
            author: "Bo".into(),
            body: "Done".into(),
            created_at: "today".into(),
            resolved: false,
        },
    ]);
    assert_eq!(c.row_views()[0].comment_count, 1);
    let panel = c.comment_panel().expect("panel");
    assert_eq!(panel.rows.len(), 2);
    assert_eq!(panel.rows[1].depth, 1);

    let effects = c.comment_resolve("c1");
    assert_eq!(
        effects,
        vec![ControllerEffect::Comment {
            request: CommentRequest::Resolve { id: "c1".into() }
        }]
    );
    assert!(
        c.comment_submit(&id, "", "   ")
            .iter()
            .any(|e| matches!(e, ControllerEffect::Error(_)))
    );
    let sent = c.comment_submit(&id, "c1", " thanks ");
    assert_eq!(
        sent,
        vec![ControllerEffect::Comment {
            request: CommentRequest::Submit {
                block_id: id,
                parent_id: Some("c1".into()),
                body: "thanks".into(),
            }
        }]
    );
}

#[test]
fn the_block_menu_duplicates_and_deletes() {
    let mut c = page(&["one"]);
    c.long_pressed(0);
    assert!(c.block_menu_open());
    c.block_menu_chosen("duplicate");
    assert_eq!(c.rows().len(), 2);

    c.long_pressed(0);
    c.block_menu_chosen("delete");
    assert_eq!(c.rows().len(), 1);
    assert!(!c.block_menu_open());
}

#[test]
fn presence_gives_one_entry_per_co_editor() {
    use rimelight_content::sync::{Cursor, RemoteCursor};
    let mut c = page(&["one"]);
    let id = c.rows()[0].id.clone();
    let cursor = |user: &str| RemoteCursor {
        user_id: user.into(),
        name: "Ana".into(),
        color: "#3b82f6".into(),
        cursor: Some(Cursor {
            block: id.clone(),
            field: "text".into(),
            locale: "en".into(),
            anchor: 1,
            head: 1,
        }),
    };
    c.set_remote_cursors(vec![cursor("u1"), cursor("u1"), cursor("u2")]);
    assert_eq!(c.presence_views().len(), 2);
    assert_eq!(c.presence_views()[0].initial, "A");
    assert!(c.row_views()[0].presence.is_some());
}

#[test]
fn to_markdown_escapes_user_text() {
    let value = TextValue {
        plain: "*a* [b]".into(),
        spans: vec![Span {
            range: 0..7,
            marks: Vec::new(),
            link: None,
            mention: None,
            emoji: false,
            icon: None,
        }],
    };
    assert_eq!(to_markdown(&value), "\\*a\\* \\[b\\]");
}

#[test]
fn to_markdown_draws_a_mention_as_a_link() {
    let value = TextValue {
        plain: "@Ana".into(),
        spans: vec![Span {
            range: 0..4,
            marks: Vec::new(),
            link: None,
            mention: Some(Mention {
                entry_id: "e-1".into(),
                collection: "people".into(),
                slug: "ana".into(),
                title: Some("Ana".into()),
            }),
            emoji: false,
            icon: None,
        }],
    };
    assert_eq!(to_markdown(&value), "[@Ana](mention:e-1)");
}

#[test]
fn a_leading_dash_in_user_text_cannot_start_a_list() {
    let value = TextValue {
        plain: "- not a list".into(),
        spans: vec![Span {
            range: 0..12,
            marks: Vec::new(),
            link: None,
            mention: None,
            emoji: false,
            icon: None,
        }],
    };
    assert_eq!(to_markdown(&value), "\\- not a list");
}

#[test]
fn slint_control_codes_map_to_keys() {
    let input = KeyInput::from_slint("\u{0008}", Mods::default());
    assert_eq!(input.key, Key::Backspace);
    let up = KeyInput::from_slint("\u{F700}", Mods::default());
    assert_eq!(up.key, Key::Up);
    let letter = KeyInput::from_slint("b", ctrl());
    assert_eq!(letter.key, Key::Char('b'));
}

#[test]
fn embed_url_carries_the_block_json_in_the_fragment() {
    let block = json!({"id": "b1", "type": "LivePreviewBlock", "props": {"height": 320}});
    let url = embed_url("https://example.test/", &block);
    assert!(url.starts_with("https://example.test/embed/block#"));
    assert!(!url.contains("//embed"));
}
