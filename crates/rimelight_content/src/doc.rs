//! Page JSON <-> Loro document (MAPPING sections 2 and 5).
//!
//! Layout: root map `meta` (slug, type, template fields, `title:<locale>` and `description:<locale>` text, the
//! `properties` child map), and root tree `blocks` (fractional index, one node per block, node data in `get_meta`).
//!
//! Every document must be configured with [`configure`] before any edit or import. [`import_page`] and
//! [`fork_draft`] do that. A document loaded from bytes elsewhere must be configured by its caller.

use crate::error::{Error, map_loro};
use crate::registry::{TextKind, text_kind};
use crate::richtext::{self, is_locale_code, is_plain, is_rich, loro_to_json};
use loro::{
    Container, ExpandType, ExportMode, LoroDoc, LoroList, LoroMap, LoroText, LoroTree, StyleConfig,
    StyleConfigMap, TreeID, TreeParentId, ValueOrContainer,
};
use serde_json::{Map, Value, json};

/// Lock and visibility keys of a block: plain values in the tree node's data, only when present in the JSON.
const BLOCK_LOCK_KEYS: [&str; 5] = [
    "isTemplated",
    "templatedProps",
    "allowedRoles",
    "requiredPermission",
    "visibility",
];

/// Applies the text style config (MAPPING section 2). Required on every document before editing.
pub fn configure(doc: &LoroDoc) {
    let mut styles = StyleConfigMap::new();
    for key in ["bold", "italic", "underline", "strikethrough"] {
        styles.insert(
            key.into(),
            StyleConfig {
                expand: ExpandType::After,
            },
        );
    }
    for key in ["code", "link", "mention", "emoji", "icon"] {
        styles.insert(
            key.into(),
            StyleConfig {
                expand: ExpandType::None,
            },
        );
    }
    doc.config_text_style(styles);
}

/// Builds a Loro document from published page JSON. Import is deterministic: the same JSON gives the same document
/// shape. Input is not normalized first, so export returns the normal form of the input.
pub fn import_page(page: &Value) -> Result<LoroDoc, Error> {
    let doc = LoroDoc::new();
    configure(&doc);

    let meta_json = page
        .get("meta")
        .and_then(Value::as_object)
        .ok_or_else(|| shape("meta must be an object"))?;
    let content = page
        .get("content")
        .and_then(Value::as_object)
        .ok_or_else(|| shape("content must be an object"))?;
    let blocks = content
        .get("blocks")
        .and_then(Value::as_array)
        .ok_or_else(|| shape("content.blocks must be an array"))?;
    let properties = content
        .get("properties")
        .and_then(Value::as_object)
        .ok_or_else(|| shape("content.properties must be an object"))?;

    let meta = doc.get_map("meta");
    for (key, value) in meta_json {
        if key == "properties" {
            return Err(shape("meta.properties is reserved for content.properties"));
        }
        if (key == "title" || key == "description") && is_localized_strings(value) {
            if let Some(locales) = value.as_object() {
                for (locale, text) in locales {
                    let node = new_text(&meta, &format!("{key}:{locale}"))?;
                    richtext::import_plain(&node, text.as_str().unwrap_or_default())?;
                }
            }
        } else {
            set_plain(&meta, key, value)?;
        }
    }
    let props = map_loro(meta.insert_container("properties", LoroMap::new()))?;
    for (key, value) in properties {
        set_plain(&props, key, value)?;
    }

    let tree = doc.get_tree("blocks");
    tree.enable_fractional_index(0);
    for block in blocks {
        import_block(&tree, TreeParentId::Root, block)?;
    }
    doc.commit();
    Ok(doc)
}

