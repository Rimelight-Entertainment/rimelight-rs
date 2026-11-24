//! Contract tests for MAPPING.md, run on the shared fixtures.

use loro::ExportMode;
use rimelight_content::{
    Cursor, EditorDoc, Error, Frame, MarkKind, Received, SyncSession, TextKind, decode, doc,
    encode, normalize_page, registry, text_kind,
};
use serde_json::Value;

const DOCS_GUIDE: &str = include_str!("../fixtures/docs-guide.page.json");
const WIKI_CHARACTER: &str = include_str!("../fixtures/wiki-character.page.json");
const SCREENPLAY: &str = include_str!("../fixtures/screenplay.page.json");
const WEB_ONLY: &str = include_str!("../fixtures/web-only.page.json");

/// Two paragraphs of the docs-guide fixture (see `fixtures/docs-guide.page.json`).
const PARA_A: &str = "0f7c6a52-3b1e-4d8a-9c2f-6e1a2b3c4d01";
const PARA_B: &str = "0f7c6a52-3b1e-4d8a-9c2f-6e1a2b3c4d02";

fn fixtures() -> Vec<(&'static str, Value)> {
    vec![
        ("docs-guide", parse(DOCS_GUIDE)),
        ("wiki-character", parse(WIKI_CHARACTER)),
        ("screenplay", parse(SCREENPLAY)),
        ("web-only", parse(WEB_ONLY)),
    ]
}

fn parse(text: &str) -> Value {
    serde_json::from_str(text).expect("fixture is valid JSON")
}

fn find_block<'a>(blocks: &'a Value, id: &str) -> Option<&'a Value> {
    for block in blocks.as_array()? {
        if block["id"].as_str() == Some(id) {
            return Some(block);
        }
        if let Some(found) = block.get("children").and_then(|c| find_block(c, id)) {
            return Some(found);
        }
    }
    None
}

/// The text of a rich text value, nodes concatenated (mentions by title or slug).
fn plain(nodes: &Value) -> String {
    let mut out = String::new();
    for node in nodes.as_array().into_iter().flatten() {
        match node["type"].as_str() {
            Some("text") => out.push_str(node["text"].as_str().unwrap_or_default()),
            Some("emoji") => out.push_str(node["emoji"].as_str().unwrap_or_default()),
            Some("mention") => out.push_str(
                node["title"]
                    .as_str()
                    .or_else(|| node["slug"].as_str())
                    .unwrap_or_default(),
            ),
            Some("link") => out.push_str(&plain(&node["children"])),
            _ => {}
        }
    }
    out
}

fn paragraph_text(page: &Value, id: &str) -> String {
    let block = find_block(&page["content"]["blocks"], id).expect("block exists");
    plain(&block["props"]["text"])
}

fn import(page: &Value) -> loro::LoroDoc {
    doc::import_page(page).expect("import succeeds")
}

#[test]
fn round_trip_is_normal_form_for_every_fixture() {
    for (name, page) in fixtures() {
        let loro = import(&page);
        let out = doc::export_page(&loro).expect("export succeeds");
        assert_eq!(
            out,
            normalize_page(&page),
            "{name}: export(import(x)) == normalize(x)"
        );
    }
}

#[test]
fn fixtures_are_already_normal() {
    for (name, page) in fixtures() {
        assert_eq!(
            normalize_page(&page),
            page,
            "{name}: fixture must be in normal form"
        );
    }
}

fn check_blocks(name: &str, blocks: &Value, parent: Option<&str>) {
    for block in blocks.as_array().expect("blocks is an array") {
        let block_type = block["type"].as_str().expect("block type");
        // Unknown types are kept as they are, with their whole subtree.
        if block_type == "FutureBlock" {
            continue;
        }
        assert!(
            registry::block_def(block_type).is_some(),
            "{name}: {block_type} is not in the registry"
        );
        assert!(
            registry::allowed_child(parent, block_type),
            "{name}: {block_type} is not allowed under {parent:?}"
        );
        if let Some(children) = block.get("children") {
            check_blocks(name, children, Some(block_type));
        }
    }
}

