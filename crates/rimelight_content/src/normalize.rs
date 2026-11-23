//! The normal form (MAPPING section 4). `export(import(x)) == normalize(x)` for every fixture.
//!
//! Normalization also merges what Loro merges on export: adjacent text nodes with the same marks, adjacent mentions
//! with equal values (one atomic node), adjacent links with equal `url` and `openInNewTab` (children concatenated),
//! and adjacent emoji with equal shortcode (text concatenated). Icons are never merged: one node per icon character.

use crate::registry::{TextKind, text_kind};
use crate::richtext::{CANON_MARKS, is_rich};
use serde_json::{Map, Value, json};

/// Normal form of a whole page. Meta is kept as is. Rich props of every block are normalized, empty `children`
/// are removed, and the block tree is normalized recursively.
pub fn normalize_page(page: &Value) -> Value {
    let mut out = page.clone();
    if let Some(blocks) = out
        .pointer_mut("/content/blocks")
        .and_then(Value::as_array_mut)
    {
        for block in blocks.iter_mut() {
            let normalized = normalize_block(block);
            *block = normalized;
        }
    }
    out
}

/// Normal form of one block: rich props (by the registry's text rules) and its children.
pub fn normalize_block(block: &Value) -> Value {
    let Some(obj) = block.as_object() else {
        return block.clone();
    };
    let mut out = obj.clone();
    let block_type = obj.get("type").and_then(Value::as_str).unwrap_or_default();
    if let Some(props) = obj.get("props").and_then(Value::as_object) {
        let mut new_props = props.clone();
        for (key, value) in props {
            if text_kind(block_type, key) == Some(TextKind::Rich) && is_rich(value) {
                new_props.insert(key.clone(), normalize_rich(value));
            }
        }
        out.insert("props".into(), Value::Object(new_props));
    }
    match obj.get("children").and_then(Value::as_array) {
        Some(children) if !children.is_empty() => {
            out.insert(
                "children".into(),
                Value::Array(children.iter().map(normalize_block).collect()),
            );
        }
        _ => {
            out.remove("children");
        }
    }
    Value::Object(out)
}

/// Normal form of a rich text value: a node array, or a per-locale map of node arrays.
pub fn normalize_rich(value: &Value) -> Value {
    match value {
        Value::Array(nodes) => Value::Array(normalize_nodes(nodes)),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(locale, v)| {
                    let v = match v {
                        Value::Array(nodes) => Value::Array(normalize_nodes(nodes)),
                        other => other.clone(),
                    };
                    (locale.clone(), v)
                })
                .collect::<Map<String, Value>>(),
        ),
        other => other.clone(),
    }
}

/// Normalizes a node list: each node, then merges of adjacent nodes that Loro would merge.
pub(crate) fn normalize_nodes(nodes: &[Value]) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::with_capacity(nodes.len());
    for node in nodes {
        if let Some(normalized) = normalize_node(node) {
            push_merged(&mut out, normalized);
        }
    }
    out
}

