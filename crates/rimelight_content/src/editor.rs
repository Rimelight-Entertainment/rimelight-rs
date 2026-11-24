//! The editing surface over the MAPPING document (EDITOR_API.md, module `editor`).
//!
//! Every operation edits the Loro document in the layout MAPPING.md defines, through the helpers in `doc.rs` and
//! `richtext.rs`. Public offsets are UTF-8 byte offsets into the plain text of one field; they are converted to Loro
//! unicode positions here. Every write commits. Registry rules (`registry.rs`) and template locks are checked before
//! anything is changed, so a failed call leaves the document as it was.

use crate::doc;
use crate::error::{Error, map_loro};
use crate::registry::{TextKind, allowed_child, block_def, text_kind};
use crate::richtext::{self, is_locale_code, json_to_loro, loro_to_json, runs_to_nodes};
use loro::{
    Container, LoroDoc, LoroList, LoroMap, LoroText, LoroTree, LoroValue, TextDelta, TreeID,
    TreeParentId, UndoManager, ValueOrContainer,
};
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;
use std::ops::Range;
use uuid::Uuid;

/// Lock and visibility keys of a block (MAPPING section 2). Set through `set_access`, not `set_prop`.
const BLOCK_LOCK_KEYS: [&str; 5] = [
    "isTemplated",
    "templatedProps",
    "allowedRoles",
    "requiredPermission",
    "visibility",
];

/// The list block types. A list item nests under another item through a list block.
const LIST_TYPES: [&str; 2] = ["UnorderedListBlock", "OrderedListBlock"];

/// The editor's document: a configured `LoroDoc` plus an undo manager that tracks local changes.
pub struct EditorDoc {
    doc: LoroDoc,
    history: UndoManager,
}

/// One block as the editor lists it: the tree flattened in display order (depth first).
#[derive(Debug, Clone, PartialEq)]
pub struct BlockRow {
    pub id: String,
    pub block_type: String,
    pub depth: usize,
    pub parent: Option<String>,
    /// Position among its siblings.
    pub index: usize,
    pub has_children: bool,
    /// The registry's inline text prop (`CodeBlock`: `code`).
    pub inline_field: Option<String>,
    /// Whether the inline field is rich text.
    pub rich: bool,
    /// `SectionBlock`: 2 + the number of section ancestors, capped at 6.
    pub heading_level: Option<u8>,
    /// The plain (`p:`) props. Text props are read with [`EditorDoc::text`].
    pub props: Map<String, Value>,
    pub access: BlockAccess,
    /// `isTemplated`.
    pub templated: bool,
    pub locked_props: LockedProps,
    /// Registry `webOnly`.
    pub web_only: bool,
}

/// Visibility, roles and permission of a block. `None` / empty means inherit.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BlockAccess {
    /// `"internal"` or `"public"`.
    pub visibility: Option<String>,
    pub allowed_roles: Vec<String>,
    pub required_permission: Option<String>,
}

/// Which props a template locks (`templatedProps`).
#[derive(Debug, Clone, PartialEq)]
pub enum LockedProps {
    None,
    All,
    Keys(Vec<String>),
}

/// The plain text of one field and the spans that cover every byte of it, in order.
#[derive(Debug, Clone, PartialEq)]
pub struct TextValue {
    pub plain: String,
    pub spans: Vec<Span>,
}

/// One run of text with the same attributes.
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub range: Range<usize>,
    pub marks: Vec<MarkKind>,
    pub link: Option<String>,
    pub mention: Option<Mention>,
    pub emoji: bool,
    pub icon: Option<String>,
}

/// Inline text marks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkKind {
    Bold,
    Italic,
    Underline,
    Strikethrough,
    Code,
}

impl MarkKind {
    /// Every mark, in canonical order.
    pub const ALL: [MarkKind; 5] = [
        MarkKind::Bold,
        MarkKind::Italic,
        MarkKind::Underline,
        MarkKind::Strikethrough,
        MarkKind::Code,
    ];

    /// The Loro style key of this mark (MAPPING section 2).
    pub fn key(self) -> &'static str {
        match self {
            MarkKind::Bold => "bold",
            MarkKind::Italic => "italic",
            MarkKind::Underline => "underline",
            MarkKind::Strikethrough => "strikethrough",
            MarkKind::Code => "code",
        }
    }
}

/// A mention: one atomic run in rich text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mention {
    pub entry_id: String,
    pub collection: String,
    pub slug: String,
    pub title: Option<String>,
}

impl EditorDoc {
    /// Wraps a document. The text style config is applied, the block tree gets fractional indexes if it has none, and
    /// the undo manager starts tracking local changes from here on.
    pub fn from_doc(doc: LoroDoc) -> Self {
        doc::configure(&doc);
        let tree = doc.get_tree("blocks");
        if !tree.is_fractional_index_enabled() {
            tree.enable_fractional_index(0);
        }
        let history = UndoManager::new(&doc);
        Self { doc, history }
    }

    /// A document from its snapshot bytes (see [`EditorDoc::from_doc`]).
    pub fn from_snapshot(bytes: &[u8]) -> Result<Self, Error> {
        Ok(Self::from_doc(map_loro(LoroDoc::from_snapshot(bytes))?))
    }

    /// The underlying Loro document.
    pub fn doc(&self) -> &LoroDoc {
        &self.doc
    }

    // ----- reading -----

    /// The block tree flattened in display order. BlockRow carries no text, so `_locale` does not change the result.
    pub fn rows(&self, _locale: &str) -> Vec<BlockRow> {
        let tree = self.tree();
        let mut out = Vec::new();
        collect_rows(&tree, TreeParentId::Root, None, 0, 0, &mut out);
        out
    }

    /// One row by block id.
    pub fn row(&self, id: &str, locale: &str) -> Option<BlockRow> {
        self.rows(locale).into_iter().find(|row| row.id == id)
    }

    /// The text of one field in one locale. An unknown block or field gives an empty value.
    pub fn text(&self, id: &str, field: &str, locale: &str) -> TextValue {
        let tree = self.tree();
        let text = doc::find_node(&tree, &TreeParentId::Root, id)
            .and_then(|node| tree.get_meta(node).ok())
            .and_then(|meta| existing_text(&meta, field, locale));
        match text {
            Some(text) => text_value(&text),
            None => TextValue {
                plain: String::new(),
                spans: Vec::new(),
            },
        }
    }

    /// The published form of the whole page (`export_page`).
    pub fn page(&self) -> Result<Value, Error> {
        doc::export_page(&self.doc)
    }

    /// Every locale that a localized field or the title or description uses, sorted, with `en` first.
    pub fn locales(&self) -> Vec<String> {
        let mut found = BTreeSet::new();
        let root = self.root_meta();
        for key in root.keys() {
            let key = key.to_string();
            for prefix in ["title:", "description:"] {
                if let Some(locale) = key.strip_prefix(prefix)
                    && is_locale_code(locale)
                {
                    found.insert(locale.to_string());
                }
            }
        }
        collect_locales(&self.tree(), TreeParentId::Root, &mut found);
        // Text stored without a locale is the default locale's, so every page has the default locale.
        found.insert(DEFAULT_LOCALE.to_string());
        let mut out: Vec<String> = found.into_iter().collect();
        if let Some(pos) = out.iter().position(|locale| locale == "en") {
            let en = out.remove(pos);
            out.insert(0, en);
        }
        out
    }

    // ----- text (every call commits) -----