/// Published page JSON from a Loro document. Output is in normal form (MAPPING section 4).
pub fn export_page(doc: &LoroDoc) -> Result<Value, Error> {
    doc.commit();
    let meta = doc.get_map("meta");

    let mut meta_out = Map::new();
    let mut titles = Map::new();
    let mut descriptions = Map::new();
    // UNSURE: `keys()` may yield `InternalString` rather than `String`; `to_string()` covers both.
    let keys: Vec<String> = meta.keys().map(|k| k.to_string()).collect();
    for key in keys {
        if key == "properties" {
            continue;
        }
        let Some(entry) = meta.get(&key) else {
            continue;
        };
        match entry {
            ValueOrContainer::Container(Container::Text(text)) => {
                let value = richtext::export_plain(&text);
                if let Some(locale) = key.strip_prefix("title:") {
                    titles.insert(locale.to_string(), json!(value));
                } else if let Some(locale) = key.strip_prefix("description:") {
                    descriptions.insert(locale.to_string(), json!(value));
                } else {
                    meta_out.insert(key, json!(value));
                }
            }
            ValueOrContainer::Value(value) => {
                meta_out.insert(key, loro_to_json(&value));
            }
            _ => {}
        }
    }
    if !titles.is_empty() {
        meta_out.insert("title".into(), Value::Object(titles));
    }
    if !descriptions.is_empty() {
        meta_out.insert("description".into(), Value::Object(descriptions));
    }

    let mut properties = Map::new();
    if let Some(ValueOrContainer::Container(Container::Map(props))) = meta.get("properties") {
        for key in props.keys() {
            let key = key.to_string();
            if let Some(ValueOrContainer::Value(value)) = props.get(&key) {
                properties.insert(key, loro_to_json(&value));
            }
        }
    }

    let tree = doc.get_tree("blocks");
    let mut blocks = Vec::new();
    for node in tree.children(TreeParentId::Root).unwrap_or_default() {
        blocks.push(export_block(&tree, node)?);
    }

    Ok(json!({
        "meta": Value::Object(meta_out),
        "content": { "blocks": blocks, "properties": Value::Object(properties) },
    }))
}

/// A draft: a fork of `main` with the text style config applied.
pub fn fork_draft(main: &LoroDoc) -> LoroDoc {
    let draft = main.fork();
    configure(&draft);
    draft
}

/// Publishes a draft: imports the draft's updates that `main` does not have yet (MAPPING section 5).
pub fn merge_draft(main: &LoroDoc, draft: &LoroDoc) -> Result<(), Error> {
    main.commit();
    draft.commit();
    let updates = map_loro(draft.export(ExportMode::updates(&main.oplog_vv())))?;
    map_loro(main.import(&updates))?;
    Ok(())
}

/// Brings a draft up to date with `main` (MAPPING section 5).
pub fn catch_up(draft: &LoroDoc, main: &LoroDoc) -> Result<(), Error> {
    main.commit();
    draft.commit();
    let updates = map_loro(main.export(ExportMode::updates(&draft.oplog_vv())))?;
    map_loro(draft.import(&updates))?;
    Ok(())
}

/// The data map of the block with `block_id`, searched through the whole tree.
pub fn find_block_meta(doc: &LoroDoc, block_id: &str) -> Option<LoroMap> {
    let tree = doc.get_tree("blocks");
    let node = find_node(&tree, &TreeParentId::Root, block_id)?;
    tree.get_meta(node).ok()
}

/// The Loro text under `key` (for example `t:text` or `t:text:en`) of the block with `block_id`.
pub fn block_text(doc: &LoroDoc, block_id: &str, key: &str) -> Option<LoroText> {
    let meta = find_block_meta(doc, block_id)?;
    match meta.get(key)? {
        ValueOrContainer::Container(Container::Text(text)) => Some(text),
        _ => None,
    }
}

pub(crate) fn find_node(tree: &LoroTree, parent: &TreeParentId, block_id: &str) -> Option<TreeID> {
    for node in tree.children(*parent).unwrap_or_default() {
        let Ok(meta) = tree.get_meta(node) else {
            continue;
        };
        if string_entry(&meta, "id").as_deref() == Some(block_id) {
            return Some(node);
        }
        if let Some(found) = find_node(tree, &TreeParentId::Node(node), block_id) {
            return Some(found);
        }
    }
    None
}

fn import_block(tree: &LoroTree, parent: TreeParentId, block: &Value) -> Result<(), Error> {
    let node = map_loro(tree.create(parent))?;
    fill_block(tree, node, block)
}

