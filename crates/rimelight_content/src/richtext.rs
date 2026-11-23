//! Rich and plain text between JSON and Loro text (MAPPING sections 2, 3 and the value rules in 2).
//!
//! Positions are Unicode scalar indexes. Loro style keys are the mark names. `mention`, `emoji`, `icon` and `link`
//! carry an object or string value as described in section 3.

use crate::error::{Error, map_loro};
use crate::normalize::normalize_nodes;
use loro::{LoroText, LoroValue, TextDelta};
use serde_json::{Map, Value, json};

/// The character stored for an inline icon.
pub const ICON_CHAR: char = '\u{FFFC}';

/// Marks in canonical order (the order of the normal form and of exported `marks`).
pub const CANON_MARKS: [&str; 5] = ["bold", "italic", "underline", "strikethrough", "code"];

/// Marks that grow into text inserted after them (`expand: after`). Every inserted text range sets them explicitly,
/// so an inserted run never inherits a mark it does not have.
const EXPAND_AFTER_MARKS: [&str; 4] = ["bold", "italic", "underline", "strikethrough"];

/// Whether `s` is a locale code: `en`, or `pt-BR`.
pub fn is_locale_code(s: &str) -> bool {
    let b = s.as_bytes();
    match b.len() {
        2 => b[0].is_ascii_lowercase() && b[1].is_ascii_lowercase(),
        5 => {
            b[0].is_ascii_lowercase()
                && b[1].is_ascii_lowercase()
                && b[2] == b'-'
                && b[3].is_ascii_uppercase()
                && b[4].is_ascii_uppercase()
        }
        _ => false,
    }
}

/// A localized value: a non-empty object whose keys are all locale codes. An empty object is not localized.
pub fn localized_map(value: &Value) -> Option<&Map<String, Value>> {
    let map = value.as_object()?;
    (!map.is_empty() && map.keys().all(|k| is_locale_code(k))).then_some(map)
}

/// Whether `value` is an array whose every node has a known inline type.
pub fn is_rich_nodes(value: &Value) -> bool {
    value
        .as_array()
        .is_some_and(|nodes| nodes.iter().all(is_known_node))
}

fn is_known_node(node: &Value) -> bool {
    matches!(
        node.get("type").and_then(Value::as_str),
        Some("text" | "link" | "mention" | "emoji" | "icon")
    )
}

/// Whether a rich-text prop value can be stored as rich text: a valid node array, or a localized map of them.
/// Anything else is stored as plain JSON (lossless fallback).
pub fn is_rich(value: &Value) -> bool {
    if value.is_array() {
        return is_rich_nodes(value);
    }
    localized_map(value).is_some_and(|map| map.values().all(is_rich_nodes))
}

/// Whether a plain-text prop value can be stored as plain text: a string, or a localized map of strings.
pub fn is_plain(value: &Value) -> bool {
    value.is_string() || localized_map(value).is_some_and(|map| map.values().all(Value::is_string))
}