    /// Replaces the text of a field with `new_plain`, changing only the differing middle. A mention touched by the
    /// change is deleted whole.
    pub fn replace_text(
        &mut self,
        id: &str,
        field: &str,
        locale: &str,
        new_plain: &str,
    ) -> Result<(), Error> {
        self.write(|d| d.replace_text_op(id, field, locale, new_plain))
    }

    /// Toggles one mark over a byte range: removes it if the whole range has it, adds it otherwise.
    pub fn toggle_mark(
        &mut self,
        id: &str,
        field: &str,
        locale: &str,
        range: Range<usize>,
        mark: MarkKind,
    ) -> Result<(), Error> {
        self.write(|d| d.toggle_mark_op(id, field, locale, range, mark))
    }

    /// Sets (`Some`) or removes (`None`) the link over a byte range.
    pub fn set_link(
        &mut self,
        id: &str,
        field: &str,
        locale: &str,
        range: Range<usize>,
        url: Option<&str>,
    ) -> Result<(), Error> {
        self.write(|d| d.set_link_op(id, field, locale, range, url))
    }

    /// Inserts a mention at byte offset `at`. Returns the caret after it.
    pub fn insert_mention(
        &mut self,
        id: &str,
        field: &str,
        locale: &str,
        at: usize,
        mention: Mention,
    ) -> Result<usize, Error> {
        self.write(|d| d.insert_mention_op(id, field, locale, at, &mention))
    }

    /// Inserts an emoji at byte offset `at`. Returns the caret after it.
    pub fn insert_emoji(
        &mut self,
        id: &str,
        field: &str,
        locale: &str,
        at: usize,
        emoji: &str,
        shortcode: &str,
    ) -> Result<usize, Error> {
        self.write(|d| d.insert_emoji_op(id, field, locale, at, emoji, shortcode))
    }

    // ----- blocks (every call commits) -----

    /// Inserts a new block of `block_type` under `parent` (`None` = the page root), after the sibling `after` (`None` =
    /// first child). Defaults and initial children come from the registry, with fresh UUID v4 ids. Returns the new id.
    pub fn insert_block(
        &mut self,
        parent: Option<&str>,
        after: Option<&str>,
        block_type: &str,
    ) -> Result<String, Error> {
        self.write(|d| d.insert_block_op(parent, after, block_type))
    }

    /// Splits a text block at byte offset `at`: the text after it moves into a new block after this one. A
    /// `SectionBlock` title moves into a new `ParagraphBlock` placed first among the section's children. Returns the
    /// new id.
    pub fn split_block(
        &mut self,
        id: &str,
        field: &str,
        locale: &str,
        at: usize,
    ) -> Result<String, Error> {
        self.write(|d| d.split_block_op(id, field, locale, at))
    }

    /// Appends this block's text to the previous sibling's inline text and deletes this block. The children of this
    /// block move to the previous sibling. Returns `None` when there is no previous text block.
    pub fn merge_into_previous(
        &mut self,
        id: &str,
        field: &str,
        locale: &str,
    ) -> Result<Option<(String, usize)>, Error> {
        self.write(|d| d.merge_op(id, field, locale))
    }

    /// Deletes blocks with their subtrees. All ids are checked first; a block inside another listed block goes with
    /// it.
    pub fn delete_blocks(&mut self, ids: &[String]) -> Result<(), Error> {
        self.write(|d| d.delete_blocks_op(ids))
    }

    /// Moves a block under `parent` (`None` = the page root) at `index` among its children.
    pub fn move_block(
        &mut self,
        id: &str,
        parent: Option<&str>,
        index: usize,
    ) -> Result<(), Error> {
        self.write(|d| d.move_block_op(id, parent, index))
    }

    /// Moves a block before its previous sibling. A block that is already first does nothing.
    pub fn move_up(&mut self, id: &str) -> Result<(), Error> {
        self.write(|d| d.move_up_op(id))
    }

    /// Moves a block after its next sibling. A block that is already last does nothing.
    pub fn move_down(&mut self, id: &str) -> Result<(), Error> {
        self.write(|d| d.move_down_op(id))
    }

    /// Makes a block the last child of its previous sibling. A list item goes into a nested list of the previous
    /// item (created if needed), because a `ListItemBlock` may not hold another item directly.
    pub fn indent(&mut self, id: &str) -> Result<(), Error> {
        self.write(|d| d.indent_op(id))
    }

    /// Moves a block to be the next sibling of its parent. A list item moves out of its nested list, after the item
    /// that holds that list.
    pub fn outdent(&mut self, id: &str) -> Result<(), Error> {
        self.write(|d| d.outdent_op(id))
    }

    /// Changes the block type in place. The inline text is carried over, and children that the new type does not
    /// allow are deleted. Other props take the new type's defaults.
    pub fn turn_into(&mut self, id: &str, block_type: &str) -> Result<(), Error> {
        self.write(|d| d.turn_into_op(id, block_type))
    }

    /// Copies a block and its subtree with fresh ids, placed right after it. Returns the new root id.
    pub fn duplicate(&mut self, id: &str) -> Result<String, Error> {
        self.write(|d| d.duplicate_op(id))
    }

    /// Sets a prop. A text prop given a string or rich nodes becomes Loro text; anything else is plain JSON.
    pub fn set_prop(&mut self, id: &str, key: &str, value: Value) -> Result<(), Error> {
        self.write(|d| d.set_prop_op(id, key, &value))
    }

    /// Sets visibility, allowed roles and required permission of a block.
    pub fn set_access(&mut self, id: &str, access: BlockAccess) -> Result<(), Error> {
        self.write(|d| d.set_access_op(id, &access))
    }

    /// Sets the page title or description (`key` = `"title"` or `"description"`) in one locale.
    pub fn set_meta_text(&mut self, key: &str, locale: &str, value: &str) -> Result<(), Error> {
        self.write(|d| d.set_meta_text_op(key, locale, value))
    }

    /// Sets one page property (`meta.properties`). A null value removes it.
    pub fn set_property(&mut self, key: &str, value: Value) -> Result<(), Error> {
        self.write(|d| d.set_property_op(key, &value))
    }

    // ----- tables (every call commits) -----

    /// Sets one cell of a `TableBlock`. `row` is the row index and `column` the column key.
    pub fn table_set_cell(
        &mut self,
        id: &str,
        row: usize,
        column: &str,
        value: &str,
    ) -> Result<(), Error> {
        self.write(|d| d.table_set_cell_op(id, row, column, value))
    }

    /// Inserts an empty row at `at`, with one empty cell per column.
    pub fn table_insert_row(&mut self, id: &str, at: usize) -> Result<(), Error> {
        self.write(|d| d.table_insert_row_op(id, at))
    }

    /// Deletes the row at `at`.
    pub fn table_delete_row(&mut self, id: &str, at: usize) -> Result<(), Error> {
        self.write(|d| d.table_delete_row_op(id, at))
    }

    // ----- history (local changes only) -----

    /// Undoes the last local change. Returns false when there is nothing to undo.
    pub fn undo(&mut self) -> bool {
        self.history.undo().unwrap_or(false)
    }

    /// Redoes the last undone change. Returns false when there is nothing to redo.
    pub fn redo(&mut self) -> bool {
        self.history.redo().unwrap_or(false)
    }

    // ----- internals -----

    /// Runs one write and commits it, whether it succeeded or not, so partial state never stays pending.
    fn write<T>(&mut self, op: impl FnOnce(&EditorDoc) -> Result<T, Error>) -> Result<T, Error> {
        let out = op(self);
        self.doc.commit();
        out
    }