/// Writes the id, type, lock keys, props and children of `block` into the tree node `node` (created by the caller).
pub(crate) fn fill_block(tree: &LoroTree, node: TreeID, block: &Value) -> Result<(), Error> {
    let obj = block
        .as_object()
        .ok_or_else(|| shape("block must be an object"))?;
    let id = str_field(obj, "id")?;
    let block_type = str_field(obj, "type")?;
    let props = obj
        .get("props")
        .and_then(Value::as_object)
        .ok_or_else(|| shape("block props must be an object"))?;

    let meta = map_loro(tree.get_meta(node))?;
    set_plain(&meta, "id", &json!(id))?;
    set_plain(&meta, "type", &json!(block_type))?;
    for key in BLOCK_LOCK_KEYS {
        if let Some(value) = obj.get(key) {
            set_plain(&meta, key, value)?;
        }
    }
    for (key, value) in props {
        import_prop(&meta, block_type, key, value)?;
    }
    if let Some(children) = obj.get("children").and_then(Value::as_array) {
        for child in children {
            import_block(tree, TreeParentId::Node(node), child)?;
        }
    }
    Ok(())
}

/// Stores one prop by the rules of MAPPING section 2. A value that does not fit its text kind is stored as `p:`.
pub(crate) fn import_prop(
    meta: &LoroMap,
    block_type: &str,
    key: &str,
    value: &Value,
) -> Result<(), Error> {
    if block_type == "TableBlock" && key == "rows" && rows_ok(value) {
        return import_rows(meta, value);
    }
    match text_kind(block_type, key) {
        Some(TextKind::Rich) if is_rich(value) => {
            if let Some(locales) = richtext::localized_map(value) {
                for (locale, nodes) in locales {
                    let text = new_text(meta, &format!("t:{key}:{locale}"))?;
                    richtext::import_rich(&text, as_nodes(nodes))?;
                }
            } else {
                let text = new_text(meta, &format!("t:{key}"))?;
                richtext::import_rich(&text, as_nodes(value))?;
            }
            Ok(())
        }
        Some(TextKind::Plain) if is_plain(value) => {
            if let Value::String(s) = value {
                let text = new_text(meta, &format!("t:{key}"))?;
                richtext::import_plain(&text, s)?;
            } else if let Some(locales) = richtext::localized_map(value) {
                for (locale, s) in locales {
                    let text = new_text(meta, &format!("t:{key}:{locale}"))?;
                    richtext::import_plain(&text, s.as_str().unwrap_or_default())?;
                }
            }
            Ok(())
        }
        _ => set_plain(meta, &format!("p:{key}"), value),
    }
}

/// TableBlock rows: a Loro list of maps, one Loro text per column.
fn import_rows(meta: &LoroMap, rows: &Value) -> Result<(), Error> {
    let list = map_loro(meta.insert_container("rows", LoroList::new()))?;
    let rows = rows.as_array().map_or(&[][..], Vec::as_slice);
    for (index, row) in rows.iter().enumerate() {
        let row_map = map_loro(list.insert_container(index, LoroMap::new()))?;
        if let Some(cells) = row.as_object() {
            for (column, cell) in cells {
                let text = map_loro(row_map.insert_container(column, LoroText::new()))?;
                richtext::import_plain(&text, cell.as_str().unwrap_or_default())?;
            }
        }
    }
    Ok(())
}

fn rows_ok(value: &Value) -> bool {
    value.as_array().is_some_and(|rows| {
        rows.iter().all(|row| {
            row.as_object()
                .is_some_and(|cells| cells.values().all(Value::is_string))
        })
    })
}