/// Appends `nodes` to `text` as rich text (MAPPING section 3, import).
///
/// Returns [`Error::UnknownInline`] for an inline type this crate does not know. Callers check [`is_rich`] first, so
/// this only happens on a caller bug.
pub fn import_rich(text: &LoroText, nodes: &[Value]) -> Result<(), Error> {
    for node in nodes {
        let kind = node.get("type").and_then(Value::as_str).unwrap_or_default();
        match kind {
            "text" => put_text(text, str_field(node, "text"), &node_marks(node))?,
            "link" => {
                let start = text.len_unicode();
                let children: &[Value] = node
                    .get("children")
                    .and_then(Value::as_array)
                    .map_or(&[][..], Vec::as_slice);
                for child in children {
                    put_text(text, str_field(child, "text"), &node_marks(child))?;
                }
                let end = text.len_unicode();
                let mut value = Map::new();
                value.insert("url".into(), json!(str_field(node, "url")));
                if node.get("openInNewTab") == Some(&Value::Bool(true)) {
                    value.insert("openInNewTab".into(), Value::Bool(true));
                }
                if start < end {
                    map_loro(text.mark(start..end, "link", json_to_loro(&Value::Object(value))))?;
                }
            }
            "mention" => {
                let slug = str_field(node, "slug");
                let title = node.get("title").and_then(Value::as_str);
                // Title, else slug, else "@": the text is never empty.
                let label = title
                    .filter(|t| !t.is_empty())
                    .or(Some(slug).filter(|s| !s.is_empty()))
                    .unwrap_or("@");
                let mut value = Map::new();
                value.insert("entryId".into(), json!(str_field(node, "entryId")));
                value.insert("collection".into(), json!(str_field(node, "collection")));
                value.insert("slug".into(), json!(slug));
                if let Some(title) = title {
                    value.insert("title".into(), json!(title));
                }
                let start = text.len_unicode();
                map_loro(text.insert(start, label))?;
                let end = text.len_unicode();
                map_loro(text.mark(start..end, "mention", json_to_loro(&Value::Object(value))))?;
            }
            "emoji" => {
                let emoji = str_field(node, "emoji");
                if emoji.is_empty() {
                    continue;
                }
                let shortcode = node
                    .get("shortcode")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let start = text.len_unicode();
                map_loro(text.insert(start, emoji))?;
                let end = text.len_unicode();
                map_loro(text.mark(start..end, "emoji", json_to_loro(&json!(shortcode))))?;
            }
            "icon" => {
                let mut value = Map::new();
                value.insert("name".into(), json!(str_field(node, "name")));
                if let Some(label) = node.get("label").and_then(Value::as_str) {
                    value.insert("label".into(), json!(label));
                }
                let start = text.len_unicode();
                map_loro(text.insert(start, &ICON_CHAR.to_string()))?;
                map_loro(text.mark(
                    start..start + 1,
                    "icon",
                    json_to_loro(&Value::Object(value)),
                ))?;
            }
            other => return Err(Error::UnknownInline(other.to_string())),
        }
    }
    Ok(())
}

/// Appends `s` with the given marks. The four expand-after marks are set true or false explicitly.
fn put_text(text: &LoroText, s: &str, marks: &[&str]) -> Result<(), Error> {
    if s.is_empty() {
        return Ok(());
    }
    let start = text.len_unicode();
    map_loro(text.insert(start, s))?;
    let end = start + s.chars().count();
    for key in EXPAND_AFTER_MARKS {
        map_loro(text.mark(
            start..end,
            key,
            json_to_loro(&Value::Bool(marks.contains(&key))),
        ))?;
    }
    if marks.contains(&"code") {
        map_loro(text.mark(start..end, "code", json_to_loro(&Value::Bool(true))))?;
    }
    Ok(())
}

/// Reads `text` back as normal-form rich text nodes (MAPPING section 3 export, then section 4).
pub fn export_rich(text: &LoroText) -> Value {
    let mut raw: Vec<Value> = Vec::new();
    for item in text.to_delta() {
        let TextDelta::Insert { insert, attributes } = item else {
            continue;
        };
        let attrs = attributes.unwrap_or_default();
        let attr = |key: &str| attrs.get(key).map(loro_to_json);
        let marks: Vec<&str> = CANON_MARKS
            .iter()
            .copied()
            .filter(|m| matches!(attrs.get(*m), Some(LoroValue::Bool(true))))
            .collect();

        if let Some(mention) = attr("mention") {
            // A mention run is atomic: its own text is discarded.
            raw.push(with_type(mention, "mention"));
        } else if let Some(icon) = attr("icon") {
            for _ in insert.chars().filter(|c| *c == ICON_CHAR) {
                raw.push(with_type(icon.clone(), "icon"));
            }
        } else if let Some(emoji) = attr("emoji") {
            let mut node = Map::new();
            node.insert("type".into(), json!("emoji"));
            node.insert("emoji".into(), json!(insert));
            if let Some(code) = emoji.as_str().filter(|c| !c.is_empty()) {
                node.insert("shortcode".into(), json!(code));
            }
            raw.push(Value::Object(node));
        } else if let Some(link) = attr("link") {
            // A link run is a text child of a link node; normalization merges adjacent links.
            let mut node = link.as_object().cloned().unwrap_or_default();
            node.insert("type".into(), json!("link"));
            node.insert("children".into(), json!([text_node(&insert, &marks)]));
            raw.push(Value::Object(node));
        } else {
            raw.push(text_node(&insert, &marks));
        }
    }
    Value::Array(normalize_nodes(&raw))
}