    fn tree(&self) -> LoroTree {
        self.doc.get_tree("blocks")
    }

    fn root_meta(&self) -> LoroMap {
        self.doc.get_map("meta")
    }

    fn find(&self, id: &str) -> Result<TreeID, Error> {
        doc::find_node(&self.tree(), &TreeParentId::Root, id)
            .ok_or_else(|| Error::NotFound(format!("block {id}")))
    }

    fn meta_of(&self, node: TreeID) -> Result<LoroMap, Error> {
        map_loro(self.tree().get_meta(node))
    }

    fn type_of(&self, node: TreeID) -> Result<String, Error> {
        Ok(string_entry(&self.meta_of(node)?, "type").unwrap_or_default())
    }

    /// The type of a parent: `None` for the page root.
    fn parent_type(&self, parent: TreeParentId) -> Result<Option<String>, Error> {
        match parent {
            TreeParentId::Node(node) => self.type_of(node).map(Some),
            _ => Ok(None),
        }
    }

    fn parent_of(&self, node: TreeID) -> TreeParentId {
        self.tree().parent(node).unwrap_or(TreeParentId::Root)
    }

    fn siblings(&self, parent: TreeParentId) -> Vec<TreeID> {
        self.tree().children(parent).unwrap_or_default()
    }

    /// The parent for `parent` as given to the public API: `None` is the root, `Some` a block.
    fn resolve_parent(
        &self,
        parent: Option<&str>,
    ) -> Result<(TreeParentId, Option<String>), Error> {
        match parent {
            None => Ok((TreeParentId::Root, None)),
            Some(id) => {
                let node = self.find(id)?;
                Ok((TreeParentId::Node(node), Some(self.type_of(node)?)))
            }
        }
    }

    fn is_ancestor(&self, ancestor: TreeID, node: TreeID) -> bool {
        let tree = self.tree();
        let mut current = node;
        while let Some(TreeParentId::Node(parent)) = tree.parent(current) {
            if parent == ancestor {
                return true;
            }
            current = parent;
        }
        false
    }

    /// Deleting or moving a templated block is locked.
    fn guard_delete(&self, node: TreeID) -> Result<(), Error> {
        if is_templated(&self.meta_of(node)?) {
            Err(Error::Locked)
        } else {
            Ok(())
        }
    }

    /// Setting a prop of a templated block is locked when `templatedProps` is `true` or lists the prop.
    fn guard_prop(&self, node: TreeID, prop: &str) -> Result<(), Error> {
        let meta = self.meta_of(node)?;
        if !is_templated(&meta) {
            return Ok(());
        }
        let locked = match value_entry(&meta, "templatedProps") {
            Some(Value::Bool(all)) => all,
            Some(Value::Array(keys)) => keys.iter().any(|key| key.as_str() == Some(prop)),
            _ => false,
        };
        if locked { Err(Error::Locked) } else { Ok(()) }
    }

    /// The kind of a text field of the block's type, or NotAllowed.
    fn text_field(&self, node: TreeID, field: &str) -> Result<(String, TextKind), Error> {
        let ty = self.type_of(node)?;
        match text_kind(&ty, field) {
            Some(kind) => Ok((ty, kind)),
            None => Err(Error::NotAllowed(format!(
                "{field} is not a text prop of {ty}"
            ))),
        }
    }

    /// A rich text field of the block's type, or NotAllowed.
    fn rich_field(&self, node: TreeID, field: &str) -> Result<(), Error> {
        match self.text_field(node, field)? {
            (_, TextKind::Rich) => Ok(()),
            (ty, TextKind::Plain) => {
                Err(Error::NotAllowed(format!("{field} of {ty} is plain text")))
            }
        }
    }

    fn replace_text_op(
        &self,
        id: &str,
        field: &str,
        locale: &str,
        new_plain: &str,
    ) -> Result<(), Error> {
        let node = self.find(id)?;
        self.text_field(node, field)?;
        self.guard_prop(node, field)?;
        let text = text_or_create(&self.meta_of(node)?, field, locale)?;
        apply_diff(&text, new_plain)
    }

    fn toggle_mark_op(
        &self,
        id: &str,
        field: &str,
        locale: &str,
        range: Range<usize>,
        mark: MarkKind,
    ) -> Result<(), Error> {
        let node = self.find(id)?;
        self.rich_field(node, field)?;
        self.guard_prop(node, field)?;
        let text = existing_text(&self.meta_of(node)?, field, locale)
            .ok_or_else(|| Error::NotFound(format!("{field} of {id}")))?;
        let value = text_value(&text);
        let (start, end) = unicode_range(&value.plain, &range)?;
        if start == end {
            return Ok(());
        }
        let all_marked = value
            .spans
            .iter()
            .filter(|span| span.range.start < range.end && span.range.end > range.start)
            .all(|span| span.marks.contains(&mark));
        if all_marked {
            map_loro(text.unmark(start..end, mark.key()))
        } else {
            map_loro(text.mark(start..end, mark.key(), LoroValue::Bool(true)))
        }
    }

    fn set_link_op(
        &self,
        id: &str,
        field: &str,
        locale: &str,
        range: Range<usize>,
        url: Option<&str>,
    ) -> Result<(), Error> {
        let node = self.find(id)?;
        self.rich_field(node, field)?;
        self.guard_prop(node, field)?;
        let text = existing_text(&self.meta_of(node)?, field, locale)
            .ok_or_else(|| Error::NotFound(format!("{field} of {id}")))?;
        let plain = text.to_string();
        let (start, end) = unicode_range(&plain, &range)?;
        if start == end {
            return Ok(());
        }
        match url {
            Some(url) => {
                map_loro(text.mark(start..end, "link", json_to_loro(&json!({ "url": url }))))
            }
            None => map_loro(text.unmark(start..end, "link")),
        }
    }

    fn insert_mention_op(
        &self,
        id: &str,
        field: &str,
        locale: &str,
        at: usize,
        mention: &Mention,
    ) -> Result<usize, Error> {
        let node = self.find(id)?;
        self.rich_field(node, field)?;
        self.guard_prop(node, field)?;
        let text = text_or_create(&self.meta_of(node)?, field, locale)?;
        let at_unicode = byte_to_unicode(&text.to_string(), at)?;
        let label = mention_label(mention);
        let label_len = label.chars().count();
        map_loro(text.insert(at_unicode, &label))?;

        let mut value = Map::new();
        value.insert("entryId".into(), Value::String(mention.entry_id.clone()));
        value.insert(
            "collection".into(),
            Value::String(mention.collection.clone()),
        );
        value.insert("slug".into(), Value::String(mention.slug.clone()));
        if let Some(title) = &mention.title {
            value.insert("title".into(), Value::String(title.clone()));
        }
        map_loro(text.mark(
            at_unicode..at_unicode + label_len,
            "mention",
            json_to_loro(&Value::Object(value)),
        ))?;
        Ok(at + label.len())
    }

    fn insert_emoji_op(
        &self,
        id: &str,
        field: &str,
        locale: &str,
        at: usize,
        emoji: &str,
        shortcode: &str,
    ) -> Result<usize, Error> {
        let node = self.find(id)?;
        self.rich_field(node, field)?;
        self.guard_prop(node, field)?;
        if emoji.is_empty() {
            return Ok(at);
        }
        let text = text_or_create(&self.meta_of(node)?, field, locale)?;
        let at_unicode = byte_to_unicode(&text.to_string(), at)?;
        map_loro(text.insert(at_unicode, emoji))?;
        map_loro(text.mark(
            at_unicode..at_unicode + emoji.chars().count(),
            "emoji",
            json_to_loro(&Value::String(shortcode.to_string())),
        ))?;
        Ok(at + emoji.len())
    }