#[test]
fn every_fixture_block_is_in_registry_and_nests_legally() {
    for (name, page) in fixtures() {
        check_blocks(name, &page["content"]["blocks"], None);
    }
}

#[test]
fn text_rules_follow_the_registry() {
    assert_eq!(text_kind("ParagraphBlock", "text"), Some(TextKind::Rich));
    assert_eq!(text_kind("ListItemBlock", "content"), Some(TextKind::Rich));
    assert_eq!(text_kind("TabItemBlock", "label"), Some(TextKind::Plain));
    assert_eq!(text_kind("CodeBlock", "code"), Some(TextKind::Plain));
    assert_eq!(text_kind("SceneBlock", "description"), Some(TextKind::Rich));
    assert_eq!(text_kind("ScriptBlock", "characters"), None);
    assert_eq!(text_kind("FutureBlock", "text"), None);
}

#[test]
fn draft_edit_merges_into_main() {
    let page = parse(DOCS_GUIDE);
    let main = import(&page);
    let draft = doc::fork_draft(&main);

    let text = doc::block_text(&draft, PARA_A, "t:text").expect("paragraph text in draft");
    text.insert(0, "DRAFT ").expect("edit draft");
    draft.commit();

    doc::merge_draft(&main, &draft).expect("merge succeeds");
    let out = doc::export_page(&main).expect("export succeeds");
    assert!(paragraph_text(&out, PARA_A).starts_with("DRAFT "));
    assert!(paragraph_text(&out, PARA_B).starts_with("Second paragraph"));
}

#[test]
fn concurrent_drafts_keep_both_edits() {
    let page = parse(DOCS_GUIDE);
    let main = import(&page);
    let a = doc::fork_draft(&main);
    let b = doc::fork_draft(&main);

    // Both edit the same paragraph at the same position, and `a` also edits another block.
    doc::block_text(&a, PARA_A, "t:text")
        .expect("text in a")
        .insert(0, "A-")
        .expect("edit a");
    doc::block_text(&a, PARA_B, "t:text")
        .expect("text in a")
        .insert(0, "X-")
        .expect("edit a");
    a.commit();
    doc::block_text(&b, PARA_A, "t:text")
        .expect("text in b")
        .insert(0, "B-")
        .expect("edit b");
    b.commit();

    doc::merge_draft(&main, &a).expect("merge a");
    doc::merge_draft(&main, &b).expect("merge b");

    let out = doc::export_page(&main).expect("export succeeds");
    let para_a = paragraph_text(&out, PARA_A);
    assert!(para_a.contains("A-"), "edit from a is kept: {para_a}");
    assert!(para_a.contains("B-"), "edit from b is kept: {para_a}");
    assert!(paragraph_text(&out, PARA_B).starts_with("X-"));
}

#[test]
fn catch_up_brings_main_edits_into_draft() {
    let page = parse(DOCS_GUIDE);
    let main = import(&page);
    let draft = doc::fork_draft(&main);

    doc::block_text(&main, PARA_A, "t:text")
        .expect("text in main")
        .insert(0, "M-")
        .expect("edit main");
    main.commit();

    doc::catch_up(&draft, &main).expect("catch up succeeds");
    let out = doc::export_page(&draft).expect("export succeeds");
    assert!(paragraph_text(&out, PARA_A).starts_with("M-"));
}

#[test]
fn registry_parses_and_lists_site_blocks() {
    let blocks = registry::all_blocks();
    assert!(blocks.iter().any(|b| b.block_type == "ParagraphBlock"));
    assert!(
        blocks
            .iter()
            .any(|b| b.block_type == "SceneBlock" && b.parents.is_some())
    );
    let web_only: Vec<&str> = blocks
        .iter()
        .filter(|b| b.web_only)
        .map(|b| b.block_type.as_str())
        .collect();
    assert_eq!(web_only, vec!["LivePreviewBlock", "ApiDocumentationBlock"]);
}