/// Writes plain text into `text` (appends at position 0; callers write into a fresh text).
pub fn import_plain(text: &LoroText, s: &str) -> Result<(), Error> {
    if s.is_empty() {
        return Ok(());
    }
    map_loro(text.insert(0, s))
}

/// Plain text of `text`.
pub fn export_plain(text: &LoroText) -> String {
    text.to_string()
}

/// A JSON value as a Loro value (plain props, marks, metadata).
pub(crate) fn json_to_loro(value: &Value) -> LoroValue {
    match value {
        Value::Null => LoroValue::Null,
        Value::Bool(b) => LoroValue::Bool(*b),
        Value::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => LoroValue::I64(i),
            (None, Some(f)) => LoroValue::Double(f),
            (None, None) => LoroValue::Null,
        },
        // UNSURE: the payload constructors (string, list, map) are the best guess for loro 1.x. The `.into()` targets
        // may need `LoroValue::from(...)` or a different map type (FxHashMap) at build time.
        Value::String(s) => LoroValue::String(s.clone().into()),
        Value::Array(items) => {
            LoroValue::List(items.iter().map(json_to_loro).collect::<Vec<_>>().into())
        }
        Value::Object(map) => LoroValue::Map(
            map.iter()
                .map(|(k, v)| (k.clone(), json_to_loro(v)))
                .collect::<std::collections::HashMap<_, _>>()
                .into(),
        ),
    }
}

/// A Loro value as JSON.
pub(crate) fn loro_to_json(value: &LoroValue) -> Value {
    match value {
        LoroValue::Null => Value::Null,
        LoroValue::Bool(b) => Value::Bool(*b),
        LoroValue::Double(f) => serde_json::Number::from_f64(*f).map_or(Value::Null, Value::Number),
        LoroValue::I64(i) => Value::Number((*i).into()),
        LoroValue::String(s) => Value::String(s.to_string()),
        LoroValue::List(items) => Value::Array(items.iter().map(loro_to_json).collect()),
        LoroValue::Map(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.to_string(), loro_to_json(v)))
                .collect(),
        ),
        // Binary and container values never occur in the mapping.
        _ => Value::Null,
    }
}

fn str_field<'a>(node: &'a Value, key: &str) -> &'a str {
    node.get(key).and_then(Value::as_str).unwrap_or_default()
}

/// The canonical-order marks of an imported node that are `true`.
fn node_marks(node: &Value) -> Vec<&'static str> {
    let present: Vec<&str> = node
        .get("marks")
        .and_then(Value::as_array)
        .map(|marks| marks.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    CANON_MARKS
        .iter()
        .copied()
        .filter(|m| present.contains(m))
        .collect()
}

fn text_node(text: &str, marks: &[&str]) -> Value {
    let mut node = Map::new();
    node.insert("type".into(), json!("text"));
    node.insert("text".into(), json!(text));
    if !marks.is_empty() {
        node.insert("marks".into(), json!(marks));
    }
    Value::Object(node)
}

/// Adds `type` to an attribute object so it reads as an inline node.
fn with_type(value: Value, kind: &str) -> Value {
    let mut node = value.as_object().cloned().unwrap_or_default();
    node.insert("type".into(), json!(kind));
    Value::Object(node)
}