    fn insert_block_op(
        &self,
        parent: Option<&str>,
        after: Option<&str>,
        block_type: &str,
    ) -> Result<String, Error> {
        let tree = self.tree();
        let (parent_tid, parent_ty) = self.resolve_parent(parent)?;
        if !allowed_child(parent_ty.as_deref(), block_type) {
            return Err(Error::NotAllowed(format!(
                "{block_type} cannot sit under {}",
                parent_ty.as_deref().unwrap_or("the page root")
            )));
        }
        let def = block_def(block_type)
            .ok_or_else(|| Error::NotAllowed(format!("unknown block type {block_type}")))?;
        // Web-only blocks are fully editable in the native editor (decision 0049, answer 8); their preview is a web view.
        let index = match after {
            None => 0,
            Some(after_id) => {
                let after_node = self.find(after_id)?;
                if self.parent_of(after_node) != parent_tid {
                    return Err(Error::NotAllowed(format!(
                        "{after_id} is not a child of the parent"
                    )));
                }
                self.siblings(parent_tid)
                    .iter()
                    .position(|node| *node == after_node)
                    .map_or(0, |pos| pos + 1)
            }
        };
        let new_id = new_id();
        let mut block = json!({
            "id": new_id,
            "type": block_type,
            "props": Value::Object(def.default_props.clone()),
        });
        if let Some(initial) = &def.initial_children {
            let mut children = Value::Array(initial.clone());
            if let Value::Array(items) = &mut children {
                for item in items {
                    refresh_ids(item);
                }
            }
            block["children"] = children;
        }
        let node = map_loro(tree.create_at(parent_tid, index))?;
        doc::fill_block(&tree, node, &block)?;
        Ok(new_id)
    }

    fn split_block_op(
        &self,
        id: &str,
        field: &str,
        locale: &str,
        at: usize,
    ) -> Result<String, Error> {
        let tree = self.tree();
        let node = self.find(id)?;
        self.guard_prop(node, field)?;
        let meta = self.meta_of(node)?;
        let ty = string_entry(&meta, "type").unwrap_or_default();
        text_kind(&ty, field)
            .ok_or_else(|| Error::NotAllowed(format!("{field} is not a text prop of {ty}")))?;

        let section_title = ty == "SectionBlock" && field == "title";
        if !section_title && inline_field_of(&ty).as_deref() != Some(field) {
            return Err(Error::NotAllowed(format!(
                "{field} is not the inline text of {ty}"
            )));
        }
        let (parent, index, new_type) = if section_title {
            (TreeParentId::Node(node), 0, "ParagraphBlock".to_string())
        } else {
            let parent = self.parent_of(node);
            let index = self
                .siblings(parent)
                .iter()
                .position(|sibling| *sibling == node)
                .map_or(1, |pos| pos + 1);
            (parent, index, ty.clone())
        };
        let parent_ty = self.parent_type(parent)?;
        if !allowed_child(parent_ty.as_deref(), &new_type) {
            return Err(Error::NotAllowed(format!(
                "{new_type} cannot sit under the parent"
            )));
        }
        let target_field = inline_field_of(&new_type)
            .ok_or_else(|| Error::NotAllowed(format!("{new_type} has no inline text")))?;
        let target_kind = text_kind(&new_type, &target_field).ok_or_else(|| {
            Error::NotAllowed(format!("{target_field} is not a text prop of {new_type}"))
        })?;

        // Read the tail before anything changes.
        let source = existing_text(&meta, field, locale);
        let (cut, tail_nodes, tail_plain) = match &source {
            Some(text) => {
                let cut = byte_to_unicode(&text.to_string(), at)?;
                let nodes = runs_to_nodes(tail_from(text.to_delta(), cut));
                let plain = plain_of_nodes(&nodes);
                (cut, nodes, plain)
            }
            None => (0, json!([]), String::new()),
        };

        // Create the new block with the same localization shape as the source field.
        let empty = match target_kind {
            TextKind::Rich => json!([]),
            TextKind::Plain => json!(""),
        };
        let value = if is_localized(&meta, field) {
            let mut by_locale = Map::new();
            by_locale.insert(locale.to_string(), empty);
            Value::Object(by_locale)
        } else {
            empty
        };
        let mut props = block_def(&new_type)
            .map(|def| def.default_props.clone())
            .unwrap_or_default();
        props.insert(target_field.clone(), value);
        let new_id = new_id();
        let block = json!({
            "id": new_id,
            "type": new_type,
            "props": Value::Object(props),
        });
        let new_node = map_loro(tree.create_at(parent, index))?;
        doc::fill_block(&tree, new_node, &block)?;

        let target = text_or_create(&self.meta_of(new_node)?, &target_field, locale)?;
        match target_kind {
            TextKind::Rich => {
                let nodes: Vec<Value> = tail_nodes.as_array().cloned().unwrap_or_default();
                richtext::import_rich(&target, &nodes)?;
            }
            TextKind::Plain => richtext::import_plain(&target, &tail_plain)?,
        }

        // Remove the tail from the original block.
        if let Some(text) = &source {
            let total = text.len_unicode();
            if total > cut {
                map_loro(text.delete(cut, total - cut))?;
            }
        }
        Ok(new_id)
    }

    fn merge_op(
        &self,
        id: &str,
        field: &str,
        locale: &str,
    ) -> Result<Option<(String, usize)>, Error> {
        let node = self.find(id)?;
        self.guard_delete(node)?;
        let meta = self.meta_of(node)?;
        let ty = string_entry(&meta, "type").unwrap_or_default();
        if inline_field_of(&ty).as_deref() != Some(field) {
            return Err(Error::NotAllowed(format!(
                "{field} is not the inline text of {ty}"
            )));
        }
        let parent = self.parent_of(node);
        let siblings = self.siblings(parent);
        let pos = siblings
            .iter()
            .position(|sibling| *sibling == node)
            .unwrap_or(0);
        if pos == 0 {
            return Ok(None);
        }
        let prev = siblings[pos - 1];
        let prev_ty = self.type_of(prev)?;
        let (Some(prev_field), Some(prev_kind)) = (
            inline_field_of(&prev_ty),
            inline_field_of(&prev_ty).and_then(|f| text_kind(&prev_ty, &f)),
        ) else {
            return Ok(None);
        };
        self.guard_prop(prev, &prev_field)?;

        // The children move under the previous block, so they must fit there.
        let children = self.siblings(TreeParentId::Node(node));
        for child in &children {
            let child_ty = self.type_of(*child)?;
            if !allowed_child(Some(prev_ty.as_str()), child_ty.as_str()) {
                return Err(Error::NotAllowed(format!(
                    "{child_ty} cannot sit under {prev_ty}"
                )));
            }
        }

        let prev_meta = self.meta_of(prev)?;
        let target = text_or_create(&prev_meta, &prev_field, locale)?;
        let caret = target.to_string().len();
        let source_nodes = match existing_text(&meta, field, locale) {
            Some(text) => runs_to_nodes(text.to_delta()),
            None => json!([]),
        };
        match prev_kind {
            TextKind::Rich => {
                let nodes: Vec<Value> = source_nodes.as_array().cloned().unwrap_or_default();
                richtext::import_rich(&target, &nodes)?;
            }
            TextKind::Plain => {
                let plain = plain_of_nodes(&source_nodes);
                if !plain.is_empty() {
                    map_loro(target.insert(target.len_unicode(), &plain))?;
                }
            }
        }

        let tree = self.tree();
        for child in children {
            map_loro(tree.mov(child, TreeParentId::Node(prev)))?;
        }
        map_loro(tree.delete(node))?;
        Ok(Some((
            string_entry(&prev_meta, "id").unwrap_or_default(),
            caret,
        )))
    }