/// Cross-language check (MAPPING.md section 4): Loro bytes written by either side export to the same JSON.
///
/// `fixtures/loro/<name>.rust.bin` is written by this crate and `<name>.ts.bin` by the website's TypeScript
/// mapping, each when `RIMELIGHT_WRITE_LORO_FIXTURES=1` is set; the `.bin` files are copied between the two
/// repos and committed. Every `.bin` present must export to the normal form of its fixture.
#[test]
fn loro_bytes_from_both_languages_export_to_the_fixture() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/loro");
    if std::env::var("RIMELIGHT_WRITE_LORO_FIXTURES").as_deref() == Ok("1") {
        std::fs::create_dir_all(&dir).expect("create fixtures/loro");
        for (name, page) in fixtures() {
            let doc = doc::import_page(&page).expect("import");
            let bytes = doc
                .export(loro::ExportMode::Snapshot)
                .expect("export snapshot");
            std::fs::write(dir.join(format!("{name}.rust.bin")), bytes).expect("write bin");
        }
    }
    let mut checked = 0;
    for (name, page) in fixtures() {
        for side in ["rust", "ts"] {
            let path = dir.join(format!("{name}.{side}.bin"));
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            let doc = loro::LoroDoc::new();
            doc::configure(&doc);
            doc.import(&bytes).expect("import bytes");
            let exported = doc::export_page(&doc).expect("export");
            assert_eq!(exported, normalize_page(&page), "{name} from {side} bytes");
            checked += 1;
        }
    }
    assert!(checked > 0, "no Loro byte fixtures in {}", dir.display());
}

// ----- editor API (EDITOR_API.md) -----

/// A page for the editor tests. `p1` has a bold run and a mention in English and a Portuguese text; `s1` holds `p2`;
/// `ul` holds two list items; `tb` is a table; `tpl` is a templated paragraph whose `text` is locked.
fn editor_page() -> Value {
    serde_json::json!({
        "meta": {
            "slug": "editor-test",
            "type": "docs",
            "title": { "en": "Title", "pt": "Título" }
        },
        "content": {
            "properties": {},
            "blocks": [
                {
                    "id": "p1",
                    "type": "ParagraphBlock",
                    "props": {
                        "text": {
                            "en": [
                                { "type": "text", "text": "Hello " },
                                { "type": "text", "text": "world", "marks": ["bold"] },
                                { "type": "mention", "entryId": "e1", "collection": "docs", "slug": "intro", "title": "Intro" },
                                { "type": "text", "text": " end" }
                            ],
                            "pt": [{ "type": "text", "text": "Olá" }]
                        }
                    }
                },
                {
                    "id": "s1",
                    "type": "SectionBlock",
                    "props": { "title": [{ "type": "text", "text": "Heading" }] },
                    "children": [
                        {
                            "id": "p2",
                            "type": "ParagraphBlock",
                            "props": { "text": [{ "type": "text", "text": "Second" }] }
                        }
                    ]
                },
                {
                    "id": "ul",
                    "type": "UnorderedListBlock",
                    "props": {},
                    "children": [
                        { "id": "li1", "type": "ListItemBlock", "props": { "content": [{ "type": "text", "text": "one" }] } },
                        { "id": "li2", "type": "ListItemBlock", "props": { "content": [{ "type": "text", "text": "two" }] } }
                    ]
                },
                {
                    "id": "tb",
                    "type": "TableBlock",
                    "props": { "columns": [{ "key": "a", "header": "A" }], "rows": [{ "a": "x" }] }
                },
                {
                    "id": "tpl",
                    "type": "ParagraphBlock",
                    "props": { "text": [{ "type": "text", "text": "locked" }] },
                    "isTemplated": true,
                    "templatedProps": ["text"]
                }
            ]
        }
    })
}

fn editor_from(page: &Value) -> EditorDoc {
    EditorDoc::from_doc(doc::import_page(page).expect("editor page imports"))
}

fn top_level_ids(ed: &EditorDoc) -> Vec<String> {
    ed.rows("en")
        .into_iter()
        .filter(|row| row.parent.is_none())
        .map(|row| row.id)
        .collect()
}

