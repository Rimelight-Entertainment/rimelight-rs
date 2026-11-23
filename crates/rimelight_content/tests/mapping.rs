//! Contract tests for MAPPING.md, run on the shared fixtures.

use rimelight_content::{TextKind, doc, normalize_page, registry, text_kind};
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