    fn delete_blocks_op(&self, ids: &[String]) -> Result<(), Error> {
        let mut nodes = Vec::new();
        for id in ids {
            let node = self.find(id)?;
            self.guard_delete(node)?;
            nodes.push(node);
        }
        let top: Vec<TreeID> = nodes
            .iter()
            .copied()
            .filter(|node| {
                !nodes
                    .iter()
                    .any(|other| other != node && self.is_ancestor(*other, *node))
            })
            .collect();
        let tree = self.tree();
        for node in top {
            map_loro(tree.delete(node))?;
        }
        Ok(())
    }

    fn move_block_op(&self, id: &str, parent: Option<&str>, index: usize) -> Result<(), Error> {
        let node = self.find(id)?;
        self.guard_delete(node)?;
        let (new_parent, parent_ty) = self.resolve_parent(parent)?;
        if let TreeParentId::Node(target) = new_parent
            && (target == node || self.is_ancestor(node, target))
        {
            return Err(Error::NotAllowed("a block cannot move into itself".into()));
        }
        let ty = self.type_of(node)?;
        if !allowed_child(parent_ty.as_deref(), &ty) {
            return Err(Error::NotAllowed(format!(
                "{ty} cannot sit under the new parent"
            )));
        }
        let same_parent = self.parent_of(node) == new_parent;
        let room = self.siblings(new_parent).len() - usize::from(same_parent);
        // UNSURE: `mov_to` index is the position in the final sibling list (the source only shows the signature).
        map_loro(self.tree().mov_to(node, new_parent, index.min(room)))
    }

    fn move_up_op(&self, id: &str) -> Result<(), Error> {
        let node = self.find(id)?;
        self.guard_delete(node)?;
        let siblings = self.siblings(self.parent_of(node));
        let pos = siblings
            .iter()
            .position(|sibling| *sibling == node)
            .unwrap_or(0);
        if pos == 0 {
            return Ok(());
        }
        map_loro(self.tree().mov_before(node, siblings[pos - 1]))
    }

    fn move_down_op(&self, id: &str) -> Result<(), Error> {
        let node = self.find(id)?;
        self.guard_delete(node)?;
        let siblings = self.siblings(self.parent_of(node));
        let pos = siblings
            .iter()
            .position(|sibling| *sibling == node)
            .unwrap_or(0);
        if pos + 1 >= siblings.len() {
            return Ok(());
        }
        map_loro(self.tree().mov_after(node, siblings[pos + 1]))
    }

    fn indent_op(&self, id: &str) -> Result<(), Error> {
        let tree = self.tree();
        let node = self.find(id)?;
        self.guard_delete(node)?;
        let parent = self.parent_of(node);
        let siblings = self.siblings(parent);
        let pos = siblings
            .iter()
            .position(|sibling| *sibling == node)
            .unwrap_or(0);
        if pos == 0 {
            return Err(Error::NotAllowed(
                "no previous sibling to indent under".into(),
            ));
        }
        let prev = siblings[pos - 1];
        let ty = self.type_of(node)?;
        let prev_ty = self.type_of(prev)?;

        if ty == "ListItemBlock" && prev_ty == "ListItemBlock" {
            let list_ty = self.parent_type(parent)?.unwrap_or_default();
            if !LIST_TYPES.contains(&list_ty.as_str()) {
                return Err(Error::NotAllowed("list item is not inside a list".into()));
            }
            // Reuse the previous item's last list if it is one of this type, else make a nested list.
            let reuse = match self.siblings(TreeParentId::Node(prev)).last().copied() {
                Some(last) if self.type_of(last)? == list_ty => Some(last),
                _ => None,
            };
            let nested = match reuse {
                Some(list) => list,
                None => {
                    if !allowed_child(Some("ListItemBlock"), &list_ty) {
                        return Err(Error::NotAllowed(format!(
                            "{list_ty} cannot sit under a list item"
                        )));
                    }
                    let list = map_loro(tree.create(TreeParentId::Node(prev)))?;
                    let block = json!({ "id": new_id(), "type": list_ty.clone(), "props": {} });
                    doc::fill_block(&tree, list, &block)?;
                    list
                }
            };
            // UNSURE: `LoroTree::mov` appends the node as the last child of the new parent.
            map_loro(tree.mov(node, TreeParentId::Node(nested)))?;
            return self.prune_empty_list(parent);
        }

        if !allowed_child(Some(prev_ty.as_str()), ty.as_str()) {
            return Err(Error::NotAllowed(format!(
                "{ty} cannot sit under {prev_ty}"
            )));
        }
        map_loro(tree.mov(node, TreeParentId::Node(prev)))
    }

    fn outdent_op(&self, id: &str) -> Result<(), Error> {
        let tree = self.tree();
        let node = self.find(id)?;
        self.guard_delete(node)?;
        let parent = self.parent_of(node);
        let TreeParentId::Node(holder) = parent else {
            return Err(Error::NotAllowed("already at the top level".into()));
        };
        let holder_ty = self.type_of(holder)?;
        let ty = self.type_of(node)?;

        if ty == "ListItemBlock"
            && LIST_TYPES.contains(&holder_ty.as_str())
            && let TreeParentId::Node(item) = self.parent_of(holder)
            && self.type_of(item)? == "ListItemBlock"
        {
            let outer_ty = self.parent_type(self.parent_of(item))?;
            if !allowed_child(outer_ty.as_deref(), &ty) {
                return Err(Error::NotAllowed(
                    "list item cannot sit in the outer list".into(),
                ));
            }
            map_loro(tree.mov_after(node, item))?;
            return self.prune_empty_list(parent);
        }

        let grand = self.parent_type(self.parent_of(holder))?;
        if !allowed_child(grand.as_deref(), &ty) {
            return Err(Error::NotAllowed(format!(
                "{ty} cannot sit under the outer parent"
            )));
        }
        map_loro(tree.mov_after(node, holder))
    }

    /// Deletes a list block that has no items left.
    fn prune_empty_list(&self, parent: TreeParentId) -> Result<(), Error> {
        if let TreeParentId::Node(list) = parent {
            let ty = self.type_of(list)?;
            if LIST_TYPES.contains(&ty.as_str()) && self.siblings(parent).is_empty() {
                map_loro(self.tree().delete(list))?;
            }
        }
        Ok(())
    }

