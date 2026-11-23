//! WS7 "keeping it standard": the component catalogue (`packages/design/components.yaml` in the web monorepo, checked out
//! by cargo as the `rimelight_design` dependency) and this repo must agree.
//!
//! - Every component the catalogue marks `slint: done` has its Slint file here, and a gallery story.
//! - Every gallery story (outside Foundations) belongs to a component the catalogue marks done on Slint.
//!
//! Matching rule: a story id is `section/tail`. A component matches a story when the tail equals the kebab-case of the
//! component name or of one of its aliases (`ColorPicker` -> `color-picker`). A story that shows several components
//! is listed in [`COVERED_BY`]. Both tests list every offender at once.
//!
//! The catalogue is plain, fixed-shape YAML, so a small line parser reads it and no YAML crate is needed.

use rimelight_gallery::registry::{STORIES, Section};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Done on Slint but shown by another component's story: (component name, story id).
const COVERED_BY: &[(&str, &str)] = &[
    ("Rail", "shell/rail-sidebar"),
    ("Sidebar", "shell/rail-sidebar"),
    ("TabBar", "shell/tabs-split"),
    ("SplitPane", "shell/tabs-split"),
    ("TabStrip", "components/tabs"),
];

/// Done on Slint with no story of its own: (component name, reason). One line each.
const NO_STORY_ALLOWED: &[(&str, &str)] = &[];

/// Stories whose component the catalogue does not mark done on Slint: (story id, reason). One line each.
const STORY_NOT_DONE_ALLOWED: &[(&str, &str)] = &[
    (
        "components/toolbar-row",
        "catalogue has no done Slint entry for it yet",
    ),
    (
        "components/action-row",
        "catalogue says Media-only, but rimelight_ui has action-row.slint",
    ),
    (
        "components/tile",
        "catalogue says Media-only, but rimelight_ui has tile.slint",
    ),
    (
        "components/recent-list",
        "catalogue says Media-only, but rimelight_ui has recent-list.slint",
    ),
    (
        "templates/list-page",
        "templates are not catalogue components",
    ),
    (
        "templates/board-page",
        "templates are not catalogue components",
    ),
    (
        "templates/detail-page",
        "templates are not catalogue components",
    ),
];

#[derive(Debug, Default)]
struct Entry {
    name: String,
    aliases: Vec<String>,
    slint_status: String,
    slint_path: String,
}

fn kebab(s: &str) -> String {
    let mut out = String::new();
    let mut prev_lower = false;
    for c in s.chars() {
        if c.is_ascii_uppercase() {
            if prev_lower {
                out.push('-');
            }
            out.push(c.to_ascii_lowercase());
            prev_lower = false;
        } else {
            out.push(c);
            prev_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
        }
    }
    out
}

/// The token after `key` in `text`, ended by a comma, brace or line end.
fn value_after(text: &str, key: &str) -> String {
    let Some(at) = text.find(key) else {
        return String::new();
    };
    text[at + key.len()..]
        .trim_start()
        .split([',', '}', '\n'])
        .next()
        .unwrap_or("")
        .trim()
        .to_string()
}

fn finish(entry: Option<&mut Entry>, aliases: &mut String, slint: &mut String) {
    if let Some(e) = entry {
        let list = aliases
            .trim_start_matches(|c| c != '[')
            .trim_matches(|c| c == '[' || c == ']' || c == ' ' || c == '\n');
        e.aliases = list
            .split(',')
            .map(|a| a.trim().to_string())
            .filter(|a| !a.is_empty())
            .collect();
        e.slint_status = value_after(slint, "status:");
        e.slint_path = value_after(slint, "path:");
    }
    aliases.clear();
    slint.clear();
}

fn parse(yaml: &str) -> Vec<Entry> {
    let mut entries: Vec<Entry> = Vec::new();
    let mut key = String::new();
    let mut aliases_text = String::new();
    let mut slint_text = String::new();
    for line in yaml.lines() {
        if line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix("- name:") {
            finish(entries.last_mut(), &mut aliases_text, &mut slint_text);
            entries.push(Entry {
                name: name.trim().to_string(),
                ..Entry::default()
            });
            key.clear();
            continue;
        }
        let top_key = line
            .strip_prefix("  ")
            .filter(|rest| !rest.starts_with(' '))
            .and_then(|rest| rest.split_once(':'))
            .map(|(k, _)| k.to_string());
        if let Some(k) = top_key {
            key = k;
        }
        match key.as_str() {
            "aliases" => {
                aliases_text.push_str(line);
                aliases_text.push('\n');
            }
            "slint" => {
                slint_text.push_str(line);
                slint_text.push('\n');
            }
            _ => {}
        }
    }
    finish(entries.last_mut(), &mut aliases_text, &mut slint_text);
    entries
}

fn catalogue() -> Vec<Entry> {
    // UI_DIR is `<monorepo>/packages/design/crate/ui`; the catalogue sits two levels up.
    let path = Path::new(rimelight_design::UI_DIR).join("../../components.yaml");
    let yaml =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let entries = parse(&yaml);
    assert!(
        entries.len() > 100,
        "parsed only {} catalogue entries from {}",
        entries.len(),
        path.display()
    );
    entries
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn keys(e: &Entry) -> BTreeSet<String> {
    std::iter::once(&e.name)
        .chain(&e.aliases)
        .map(|n| kebab(n))
        .collect()
}

fn tail(id: &str) -> &str {
    id.split_once('/').map_or(id, |(_, t)| t)
}

fn has_story(e: &Entry) -> bool {
    let keys = keys(e);
    STORIES.iter().any(|s| keys.contains(tail(s.id)))
        || COVERED_BY
            .iter()
            .any(|(c, id)| *c == e.name && STORIES.iter().any(|s| s.id == *id))
}

#[test]
fn done_on_slint_has_a_file_and_a_story() {
    let root = repo_root();
    let mut problems = Vec::new();
    for e in catalogue().iter().filter(|e| e.slint_status == "done") {
        let file = e.slint_path.split('#').next().unwrap_or("");
        if file.starts_with("ui/widgets.slint") {
            eprintln!(
                "skipped {}: path {} is a Media (other repo) file",
                e.name, e.slint_path
            );
        } else if !root.join(file).is_file() {
            problems.push(format!("{}: file {} does not exist", e.name, e.slint_path));
        }
        let allowed = NO_STORY_ALLOWED.iter().any(|(n, _)| *n == e.name);
        if !allowed && !has_story(e) {
            problems.push(format!("{}: no gallery story", e.name));
        }
    }
    assert!(
        problems.is_empty(),
        "{} catalogue problem(s), fix the code or the allowlist:\n{}",
        problems.len(),
        problems.join("\n")
    );
}

#[test]
fn every_story_is_a_done_component() {
    let done: Vec<Entry> = catalogue()
        .into_iter()
        .filter(|e| e.slint_status == "done")
        .collect();
    let mut problems = Vec::new();
    for s in STORIES.iter().filter(|s| s.section != Section::Foundations) {
        let known = done.iter().any(|e| keys(e).contains(tail(s.id)))
            || COVERED_BY
                .iter()
                .any(|(c, id)| *id == s.id && done.iter().any(|e| e.name == *c))
            || STORY_NOT_DONE_ALLOWED.iter().any(|(id, _)| *id == s.id);
        if !known {
            problems.push(format!("{}: no catalogue entry marked done on Slint", s.id));
        }
    }
    assert!(
        problems.is_empty(),
        "{} story problem(s), fix the catalogue or the allowlist:\n{}",
        problems.len(),
        problems.join("\n")
    );
}