/// One node in normal form. `None` drops the node (empty text, empty emoji).
fn normalize_node(node: &Value) -> Option<Value> {
    let obj = node.as_object()?;
    match obj.get("type").and_then(Value::as_str).unwrap_or_default() {
        "text" => {
            let text = obj.get("text").and_then(Value::as_str)?;
            if text.is_empty() {
                return None;
            }
            Some(text_node(text, &canonical_marks(obj.get("marks"))))
        }
        "link" => {
            let url = obj.get("url").and_then(Value::as_str)?;
            let children: &[Value] = obj
                .get("children")
                .and_then(Value::as_array)
                .map_or(&[][..], Vec::as_slice);
            let mut out = Map::new();
            out.insert("type".into(), json!("link"));
            out.insert("url".into(), json!(url));
            out.insert("children".into(), Value::Array(normalize_nodes(children)));
            if obj.get("openInNewTab") == Some(&Value::Bool(true)) {
                out.insert("openInNewTab".into(), Value::Bool(true));
            }
            Some(Value::Object(out))
        }
        "mention" => {
            // color and href are delivery-only and are dropped here.
            let mut out = Map::new();
            out.insert("type".into(), json!("mention"));
            for key in ["entryId", "collection", "slug"] {
                out.insert(
                    key.into(),
                    obj.get(key).cloned().unwrap_or_else(|| json!("")),
                );
            }
            if let Some(title) = obj.get("title") {
                out.insert("title".into(), title.clone());
            }
            Some(Value::Object(out))
        }
        "emoji" => {
            let emoji = obj.get("emoji").and_then(Value::as_str)?;
            if emoji.is_empty() {
                return None;
            }
            let mut out = Map::new();
            out.insert("type".into(), json!("emoji"));
            out.insert("emoji".into(), json!(emoji));
            if let Some(code) = obj
                .get("shortcode")
                .and_then(Value::as_str)
                .filter(|c| !c.is_empty())
            {
                out.insert("shortcode".into(), json!(code));
            }
            Some(Value::Object(out))
        }
        "icon" => {
            let mut out = Map::new();
            out.insert("type".into(), json!("icon"));
            out.insert(
                "name".into(),
                obj.get("name").cloned().unwrap_or_else(|| json!("")),
            );
            if let Some(label) = obj.get("label") {
                out.insert("label".into(), label.clone());
            }
            Some(Value::Object(out))
        }
        _ => Some(node.clone()),
    }
}

/// Appends `next` to `out`, merging it into the last node when Loro would have merged them.
fn push_merged(out: &mut Vec<Value>, next: Value) {
    if let Some(last) = out.last_mut()
        && let Some(merged) = merge_pair(last, &next)
    {
        *last = merged;
        return;
    }
    out.push(next);
}

/// The merge of two adjacent normalized nodes, if they merge.
fn merge_pair(last: &Value, next: &Value) -> Option<Value> {
    let kind = last.get("type").and_then(Value::as_str)?;
    if next.get("type").and_then(Value::as_str) != Some(kind) {
        return None;
    }
    match kind {
        "text" => {
            if last.get("marks") != next.get("marks") {
                return None;
            }
            let text = format!(
                "{}{}",
                last.get("text").and_then(Value::as_str).unwrap_or_default(),
                next.get("text").and_then(Value::as_str).unwrap_or_default()
            );
            Some(text_node(&text, &canonical_marks(last.get("marks"))))
        }
        "mention" => (last == next).then(|| last.clone()),
        "emoji" => {
            if last.get("shortcode") != next.get("shortcode") {
                return None;
            }
            let mut merged = last.clone();
            let emoji = format!(
                "{}{}",
                last.get("emoji")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                next.get("emoji")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
            );
            set_field(&mut merged, "emoji", json!(emoji));
            Some(merged)
        }
        "link" => {
            if last.get("url") != next.get("url")
                || last.get("openInNewTab") != next.get("openInNewTab")
            {
                return None;
            }
            let mut children: Vec<Value> = last
                .get("children")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for child in next
                .get("children")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                push_merged(&mut children, child.clone());
            }
            let mut merged = last.clone();
            set_field(&mut merged, "children", Value::Array(children));
            Some(merged)
        }
        _ => None,
    }
}

fn set_field(node: &mut Value, key: &str, value: Value) {
    if let Some(obj) = node.as_object_mut() {
        obj.insert(key.into(), value);
    }
}

/// Marks that are true, in canonical order, without duplicates.
fn canonical_marks(marks: Option<&Value>) -> Vec<&'static str> {
    let present: Vec<&str> = marks
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    CANON_MARKS
        .iter()
        .copied()
        .filter(|m| present.contains(m))
        .collect()
}

fn text_node(text: &str, marks: &[&str]) -> Value {
    let mut out = Map::new();
    out.insert("type".into(), json!("text"));
    out.insert("text".into(), json!(text));
    if !marks.is_empty() {
        out.insert("marks".into(), json!(marks));
    }
    Value::Object(out)
}