    fn turn_into_op(&self, id: &str, block_type: &str) -> Result<(), Error> {
        let tree = self.tree();
        let node = self.find(id)?;
        self.guard_delete(node)?;
        let meta = self.meta_of(node)?;
        let old_ty = string_entry(&meta, "type").unwrap_or_default();
        if old_ty == block_type {
            return Ok(());
        }
        let def = block_def(block_type)
            .ok_or_else(|| Error::NotAllowed(format!("unknown block type {block_type}")))?;
        // Web-only blocks are fully editable in the native editor (decision 0049, answer 8); their preview is a web view.
        let parent_ty = self.parent_type(self.parent_of(node))?;
        if !allowed_child(parent_ty.as_deref(), block_type) {
            return Err(Error::NotAllowed(format!(
                "{block_type} cannot sit under the parent"
            )));
        }

        let mut dropped = Vec::new();
        for kid in self.siblings(TreeParentId::Node(node)) {
            let kid_ty = self.type_of(kid)?;
            if !allowed_child(Some(block_type), kid_ty.as_str()) {
                dropped.push(kid);
            }
        }

        // Carry the inline text, every locale of it, before the node's data is cleared.
        let mut props = def.default_props.clone();
        if let (Some(src_field), Some(dst_field)) =
            (inline_field_of(&old_ty), inline_field_of(block_type))
            && text_kind(&old_ty, &src_field).is_some()
            && text_kind(block_type, &dst_field).is_some()
        {
            let dst_kind = text_kind(block_type, &dst_field);
            let mut single = None;
            let mut by_locale = Map::new();
            for key in meta.keys() {
                let key = key.to_string();
                let Some(rest) = key.strip_prefix("t:") else {
                    continue;
                };
                let (name, locale) = match rest.rsplit_once(':') {
                    Some((name, locale)) if is_locale_code(locale) => (name, Some(locale)),
                    _ => (rest, None),
                };
                if name != src_field {
                    continue;
                }
                let Some(ValueOrContainer::Container(Container::Text(text))) = meta.get(&key)
                else {
                    continue;
                };
                let nodes = runs_to_nodes(text.to_delta());
                let value = match dst_kind {
                    Some(TextKind::Plain) => Value::String(plain_of_nodes(&nodes)),
                    _ => nodes,
                };
                match locale {
                    Some(locale) => {
                        by_locale.insert(locale.to_string(), value);
                    }
                    None => single = Some(value),
                }
            }
            if !by_locale.is_empty() {
                props.insert(dst_field.clone(), Value::Object(by_locale));
            } else if let Some(value) = single {
                props.insert(dst_field.clone(), value);
            }
        }

        for kid in dropped {
            map_loro(tree.delete(kid))?;
        }
        let stale: Vec<String> = meta
            .keys()
            .map(|key| key.to_string())
            .filter(|key| key.starts_with("p:") || key.starts_with("t:") || key == "rows")
            .collect();
        for key in stale {
            map_loro(meta.delete(&key))?;
        }
        let id = string_entry(&meta, "id").unwrap_or_default();
        let block = json!({
            "id": id,
            "type": block_type,
            "props": Value::Object(props),
        });
        doc::fill_block(&tree, node, &block)
    }

    fn duplicate_op(&self, id: &str) -> Result<String, Error> {
        let tree = self.tree();
        let node = self.find(id)?;
        let parent = self.parent_of(node);
        let ty = self.type_of(node)?;
        let parent_ty = self.parent_type(parent)?;
        if !allowed_child(parent_ty.as_deref(), &ty) {
            return Err(Error::NotAllowed(format!(
                "{ty} cannot sit under the parent"
            )));
        }
        let index = self
            .siblings(parent)
            .iter()
            .position(|sibling| *sibling == node)
            .map_or(0, |pos| pos + 1);
        let mut block = doc::export_block(&tree, node)?;
        refresh_ids(&mut block);
        if let Some(obj) = block.as_object_mut() {
            // A copy is not part of the template that locked the original.
            obj.remove("isTemplated");
            obj.remove("templatedProps");
        }
        let new_id = block
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let new_node = map_loro(tree.create_at(parent, index))?;
        doc::fill_block(&tree, new_node, &block)?;
        Ok(new_id)
    }

    fn set_prop_op(&self, id: &str, key: &str, value: &Value) -> Result<(), Error> {
        let node = self.find(id)?;
        self.guard_prop(node, key)?;
        if key == "id" || key == "type" || key == "children" || BLOCK_LOCK_KEYS.contains(&key) {
            return Err(Error::NotAllowed(format!(
                "{key} is not a prop; use set_access or turn_into"
            )));
        }
        let meta = self.meta_of(node)?;
        let ty = string_entry(&meta, "type").unwrap_or_default();
        clear_prop(&meta, key)?;
        doc::import_prop(&meta, &ty, key, value)
    }

    fn set_access_op(&self, id: &str, access: &BlockAccess) -> Result<(), Error> {
        let meta = self.meta_of(self.find(id)?)?;
        if let Some(visibility) = &access.visibility
            && visibility != "internal"
            && visibility != "public"
        {
            return Err(Error::NotAllowed(format!("visibility {visibility}")));
        }
        put(
            &meta,
            "visibility",
            access.visibility.as_ref().map(|v| Value::String(v.clone())),
        )?;
        let roles = if access.allowed_roles.is_empty() {
            None
        } else {
            Some(Value::Array(
                access
                    .allowed_roles
                    .iter()
                    .map(|r| Value::String(r.clone()))
                    .collect(),
            ))
        };
        put(&meta, "allowedRoles", roles)?;
        put(
            &meta,
            "requiredPermission",
            access
                .required_permission
                .as_ref()
                .filter(|p| !p.is_empty())
                .map(|p| Value::String(p.clone())),
        )
    }

    fn set_meta_text_op(&self, key: &str, locale: &str, value: &str) -> Result<(), Error> {
        if key != "title" && key != "description" {
            return Err(Error::NotAllowed(format!("{key} is not a page text")));
        }
        let root = self.root_meta();
        let meta_key = format!("{key}:{locale}");
        match root.get(&meta_key) {
            Some(ValueOrContainer::Container(Container::Text(text))) => apply_diff(&text, value),
            _ => {
                let text = map_loro(root.insert_container(&meta_key, LoroText::new()))?;
                richtext::import_plain(&text, value)
            }
        }
    }

    fn set_property_op(&self, key: &str, value: &Value) -> Result<(), Error> {
        let root = self.root_meta();
        let props = match root.get("properties") {
            Some(ValueOrContainer::Container(Container::Map(map))) => map,
            _ => map_loro(root.insert_container("properties", LoroMap::new()))?,
        };
        if value.is_null() {
            map_loro(props.delete(key))
        } else {
            map_loro(props.insert(key, json_to_loro(value)))
        }
    }

    fn table_rows(&self, id: &str) -> Result<(TreeID, LoroList), Error> {
        let node = self.find(id)?;
        let ty = self.type_of(node)?;
        if ty != "TableBlock" {
            return Err(Error::NotAllowed(format!("{ty} is not a table")));
        }
        match self.meta_of(node)?.get("rows") {
            Some(ValueOrContainer::Container(Container::List(list))) => Ok((node, list)),
            _ => Err(Error::NotFound(format!("rows of {id}"))),
        }
    }

    fn table_set_cell_op(
        &self,
        id: &str,
        row: usize,
        column: &str,
        value: &str,
    ) -> Result<(), Error> {
        let (node, rows) = self.table_rows(id)?;
        self.guard_prop(node, "rows")?;
        let Some(ValueOrContainer::Container(Container::Map(cells))) = rows.get(row) else {
            return Err(Error::InvalidRange(format!("row {row}")));
        };
        match cells.get(column) {
            Some(ValueOrContainer::Container(Container::Text(text))) => apply_diff(&text, value),
            _ => {
                let text = map_loro(cells.insert_container(column, LoroText::new()))?;
                richtext::import_plain(&text, value)
            }
        }
    }