pub(crate) fn export_block(tree: &LoroTree, node: TreeID) -> Result<Value, Error> {
    let meta = map_loro(tree.get_meta(node))?;
    let mut block = Map::new();
    let mut props = Map::new();
    // (prop, locale, text) for every `t:` key.
    let mut texts: Vec<(String, Option<String>, LoroText)> = Vec::new();

    // UNSURE: `keys()` may yield `InternalString`; `to_string()` covers both.
    let keys: Vec<String> = meta.keys().map(|k| k.to_string()).collect();
    for key in keys {
        let Some(entry) = meta.get(&key) else {
            continue;
        };
        if key == "rows" {
            if let ValueOrContainer::Container(Container::List(list)) = entry {
                props.insert("rows".into(), export_rows(&list));
            }
        } else if let Some(prop) = key.strip_prefix("p:") {
            if let ValueOrContainer::Value(value) = entry {
                props.insert(prop.to_string(), loro_to_json(&value));
            }
        } else if let Some(rest) = key.strip_prefix("t:") {
            if let ValueOrContainer::Container(Container::Text(text)) = entry {
                match split_locale(rest) {
                    Some((prop, locale)) => {
                        texts.push((prop.to_string(), Some(locale.to_string()), text))
                    }
                    None => texts.push((rest.to_string(), None, text)),
                }
            }
        } else if (key == "id" || key == "type" || BLOCK_LOCK_KEYS.contains(&key.as_str()))
            && let ValueOrContainer::Value(value) = entry
        {
            block.insert(key.clone(), loro_to_json(&value));
        }
    }

    let block_type = block
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    for (prop, locale, text) in texts {
        let value = if text_kind(&block_type, &prop) == Some(TextKind::Rich) {
            richtext::export_rich(&text)
        } else {
            json!(richtext::export_plain(&text))
        };
        match locale {
            None => {
                props.insert(prop, value);
            }
            Some(locale) => {
                let slot = props
                    .entry(prop)
                    .or_insert_with(|| Value::Object(Map::new()));
                if let Some(locales) = slot.as_object_mut() {
                    locales.insert(locale, value);
                }
            }
        }
    }

    let mut children = Vec::new();
    for child in tree.children(TreeParentId::Node(node)).unwrap_or_default() {
        children.push(export_block(tree, child)?);
    }
    block.insert("props".into(), Value::Object(props));
    if !children.is_empty() {
        block.insert("children".into(), Value::Array(children));
    }
    Ok(Value::Object(block))
}

fn export_rows(list: &LoroList) -> Value {
    let mut rows = Vec::new();
    for index in 0..list.len() {
        let Some(ValueOrContainer::Container(Container::Map(row))) = list.get(index) else {
            continue;
        };
        let mut cells = Map::new();
        for column in row.keys() {
            let column = column.to_string();
            if let Some(ValueOrContainer::Container(Container::Text(text))) = row.get(&column) {
                cells.insert(column, json!(richtext::export_plain(&text)));
            }
        }
        rows.push(Value::Object(cells));
    }
    Value::Array(rows)
}

/// `t:<prop>:<locale>` -> (`prop`, `locale`), when the suffix is a locale code.
fn split_locale(rest: &str) -> Option<(&str, &str)> {
    let (prop, locale) = rest.rsplit_once(':')?;
    is_locale_code(locale).then_some((prop, locale))
}

fn is_localized_strings(value: &Value) -> bool {
    value
        .as_object()
        .is_some_and(|map| !map.is_empty() && map.values().all(Value::is_string))
}

fn as_nodes(value: &Value) -> &[Value] {
    value.as_array().map_or(&[][..], Vec::as_slice)
}

fn new_text(map: &LoroMap, key: &str) -> Result<LoroText, Error> {
    map_loro(map.insert_container(key, LoroText::new()))
}

fn set_plain(map: &LoroMap, key: &str, value: &Value) -> Result<(), Error> {
    map_loro(map.insert(key, richtext::json_to_loro(value)))
}

fn string_entry(map: &LoroMap, key: &str) -> Option<String> {
    match map.get(key)? {
        ValueOrContainer::Value(value) => loro_to_json(&value).as_str().map(str::to_string),
        _ => None,
    }
}

fn str_field<'a>(obj: &'a Map<String, Value>, key: &str) -> Result<&'a str, Error> {
    obj.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| shape(&format!("block {key} must be a string")))
}

fn shape(message: &str) -> Error {
    Error::Shape(message.to_string())
}