/// Sends everything `from` has ready for `to`, through the wire encoding.
fn deliver(
    from: &mut SyncSession,
    src: &EditorDoc,
    to: &mut SyncSession,
    dst: &EditorDoc,
    now: u64,
) {
    for frame in from.outgoing_at(src, now) {
        let decoded = decode(&encode(&frame)).expect("frame decodes");
        to.receive(dst, decoded);
    }
}

#[test]
fn editor_typing_is_exported() {
    let mut ed = editor_from(&editor_page());
    ed.replace_text("p2", "text", "en", "Second and more")
        .unwrap();
    assert_eq!(ed.text("p2", "text", "en").plain, "Second and more");
    let page = ed.page().unwrap();
    let block = find_block(&page["content"]["blocks"], "p2").unwrap();
    assert_eq!(block["props"]["text"][0]["text"], "Second and more");
}

#[test]
fn editor_bold_toggle() {
    let mut ed = editor_from(&editor_page());
    ed.toggle_mark("p1", "text", "en", 0..5, MarkKind::Bold)
        .unwrap();
    let bold_on = ed
        .text("p1", "text", "en")
        .spans
        .iter()
        .any(|span| span.range == (0..5) && span.marks.contains(&MarkKind::Bold));
    assert!(bold_on);
    // Toggling the same range again removes it.
    ed.toggle_mark("p1", "text", "en", 0..5, MarkKind::Bold)
        .unwrap();
    let bold_off = ed
        .text("p1", "text", "en")
        .spans
        .iter()
        .any(|span| span.range == (0..5) && span.marks.contains(&MarkKind::Bold));
    assert!(!bold_off);
}

#[test]
fn editor_mention_is_atomic() {
    let mut ed = editor_from(&editor_page());
    // Plain text is "Hello worldIntro end"; the mention covers bytes 11..16. An edit inside it removes it whole.
    ed.replace_text("p1", "text", "en", "Hello worldIn end")
        .unwrap();
    let text = ed.text("p1", "text", "en");
    assert_eq!(text.plain, "Hello worldIn end");
    assert!(text.spans.iter().all(|span| span.mention.is_none()));
}

#[test]
fn editor_split_and_merge() {
    let mut ed = editor_from(&editor_page());
    let new_id = ed.split_block("p2", "text", "en", 3).unwrap();
    assert_eq!(ed.text("p2", "text", "en").plain, "Sec");
    assert_eq!(ed.text(&new_id, "text", "en").plain, "ond");
    let merged = ed.merge_into_previous(&new_id, "text", "en").unwrap();
    assert_eq!(merged, Some(("p2".to_string(), 3)));
    assert_eq!(ed.text("p2", "text", "en").plain, "Second");
    assert!(ed.row(&new_id, "en").is_none());
}

#[test]
fn editor_insert_block_creates_initial_children() {
    let mut ed = editor_from(&editor_page());
    let list = ed
        .insert_block(None, Some("p1"), "UnorderedListBlock")
        .unwrap();
    assert!(ed.row(&list, "en").unwrap().has_children);
    let child = ed
        .rows("en")
        .into_iter()
        .find(|row| row.parent.as_deref() == Some(list.as_str()))
        .unwrap();
    assert_eq!(child.block_type, "ListItemBlock");
}

#[test]
fn editor_indent_and_outdent_list_items() {
    let mut ed = editor_from(&editor_page());
    ed.indent("li2").unwrap();
    let nested = ed.row("li2", "en").unwrap();
    assert_eq!(nested.depth, 3);
    let nested_list = nested.parent.clone().unwrap();
    assert_eq!(
        ed.row(&nested_list, "en").unwrap().parent.as_deref(),
        Some("li1")
    );

    ed.outdent("li2").unwrap();
    let back = ed.row("li2", "en").unwrap();
    assert_eq!(back.depth, 1);
    assert_eq!(back.parent.as_deref(), Some("ul"));
}

#[test]
fn editor_move_up_and_down() {
    let mut ed = editor_from(&editor_page());
    let before = top_level_ids(&ed);
    assert_eq!(before[0], "p1");
    ed.move_down("p1").unwrap();
    assert_eq!(top_level_ids(&ed)[1], "p1");
    ed.move_up("p1").unwrap();
    assert_eq!(top_level_ids(&ed), before);
}