    fn table_insert_row_op(&self, id: &str, at: usize) -> Result<(), Error> {
        let (node, rows) = self.table_rows(id)?;
        self.guard_prop(node, "rows")?;
        if at > rows.len() {
            return Err(Error::InvalidRange(format!("row {at}")));
        }
        let columns: Vec<String> = match value_entry(&self.meta_of(node)?, "p:columns") {
            Some(Value::Array(items)) => items
                .iter()
                .filter_map(|item| item.get("key").and_then(Value::as_str).map(str::to_string))
                .collect(),
            _ => Vec::new(),
        };
        let row = map_loro(rows.insert_container(at, LoroMap::new()))?;
        for column in columns {
            map_loro(row.insert_container(&column, LoroText::new()))?;
        }
        Ok(())
    }

    fn table_delete_row_op(&self, id: &str, at: usize) -> Result<(), Error> {
        let (node, rows) = self.table_rows(id)?;
        self.guard_prop(node, "rows")?;
        if at >= rows.len() {
            return Err(Error::InvalidRange(format!("row {at}")));
        }
        map_loro(rows.delete(at, 1))
    }
}

// ----- row and locale walks -----

fn collect_rows(
    tree: &LoroTree,
    parent: TreeParentId,
    parent_id: Option<String>,
    depth: usize,
    sections: usize,
    out: &mut Vec<BlockRow>,
) {
    for (index, node) in tree
        .children(parent)
        .unwrap_or_default()
        .into_iter()
        .enumerate()
    {
        let Ok(meta) = tree.get_meta(node) else {
            continue;
        };
        let block_type = string_entry(&meta, "type").unwrap_or_default();
        let id = string_entry(&meta, "id").unwrap_or_default();
        let is_section = block_type == "SectionBlock";
        let inline_field = inline_field_of(&block_type);
        let rich = inline_field
            .as_deref()
            .is_some_and(|field| text_kind(&block_type, field) == Some(TextKind::Rich));

        let mut props = Map::new();
        for key in meta.keys() {
            let key = key.to_string();
            if let Some(prop) = key.strip_prefix("p:")
                && let Some(value) = value_entry(&meta, &key)
            {
                props.insert(prop.to_string(), value);
            }
        }
        let locked_props = match value_entry(&meta, "templatedProps") {
            Some(Value::Bool(true)) => LockedProps::All,
            Some(Value::Array(keys)) => LockedProps::Keys(
                keys.iter()
                    .filter_map(|key| key.as_str().map(str::to_string))
                    .collect(),
            ),
            _ => LockedProps::None,
        };
        let row = BlockRow {
            id: id.clone(),
            web_only: block_def(&block_type).is_some_and(|def| def.web_only),
            block_type,
            depth,
            parent: parent_id.clone(),
            index,
            has_children: tree
                .children(TreeParentId::Node(node))
                .is_some_and(|kids| !kids.is_empty()),
            inline_field,
            rich,
            heading_level: is_section.then(|| (2 + sections).min(6) as u8),
            props,
            access: BlockAccess {
                visibility: string_entry(&meta, "visibility"),
                allowed_roles: string_list(&meta, "allowedRoles"),
                required_permission: string_entry(&meta, "requiredPermission"),
            },
            templated: is_templated(&meta),
            locked_props,
        };
        out.push(row);
        collect_rows(
            tree,
            TreeParentId::Node(node),
            Some(id),
            depth + 1,
            sections + usize::from(is_section),
            out,
        );
    }
}

fn collect_locales(tree: &LoroTree, parent: TreeParentId, found: &mut BTreeSet<String>) {
    for node in tree.children(parent).unwrap_or_default() {
        if let Ok(meta) = tree.get_meta(node) {
            for key in meta.keys() {
                let key = key.to_string();
                if let Some(rest) = key.strip_prefix("t:")
                    && let Some((_, locale)) = rest.rsplit_once(':')
                    && is_locale_code(locale)
                {
                    found.insert(locale.to_string());
                }
            }
        }
        collect_locales(tree, TreeParentId::Node(node), found);
    }
}

// ----- meta helpers -----

fn string_entry(meta: &LoroMap, key: &str) -> Option<String> {
    match meta.get(key) {
        Some(ValueOrContainer::Value(value)) => loro_to_json(&value).as_str().map(str::to_string),
        _ => None,
    }
}

fn value_entry(meta: &LoroMap, key: &str) -> Option<Value> {
    match meta.get(key) {
        Some(ValueOrContainer::Value(value)) => Some(loro_to_json(&value)),
        _ => None,
    }
}

