//! Block registry, parsed once from `registry/blocks.json` (the single source; the TypeScript side copies it).
//!
//! Rules that decide how props are stored in Loro (`text_kind`) and which blocks may nest (`allowed_child`) follow
//! MAPPING sections 2 and the `BlockRegistry` semantics in `@rimelight/cms`.

use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::sync::OnceLock;

const REGISTRY_JSON: &str = include_str!("../registry/blocks.json");

#[derive(Debug, Clone, Deserialize)]
pub struct RegistryFile {
    pub version: u32,
    pub blocks: Vec<BlockDef>,
}

/// One block type.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockDef {
    #[serde(rename = "type")]
    pub block_type: String,
    /// Keys `en`, `pt`, `es`.
    pub label: BTreeMap<String, String>,
    pub icon: Option<String>,
    pub category: Option<String>,
    /// `null` (leaf), `"*"` (any root-level block) or a list of child types.
    #[serde(default)]
    pub children: Option<ChildRule>,
    /// Parent types this block may live inside. `null` means no restriction (but see `nested_only`).
    #[serde(default)]
    pub parents: Option<Vec<String>>,
    #[serde(default)]
    pub nested_only: bool,
    #[serde(default)]
    pub max_nesting: Option<u32>,
    #[serde(default)]
    pub inline_text: Option<String>,
    #[serde(default)]
    pub rich_text: bool,
    #[serde(default)]
    pub web_only: bool,
    #[serde(default)]
    pub default_props: Map<String, Value>,
    /// Block JSON created with a new instance. Ids are empty; the editor assigns them.
    #[serde(default)]
    pub initial_children: Option<Vec<Value>>,
    #[serde(default)]
    pub fields: BTreeMap<String, FieldDef>,
}

/// Child rule of a block: `"*"` or an explicit list.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum ChildRule {
    Any(String),
    List(Vec<String>),
}

/// One prop field. `type` is the field vocabulary (`text`, `textarea`, `richtext`, `select`, ...).
#[derive(Debug, Clone, Deserialize)]
pub struct FieldDef {
    #[serde(rename = "type")]
    pub field_type: String,
    pub label: BTreeMap<String, String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub options: Option<Vec<SelectOption>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
}

/// How a prop is stored in Loro: rich text (inline marks) or plain text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextKind {
    Rich,
    Plain,
}

fn registry() -> &'static [BlockDef] {
    static CELL: OnceLock<Vec<BlockDef>> = OnceLock::new();
    CELL.get_or_init(|| {
        // The JSON is part of this crate; `tests/mapping.rs` parses it, so a broken file fails the test run.
        serde_json::from_str::<RegistryFile>(REGISTRY_JSON)
            .expect("registry/blocks.json is valid")
            .blocks
    })
}

/// Every block type, built-in and site-defined, in registry order.
pub fn all_blocks() -> &'static [BlockDef] {
    registry()
}

/// The definition of `block_type`, if it is in the registry. Unknown types return `None`.
pub fn block_def(block_type: &str) -> Option<&'static BlockDef> {
    registry().iter().find(|def| def.block_type == block_type)
}

/// Storage kind of one prop of one block type (MAPPING section 2, "Which props are text").
///
/// Returns `None` for every prop that is not text: non-text props, props of unknown block types, and props missing
/// from the registry. Callers store those as plain JSON.
pub fn text_kind(block_type: &str, prop: &str) -> Option<TextKind> {
    let def = block_def(block_type)?;
    if block_type == "CodeBlock" && prop == "code" {
        return Some(TextKind::Plain);
    }
    if let Some(field) = def.fields.get(prop) {
        match field.field_type.as_str() {
            "richtext" => return Some(TextKind::Rich),
            "text" | "textarea" => return Some(TextKind::Plain),
            _ => {}
        }
    }
    if def.inline_text.as_deref() == Some(prop) {
        return Some(if def.rich_text {
            TextKind::Rich
        } else {
            TextKind::Plain
        });
    }
    None
}

/// Whether a block of type `child` may sit directly under `parent` (`None` = the page root).
///
/// Mirrors `BlockRegistry.allowedChildren`: a root-level block must be known, not `nested_only`, and have no
/// `parents`. A nested block must be listed by the parent (or the parent accepts `"*"`), and if the child names
/// `parents`, the parent must be one of them. Unknown types are never allowed.
pub fn allowed_child(parent: Option<&str>, child: &str) -> bool {
    let Some(child_def) = block_def(child) else {
        return false;
    };
    let Some(parent) = parent else {
        return !child_def.nested_only && child_def.parents.is_none();
    };
    let Some(parent_def) = block_def(parent) else {
        return false;
    };
    match &parent_def.children {
        None => false,
        Some(ChildRule::Any(_)) => allowed_child(None, child),
        Some(ChildRule::List(listed)) => {
            listed.iter().any(|c| c == child)
                && child_def
                    .parents
                    .as_ref()
                    .is_none_or(|parents| parents.iter().any(|p| p == parent))
        }
    }
}