#[test]
fn editor_turn_into_keeps_text() {
    let mut ed = editor_from(&editor_page());
    ed.turn_into("p1", "QuoteBlock").unwrap();
    assert_eq!(ed.row("p1", "en").unwrap().block_type, "QuoteBlock");
    assert_eq!(ed.text("p1", "text", "en").plain, "Hello worldIntro end");
}

#[test]
fn editor_templated_block_is_locked() {
    let mut ed = editor_from(&editor_page());
    let deleted = ed.delete_blocks(&["tpl".to_string()]);
    assert!(matches!(deleted, Err(Error::Locked)));
    let set = ed.set_prop("tpl", "text", serde_json::json!([]));
    assert!(matches!(set, Err(Error::Locked)));
    assert!(ed.row("tpl", "en").unwrap().templated);
}

#[test]
fn editor_undo_and_redo() {
    let mut ed = editor_from(&editor_page());
    ed.replace_text("p2", "text", "en", "X").unwrap();
    assert_eq!(ed.text("p2", "text", "en").plain, "X");
    assert!(ed.undo());
    assert_eq!(ed.text("p2", "text", "en").plain, "Second");
    assert!(ed.redo());
    assert_eq!(ed.text("p2", "text", "en").plain, "X");
}

#[test]
fn editor_table_cell_edit() {
    let mut ed = editor_from(&editor_page());
    ed.table_set_cell("tb", 0, "a", "y").unwrap();
    let page = ed.page().unwrap();
    let block = find_block(&page["content"]["blocks"], "tb").unwrap();
    assert_eq!(block["props"]["rows"][0]["a"], "y");
}

#[test]
fn editor_locales_lists_en_first() {
    let ed = editor_from(&editor_page());
    assert_eq!(ed.locales(), vec!["en".to_string(), "pt".to_string()]);
}

#[test]
fn editor_sync_two_editors_converge() {
    let page = editor_page();
    let snapshot = doc::import_page(&page)
        .unwrap()
        .export(ExportMode::Snapshot)
        .unwrap();
    let mut a = EditorDoc::from_snapshot(&snapshot).unwrap();
    let mut b = EditorDoc::from_snapshot(&snapshot).unwrap();
    a.replace_text("p2", "text", "en", "From A").unwrap();
    b.insert_block(None, None, "DividerBlock").unwrap();

    let mut sa = SyncSession::new("user-a", "A", "#f00");
    let mut sb = SyncSession::new("user-b", "B", "#0f0");
    deliver(&mut sa, &a, &mut sb, &b, 1_000);
    deliver(&mut sb, &b, &mut sa, &a, 1_000);
    assert_eq!(a.page().unwrap(), b.page().unwrap());
    assert_eq!(a.text("p2", "text", "en").plain, "From A");
}

#[test]
fn editor_sync_presence_round_trip() {
    let page = editor_page();
    let a = editor_from(&page);
    let b = editor_from(&page);
    let mut sa = SyncSession::new("user-a", "A", "#f00");
    let mut sb = SyncSession::new("user-b", "B", "#0f0");
    sa.set_cursor(Some(Cursor {
        block: "p2".into(),
        field: "text".into(),
        locale: "en".into(),
        anchor: 1,
        head: 3,
    }));
    let presence = sa
        .outgoing_at(&a, 5_000)
        .into_iter()
        .find(|frame| matches!(frame, Frame::Presence(_)))
        .expect("cursor change is sent as presence");
    assert_eq!(
        sb.receive(&b, decode(&encode(&presence)).unwrap()),
        Received::Presence
    );

    let remote = sb.remote_cursors_at(6_000);
    assert_eq!(remote.len(), 1);
    assert_eq!(remote[0].user_id, "user-a");
    assert_eq!(remote[0].cursor.as_ref().map(|c| c.anchor), Some(1));
    assert!(sb.remote_cursors_at(5_000 + 30_001).is_empty());
}