fn string_list(meta: &LoroMap, key: &str) -> Vec<String> {
    match value_entry(meta, key) {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

fn is_templated(meta: &LoroMap) -> bool {
    value_entry(meta, "isTemplated") == Some(Value::Bool(true))
}

/// The registry's inline text prop of a block type (`CodeBlock`: `code`).
fn inline_field_of(block_type: &str) -> Option<String> {
    if block_type == "CodeBlock" {
        return Some("code".to_string());
    }
    block_def(block_type)?.inline_text.clone()
}

fn put(meta: &LoroMap, key: &str, value: Option<Value>) -> Result<(), Error> {
    match value {
        Some(value) => map_loro(meta.insert(key, json_to_loro(&value))),
        // UNSURE: `LoroMap::delete` on a key that is not present is treated as a no-op.
        None => map_loro(meta.delete(key)),
    }
}

fn new_id() -> String {
    Uuid::new_v4().to_string()
}

/// Gives a block JSON tree fresh ids.
fn refresh_ids(block: &mut Value) {
    if let Some(obj) = block.as_object_mut() {
        obj.insert("id".into(), Value::String(new_id()));
        if let Some(children) = obj.get_mut("children").and_then(Value::as_array_mut) {
            for child in children {
                refresh_ids(child);
            }
        }
    }
}

/// Removes every stored form of one prop (`p:`, `t:`, `t:<prop>:<locale>`, and the table `rows` list).
fn clear_prop(meta: &LoroMap, key: &str) -> Result<(), Error> {
    let plain = format!("p:{key}");
    let text = format!("t:{key}");
    let text_prefix = format!("t:{key}:");
    let doomed: Vec<String> = meta
        .keys()
        .map(|k| k.to_string())
        .filter(|k| {
            *k == plain
                || *k == text
                || (key == "rows" && k == "rows")
                || k.strip_prefix(text_prefix.as_str())
                    .is_some_and(is_locale_code)
        })
        .collect();
    for k in doomed {
        map_loro(meta.delete(&k))?;
    }
    Ok(())
}

fn is_localized(meta: &LoroMap, field: &str) -> bool {
    let prefix = format!("t:{field}:");
    meta.keys()
        .any(|key| key.to_string().starts_with(prefix.as_str()))
}

// ----- text helpers -----

/// The locale a non-localized field belongs to: text stored without a locale is this locale's text.
pub const DEFAULT_LOCALE: &str = "en";

fn text_at(meta: &LoroMap, key: &str) -> Option<LoroText> {
    match meta.get(key) {
        Some(ValueOrContainer::Container(Container::Text(text))) => Some(text),
        _ => None,
    }
}

/// The Loro text of a field in a locale. A localized field has one text per locale; a non-localized field's single
/// text is the default locale's (`DEFAULT_LOCALE`), so another locale has no text yet (the editor shows the default
/// as a fallback).
fn existing_text(meta: &LoroMap, field: &str, locale: &str) -> Option<LoroText> {
    if is_localized(meta, field) {
        text_at(meta, &format!("t:{field}:{locale}"))
    } else if locale == DEFAULT_LOCALE {
        text_at(meta, &format!("t:{field}"))
    } else {
        None
    }
}

/// Creates the field's text in `locale`. The default locale of a non-localized field stays non-localized. Any other
/// locale makes the field localized first: its existing text, marks included, becomes the default locale's text.
fn create_text(meta: &LoroMap, field: &str, locale: &str) -> Result<LoroText, Error> {
    if !is_localized(meta, field) {
        if locale == DEFAULT_LOCALE {
            return map_loro(meta.insert_container(&format!("t:{field}"), LoroText::new()));
        }
        let plain_key = format!("t:{field}");
        if let Some(old) = text_at(meta, &plain_key) {
            let delta = old.to_delta();
            let default = map_loro(
                meta.insert_container(&format!("t:{field}:{DEFAULT_LOCALE}"), LoroText::new()),
            )?;
            // UNSURE: LoroText::apply_delta takes the same TextDelta items to_delta returns (loro 1.18).
            map_loro(default.apply_delta(&delta))?;
            map_loro(meta.delete(&plain_key))?;
        }
    }
    map_loro(meta.insert_container(&format!("t:{field}:{locale}"), LoroText::new()))
}

fn text_or_create(meta: &LoroMap, field: &str, locale: &str) -> Result<LoroText, Error> {
    match existing_text(meta, field, locale) {
        Some(text) => Ok(text),
        None => create_text(meta, field, locale),
    }
}

/// The plain text and spans of a Loro text (MAPPING section 3, with positions as UTF-8 bytes).
fn text_value(text: &LoroText) -> TextValue {
    let mut plain = String::new();
    let mut spans = Vec::new();
    for item in text.to_delta() {
        let TextDelta::Insert { insert, attributes } = item else {
            continue;
        };
        let start = plain.len();
        plain.push_str(&insert);
        let attrs = attributes.unwrap_or_default();
        let attr = |key: &str| attrs.get(key).map(loro_to_json);
        let is_true = |key: &str| attr(key) == Some(Value::Bool(true));
        let marks: Vec<MarkKind> = MarkKind::ALL
            .iter()
            .copied()
            .filter(|mark| is_true(mark.key()))
            .collect();
        let link =
            attr("link").and_then(|v| v.get("url").and_then(Value::as_str).map(str::to_string));
        let mention = attr("mention").map(|v| Mention {
            entry_id: str_of(&v, "entryId").to_string(),
            collection: str_of(&v, "collection").to_string(),
            slug: str_of(&v, "slug").to_string(),
            title: v.get("title").and_then(Value::as_str).map(str::to_string),
        });
        let emoji = attr("emoji").is_some();
        let icon =
            attr("icon").and_then(|v| v.get("name").and_then(Value::as_str).map(str::to_string));
        spans.push(Span {
            range: start..plain.len(),
            marks,
            link,
            mention,
            emoji,
            icon,
        });
    }
    TextValue { plain, spans }
}

fn str_of<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or_default()
}

/// The label a mention shows: its title, else its slug, else `@` (MAPPING section 3).
fn mention_label(mention: &Mention) -> String {
    mention
        .title
        .clone()
        .filter(|t| !t.is_empty())
        .or_else(|| Some(mention.slug.clone()).filter(|s| !s.is_empty()))
        .unwrap_or_else(|| "@".to_string())
}

/// Plain text of normal-form rich text nodes (mentions by title or slug, icons dropped).
fn plain_of_nodes(nodes: &Value) -> String {
    let mut out = String::new();
    for node in nodes.as_array().into_iter().flatten() {
        match node.get("type").and_then(Value::as_str) {
            Some("text") => out.push_str(str_of(node, "text")),
            Some("emoji") => out.push_str(str_of(node, "emoji")),
            Some("mention") => {
                let label = Some(str_of(node, "title"))
                    .filter(|t| !t.is_empty())
                    .unwrap_or_else(|| str_of(node, "slug"));
                out.push_str(label);
            }
            Some("link") => {
                if let Some(children) = node.get("children") {
                    out.push_str(&plain_of_nodes(children));
                }
            }
            _ => {}
        }
    }
    out
}

/// The runs of `runs` from unicode position `from` on.
fn tail_from(runs: Vec<TextDelta>, from: usize) -> Vec<TextDelta> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    for run in runs {
        let TextDelta::Insert { insert, attributes } = run else {
            continue;
        };
        let len = insert.chars().count();
        if pos + len <= from {
            pos += len;
            continue;
        }
        let skip = from.saturating_sub(pos);
        pos += len;
        let kept: String = insert.chars().skip(skip).collect();
        out.push(TextDelta::Insert {
            insert: kept,
            attributes,
        });
    }
    out
}

fn byte_to_unicode(plain: &str, byte: usize) -> Result<usize, Error> {
    if byte > plain.len() || !plain.is_char_boundary(byte) {
        return Err(Error::InvalidRange(format!("byte offset {byte}")));
    }
    Ok(plain[..byte].chars().count())
}

fn unicode_range(plain: &str, range: &Range<usize>) -> Result<(usize, usize), Error> {
    if range.start > range.end {
        return Err(Error::InvalidRange(format!(
            "{}..{}",
            range.start, range.end
        )));
    }
    Ok((
        byte_to_unicode(plain, range.start)?,
        byte_to_unicode(plain, range.end)?,
    ))
}

/// The bytes of `old` that a change to `new` replaces: `(start, end)` in `old`, with the common prefix and suffix
/// kept.
fn changed_range(old: &str, new: &str) -> (usize, usize) {
    let mut prefix = 0;
    for (a, b) in old.chars().zip(new.chars()) {
        if a != b {
            break;
        }
        prefix += a.len_utf8();
    }
    let max_suffix = old.len().min(new.len()) - prefix;
    let mut suffix = 0;
    for (a, b) in old.chars().rev().zip(new.chars().rev()) {
        if a != b || suffix + a.len_utf8() > max_suffix {
            break;
        }
        suffix += a.len_utf8();
    }
    (prefix, old.len() - suffix)
}

/// Replaces the text of `text` with `new`, changing only the differing middle. A mention that the change touches is
/// deleted whole, so the edit range grows to cover it.
fn apply_diff(text: &LoroText, new: &str) -> Result<(), Error> {
    let old = text.to_string();
    let mentions: Vec<Range<usize>> = text_value(text)
        .spans
        .into_iter()
        .filter(|span| span.mention.is_some())
        .map(|span| span.range)
        .collect();
    let (mut start, mut end) = changed_range(&old, new);
    loop {
        let mut grew = false;
        for mention in &mentions {
            if mention.start < end && mention.end > start {
                let next_start = start.min(mention.start);
                let next_end = end.max(mention.end);
                if next_start != start || next_end != end {
                    start = next_start;
                    end = next_end;
                    grew = true;
                }
            }
        }
        if !grew {
            break;
        }
    }
    let tail = old.len() - end;
    let replacement = &new[start..new.len() - tail];
    let at = old[..start].chars().count();
    let removed = old[start..end].chars().count();
    if removed > 0 {
        map_loro(text.delete(at, removed))?;
    }
    if !replacement.is_empty() {
        map_loro(text.insert(at, replacement))?;
    }
    Ok(())
}
