//! The editor's behaviour in plain Rust. [`EditorController`] owns the [`EditorDoc`] and the editor state (locale, focus,
//! block selection, the `/`, `@` and `:` menus, the block menu, the link prompt, the comment panel, the inspector), and
//! turns every user action into document edits plus a list of [`ControllerEffect`]s for the app to act on. It has no
//! Slint types, so the keyboard map, the markdown shortcuts and the menus are unit-tested in `tests/controller.rs`.
//!
//! Positions. A row is an index into the display-order list of blocks (`rows`). A caret is a pair of byte offsets
//! (`anchor`, `head`) into the plain text of the row's inline field. Both come from the Slint `TextInput`, which the
//! app reports through [`EditorController::caret_moved`] before every key and every text edit.
//!
//! Block type names are the registry's (`rimelight_content/registry/blocks.json`), for example `ParagraphBlock`.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Display;
use std::ops::Range;

use rimelight_content::editor::{
    BlockAccess, BlockRow, EditorDoc, LockedProps, MarkKind, Mention, TextValue,
};
use rimelight_content::registry;
use rimelight_content::sync::RemoteCursor;
use serde_json::{Value, json};

use crate::emoji;
use crate::layout::{caret_rect, line_edges, offset_at};
use crate::view::{RowView, font_size, row_view};
use crate::webview::embed_url;

const PARAGRAPH: &str = "ParagraphBlock";
const SECTION: &str = "SectionBlock";
const UNORDERED_LIST: &str = "UnorderedListBlock";
const ORDERED_LIST: &str = "OrderedListBlock";
const LIST_ITEM: &str = "ListItemBlock";
const QUOTE: &str = "QuoteBlock";
const CODE: &str = "CodeBlock";
const DIVIDER: &str = "DividerBlock";

/// The locales the locale switcher offers, with their names. A locale appears in the switcher once it has text.
const OFFERED_LOCALES: [(&str, &str); 4] = [
    ("en", "English"),
    ("pt", "Português"),
    ("es", "Español"),
    ("pt-BR", "Português (Brasil)"),
];

/// The block actions, in menu order: action name and label.
pub const BLOCK_ACTIONS: [(&str, &str); 7] = [
    ("duplicate", "Duplicate"),
    ("comment", "Comment"),
    ("indent", "Indent"),
    ("outdent", "Outdent"),
    ("move-up", "Move up"),
    ("move-down", "Move down"),
    ("delete", "Delete"),
];

/// Search words for the insert menu, by block type (beyond the registry label and category).
const KEYWORDS: &[(&str, &[&str])] = &[
    ("ParagraphBlock", &["text", "paragraph"]),
    (
        "SectionBlock",
        &["heading", "title", "header", "h1", "h2", "h3"],
    ),
    ("CalloutBlock", &["note", "info", "warning", "tip", "alert"]),
    ("UnorderedListBlock", &["bullet", "bulleted", "list", "ul"]),
    ("OrderedListBlock", &["numbered", "ordered", "list", "ol"]),
    ("ImageBlock", &["picture", "photo", "img"]),
    ("CodeBlock", &["code", "snippet", "fence", "source"]),
    ("TableBlock", &["grid", "data", "spreadsheet"]),
    ("TabsBlock", &["tab", "tabs", "panels"]),
    ("StepsBlock", &["steps", "procedure", "instructions"]),
    ("CardsBlock", &["cards", "tiles", "grid"]),
    ("FileTreeBlock", &["files", "tree", "folder", "directory"]),
    ("LivePreviewBlock", &["preview", "demo", "component", "web"]),
    ("DividerBlock", &["rule", "hr", "line", "separator"]),
    ("QuoteBlock", &["quote", "blockquote", "citation"]),
    ("EmbedBlock", &["video", "youtube", "embed", "media"]),
    ("ScriptBlock", &["screenplay", "script", "film"]),
    ("SceneBlock", &["scene", "slugline", "location"]),
    ("DialogueBlock", &["dialogue", "character", "line"]),
    (
        "ApiDocumentationBlock",
        &["api", "docs", "props", "component", "web"],
    ),
];

/// The order the insert menu groups by category. Unknown categories sort last.
const CATEGORY_ORDER: [&str; 6] = ["text", "structure", "media", "data", "embed", "screenplay"];

/// A key, as the editor reads it. Slint reports special keys as private-use control codes in the event text; see
/// [`KeyInput::from_slint`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Key {
    Enter,
    Backspace,
    Delete,
    Tab,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    Char(char),
    /// Anything else (function keys, IME composition).
    Other,
}

/// The modifier keys down with a key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub control: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
}

/// One key press with its modifiers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyInput {
    pub key: Key,
    pub mods: Mods,
}

impl KeyInput {
    /// Reads a Slint key event. `text` is the event's text. Slint sets it to a control character or a private-use
    /// code for special keys (`Key.Backspace` is U+0008, `Key.Return` U+000A, `Key.UpArrow` U+F700, and so on).
    // UNSURE: the code points are copied from Slint's `Key` enum; confirm them against slint 1.18.1.
    pub fn from_slint(text: &str, mods: Mods) -> Self {
        let mut chars = text.chars();
        let key = match (chars.next(), chars.next()) {
            (Some(c), None) => match c {
                '\u{0008}' => Key::Backspace,
                '\u{0009}' => Key::Tab,
                '\n' | '\r' => Key::Enter,
                '\u{001b}' => Key::Escape,
                '\u{007f}' => Key::Delete,
                '\u{F700}' => Key::Up,
                '\u{F701}' => Key::Down,
                '\u{F702}' => Key::Left,
                '\u{F703}' => Key::Right,
                '\u{F729}' => Key::Home,
                '\u{F72B}' => Key::End,
                c if ('\u{F700}'..='\u{F8FF}').contains(&c) => Key::Other,
                c => Key::Char(c),
            },
            _ => Key::Other,
        };
        Self { key, mods }
    }
}

/// A change to a comment, for the app to send to the API. The editor never talks to the network itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommentRequest {
    /// A new thread (`parent_id` None) on `block_id` (empty for a page-level comment), or a reply to `parent_id`.
    Submit {
        block_id: String,
        parent_id: Option<String>,
        body: String,
    },
    Resolve {
        id: String,
    },
    Reopen {
        id: String,
    },
}

/// What the app must do after an action. The effects are ordered as the controller produced them.
#[derive(Clone, Debug, PartialEq)]
pub enum ControllerEffect {
    /// Rows changed (text, structure, selection or presence): push `rows` again.
    RowsChanged,
    /// Move the focus: display row index, then the caret as byte offsets in that row's inline text.
    Focus {
        row: usize,
        anchor: usize,
        head: usize,
    },
    /// Menu, prompt or panel state changed: push them again.
    MenusChanged,
    /// The document changed: send `SyncSession::outgoing` to the room.
    SyncNeeded,
    /// Search for mentions matching `query`, then answer with `set_mention_results`.
    MentionSearch { query: String },
    /// The comment panel opened for `ids` (empty = the page). The app loads its threads with `set_comments`.
    CommentRequested { ids: Vec<String> },
    /// A comment was submitted, resolved or reopened. The app calls the API and then `set_comments`.
    Comment { request: CommentRequest },
    /// The locale switched. The app may load anything it keeps per locale.
    LocaleChanged { locale: String },
    /// The phone toolbar's Done: the app hides the keyboard.
    Done,
    /// A change was refused or failed (locked, not allowed, invalid range, bad JSON). The text is for the user.
    Error(String),
}

/// What a key did: `handled` means the editor took the key (the `TextInput` must not also act on it).
#[derive(Clone, Debug, PartialEq)]
pub struct KeyResult {
    pub handled: bool,
    pub effects: Vec<ControllerEffect>,
}

impl KeyResult {
    fn pass() -> Self {
        Self {
            handled: false,
            effects: Vec::new(),
        }
    }

    fn done(effects: Vec<ControllerEffect>) -> Self {
        Self {
            handled: true,
            effects,
        }
    }
}

/// One entry of the insert menu (the `/` menu), built from the registry. `icon_key` is the registry icon's name
/// without its `i-lucide-` prefix. `category` is the group the entry sits in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InsertChoice {
    pub block_type: String,
    pub label: String,
    pub icon_key: String,
    pub category: String,
    pub keywords: Vec<String>,
}

/// Every block type the insert menu knows, in menu order (grouped by category, then registry order), with labels in
/// `locale` (falling back to English).
pub fn insert_catalogue(locale: &str) -> Vec<InsertChoice> {
    let mut items: Vec<(usize, usize, InsertChoice)> = registry::all_blocks()
        .iter()
        .enumerate()
        .map(|(at, def)| {
            let category = def.category.clone().unwrap_or_default();
            let rank = CATEGORY_ORDER
                .iter()
                .position(|c| *c == category)
                .unwrap_or(CATEGORY_ORDER.len());
            let icon_key = def
                .icon
                .as_deref()
                .unwrap_or_default()
                .trim_start_matches("i-lucide-")
                .to_string();
            let keywords: Vec<String> = KEYWORDS
                .iter()
                .find(|(t, _)| *t == def.block_type)
                .map(|(_, words)| words.iter().map(|w| w.to_string()).collect())
                .unwrap_or_default();
            let choice = InsertChoice {
                block_type: def.block_type.clone(),
                label: label_in(&def.label, locale),
                icon_key,
                category,
                keywords,
            };
            (rank, at, choice)
        })
        .collect();
    items.sort_by_key(|(rank, at, _)| (*rank, *at));
    items.into_iter().map(|(_, _, choice)| choice).collect()
}

/// A label in `locale`: the exact code, then its language, then English.
fn label_in(labels: &BTreeMap<String, String>, locale: &str) -> String {
    labels
        .get(locale)
        .or_else(|| labels.get(locale.split('-').next().unwrap_or(locale)))
        .or_else(|| labels.get("en"))
        .cloned()
        .unwrap_or_default()
}

/// One emoji the `:` picker offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmojiChoice {
    pub emoji: String,
    pub shortcode: String,
}

/// One block action of the block menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockMenuChoice {
    pub action: String,
    pub label: String,
}

/// One mention target from the app's entry search.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MentionChoice {
    pub entry_id: String,
    pub collection: String,
    pub slug: String,
    pub title: String,
}

/// One locale in the locale switcher. `present` means the document has text in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocaleChoice {
    pub code: String,
    pub label: String,
    pub present: bool,
    pub active: bool,
}

/// The link prompt as the markup shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkPromptView {
    pub url: String,
    pub new_tab: bool,
    pub has_link: bool,
}

/// One comment as the app stores it. `block_id` is empty for a page-level comment; `parent_id` is set on replies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommentView {
    pub id: String,
    pub block_id: String,
    pub parent_id: Option<String>,
    pub author: String,
    pub body: String,
    pub created_at: String,
    pub resolved: bool,
}

/// One row of the comment panel: a thread's comment, with `depth` 1 for a reply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommentRow {
    pub id: String,
    pub block_id: String,
    pub depth: usize,
    pub author: String,
    pub body: String,
    pub created: String,
    pub resolved: bool,
}

/// The comment panel: the scope (a block id, empty for the page), the focused block, and the rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommentPanelView {
    pub scope: String,
    pub block: String,
    pub rows: Vec<CommentRow>,
}

/// One co-editor in the presence strip of the page header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresenceView {
    pub user_id: String,
    pub name: String,
    pub initial: String,
    pub colour: String,
}

/// The kind of an inspector field, from the registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    Text,
    TextArea,
    Select,
    Boolean,
    Number,
    Url,
    Json,
    Media,
    Tags,
}

impl FieldKind {
    /// The name the markup switches on.
    pub fn name(self) -> &'static str {
        match self {
            FieldKind::Text => "text",
            FieldKind::TextArea => "textarea",
            FieldKind::Select => "select",
            FieldKind::Boolean => "boolean",
            FieldKind::Number => "number",
            FieldKind::Url => "url",
            FieldKind::Json => "json",
            FieldKind::Media => "media",
            FieldKind::Tags => "tags",
        }
    }
}

/// One inspector field of a block type, as the registry declares it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldSpec {
    pub key: String,
    pub label: String,
    pub kind: FieldKind,
    /// The choices of a select field.
    pub options: Vec<String>,
}

/// One inspector field as the markup shows it. Every value is text: booleans are "true" or "false", tags are
/// comma-separated, JSON is pretty-printed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InspectorFieldView {
    pub key: String,
    pub label: String,
    pub kind: &'static str,
    pub value: String,
    pub options: Vec<String>,
    pub read_only: bool,
}

/// A co-editor's caret, positioned in the row (see `layout::caret_rect`).
#[derive(Clone, Debug, PartialEq)]
pub struct RemoteCaretView {
    pub row: usize,
    pub x: f32,
    pub y: f32,
    pub height: f32,
    pub colour: String,
    pub name: String,
}

/// The caret of this editor, by block id.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Focus {
    id: String,
    anchor: usize,
    head: usize,
}

/// An open `/`, `@` or `:` menu: the block and inline field it was typed in, the byte offset of the trigger character,
/// the text typed after it, and the highlighted entry.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Trigger {
    id: String,
    field: String,
    at: usize,
    query: String,
    selected: usize,
}

/// The link prompt's target: the selected range of a block's inline field.
#[derive(Clone, Debug, PartialEq, Eq)]
struct LinkState {
    id: String,
    field: String,
    range: Range<usize>,
    anchor: usize,
    head: usize,
    has_link: bool,
    url: String,
}

/// The block menu of one block, and its highlighted entry.
#[derive(Clone, Debug, PartialEq, Eq)]
struct BlockMenuState {
    id: String,
    selected: usize,
}

/// The markdown shortcuts that turn a paragraph into another block once its text is exactly the marker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shortcut {
    Section,
    UnorderedList,
    OrderedList,
    Quote,
    Code,
    Divider,
}

/// Parses a markdown shortcut. `text` is the whole text of a paragraph just after the marker was typed.
fn shortcut(text: &str) -> Option<Shortcut> {
    match text {
        "---" => return Some(Shortcut::Divider),
        "```" => return Some(Shortcut::Code),
        "- " | "* " => return Some(Shortcut::UnorderedList),
        "> " => return Some(Shortcut::Quote),
        _ => {}
    }
    let hashes = text.chars().take_while(|c| *c == '#').count();
    if (1..=5).contains(&hashes) && &text[hashes..] == " " {
        return Some(Shortcut::Section);
    }
    let digits = text.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && &text[digits..] == ". " {
        return Some(Shortcut::OrderedList);
    }
    None
}

/// The common prefix and suffix of two strings: `(prefix, old_end, new_end)` in bytes, the one region that changed.
fn changed_region(old: &str, new: &str) -> (usize, usize, usize) {
    let prefix: usize = old
        .chars()
        .zip(new.chars())
        .take_while(|(a, b)| a == b)
        .map(|(c, _)| c.len_utf8())
        .sum();
    let suffix: usize = old[prefix..]
        .chars()
        .rev()
        .zip(new[prefix..].chars().rev())
        .take_while(|(a, b)| a == b)
        .map(|(c, _)| c.len_utf8())
        .sum();
    (prefix, old.len() - suffix, new.len() - suffix)
}

/// `text` with `range` removed. A range that does not sit on character boundaries leaves the text as it was.
fn without(text: &str, range: Range<usize>) -> String {
    match (text.get(..range.start), text.get(range.end..)) {
        (Some(head), Some(tail)) => format!("{head}{tail}"),
        _ => text.to_string(),
    }
}

/// `at` moved back to a character boundary inside `text`.
fn clamp(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

fn error(err: impl Display) -> Vec<ControllerEffect> {
    vec![ControllerEffect::Error(err.to_string())]
}

/// The lists the shortcuts and the outdent rule know by name.
const LIST_TYPES: [&str; 2] = [UNORDERED_LIST, ORDERED_LIST];

/// What Up, Down, Escape and Enter do in an open menu.
enum MenuStep {
    Move(usize),
    Choose(usize),
    Close,
    Pass,
}

fn menu_step(key: &Key, selected: usize, count: usize) -> MenuStep {
    match key {
        Key::Up => MenuStep::Move(selected.saturating_sub(1)),
        Key::Down => MenuStep::Move((selected + 1).min(count.saturating_sub(1))),
        Key::Escape => MenuStep::Close,
        Key::Enter if count > 0 => MenuStep::Choose(selected.min(count - 1)),
        _ => MenuStep::Pass,
    }
}

/// Owns the document and the editor state. Construct one per open page; the app keeps it in an `Rc<RefCell<_>>`.
pub struct EditorController {
    doc: EditorDoc,
    locale: String,
    rows: Vec<BlockRow>,
    focus: Option<Focus>,
    /// Block mode: the selected block ids. Empty in text mode.
    selection: Vec<String>,
    slash: Option<Trigger>,
    mention: Option<Trigger>,
    mention_results: Vec<MentionChoice>,
    emoji: Option<Trigger>,
    block_menu: Option<BlockMenuState>,
    link: Option<LinkState>,
    comments: Vec<CommentView>,
    /// The comment panel's scope: `None` when the panel is closed, `Some("")` for the page, else a block id.
    comment_scope: Option<String>,
    fields: HashMap<String, Vec<FieldSpec>>,
    remote: Vec<RemoteCursor>,
    layout_width: f32,
    web_base: Option<String>,
}

impl EditorController {
    /// A controller over `doc`, showing `locale`. The insert menu is built from the registry (see
    /// [`insert_catalogue`]); `insert_block` refuses a block the registry does not allow, and the refusal shows as an
    /// [`ControllerEffect::Error`].
    pub fn new(doc: EditorDoc, locale: &str) -> Self {
        let rows = doc.rows(locale);
        Self {
            doc,
            locale: locale.to_string(),
            rows,
            focus: None,
            selection: Vec::new(),
            slash: None,
            mention: None,
            mention_results: Vec::new(),
            emoji: None,
            block_menu: None,
            link: None,
            comments: Vec::new(),
            comment_scope: None,
            fields: HashMap::new(),
            remote: Vec::new(),
            layout_width: 640.0,
            web_base: None,
        }
    }

    // ----- Reading -----

    pub fn doc(&self) -> &EditorDoc {
        &self.doc
    }

    /// The document, for edits the controller does not offer (page meta, properties). Call [`Self::refresh`] after.
    pub fn doc_mut(&mut self) -> &mut EditorDoc {
        &mut self.doc
    }

    pub fn locale(&self) -> &str {
        &self.locale
    }

    pub fn rows(&self) -> &[BlockRow] {
        &self.rows
    }

    /// The rows as the markup shows them. A row whose text is empty in this locale shows the `en` text, greyed and
    /// read-only (`RowView::fallback`); typing in it creates the locale's text.
    pub fn row_views(&self) -> Vec<RowView> {
        self.rows
            .iter()
            .map(|block| {
                let (text, fallback) = match &block.inline_field {
                    Some(field) => {
                        let here = self.doc.text(&block.id, field, &self.locale);
                        if self.is_fallback(&block.id, field) {
                            (self.doc.text(&block.id, field, "en"), true)
                        } else {
                            (here, false)
                        }
                    }
                    None => (
                        TextValue {
                            plain: String::new(),
                            spans: Vec::new(),
                        },
                        false,
                    ),
                };
                let presence = self
                    .remote
                    .iter()
                    .find(|c| c.cursor.as_ref().is_some_and(|cur| cur.block == block.id && cur.locale == self.locale))
                    .map(|c| (c.color.clone(), c.name.clone()));
                let web_url = match (&self.web_base, block.web_only) {
                    (Some(base), true) => embed_url(
                        base,
                        &json!({"id": block.id, "type": block.block_type, "props": Value::Object(block.props.clone())}),
                    ),
                    _ => String::new(),
                };
                let mut view = row_view(block, &text, self.selection.contains(&block.id), presence, web_url);
                view.fallback = fallback;
                view.comment_count = self.open_comment_count(&block.id);
                view.marker = self.list_marker(block);
                view.accent = self.accent_of(block);
                view.indent = self.indent_of(block);
                view
            })
            .collect()
    }

    /// The marker of a list item: a bullet in a bulleted list, its number in a numbered list. Empty for other blocks.
    fn list_marker(&self, block: &BlockRow) -> String {
        if block.block_type != LIST_ITEM {
            return String::new();
        }
        let parent = block
            .parent
            .as_deref()
            .and_then(|id| self.rows.iter().find(|r| r.id == id));
        match parent.map(|p| p.block_type.as_str()) {
            Some(ORDERED_LIST) => format!("{}.", block.index + 1),
            _ => "\u{2022}".to_string(),
        }
    }

    /// The box a row is drawn in: the nearest callout (itself or an ancestor) as "callout-<variant>", a quote as
    /// "quote". Empty when the row is in neither.
    fn indent_of(&self, block: &BlockRow) -> usize {
        let mut indent = 0;
        let mut parent = block.parent.as_deref();
        while let Some(id) = parent {
            let Some(row) = self.rows.iter().find(|r| r.id == id) else {
                break;
            };
            if row.block_type != "SectionBlock" && row.block_type != "CalloutBlock" {
                indent += 1;
            }
            parent = row.parent.as_deref();
        }
        indent
    }

    fn accent_of(&self, block: &BlockRow) -> String {
        if block.block_type == QUOTE {
            return "quote".to_string();
        }
        let mut current = Some(block);
        while let Some(row) = current {
            if row.block_type == "CalloutBlock" {
                let variant = row
                    .props
                    .get("variant")
                    .and_then(|v| v.as_str())
                    .unwrap_or("info");
                return format!("callout-{variant}");
            }
            current = row
                .parent
                .as_deref()
                .and_then(|id| self.rows.iter().find(|r| r.id == id));
        }
        String::new()
    }

    /// The remote carets in this locale, positioned with parley at the current text and width.
    pub fn caret_views(&self) -> Vec<RemoteCaretView> {
        let mut out = Vec::new();
        for cursor in &self.remote {
            let Some(cur) = &cursor.cursor else { continue };
            if cur.locale != self.locale {
                continue;
            }
            let Some(row) = self.index_of(&cur.block) else {
                continue;
            };
            let block = &self.rows[row];
            if block.inline_field.as_deref() != Some(cur.field.as_str()) {
                continue;
            }
            let text = self.doc.text(&block.id, &cur.field, &self.locale).plain;
            let (x, y, height) = caret_rect(
                &text,
                font_size(&block.block_type, block.heading_level),
                self.layout_width,
                cur.head,
            );
            out.push(RemoteCaretView {
                row,
                x,
                y,
                height,
                colour: cursor.color.clone(),
                name: cursor.name.clone(),
            });
        }
        out
    }

    /// The co-editors, one per user, for the presence strip.
    pub fn presence_views(&self) -> Vec<PresenceView> {
        let mut out: Vec<PresenceView> = Vec::new();
        for cursor in &self.remote {
            if out.iter().any(|p| p.user_id == cursor.user_id) {
                continue;
            }
            out.push(PresenceView {
                user_id: cursor.user_id.clone(),
                name: cursor.name.clone(),
                initial: cursor
                    .name
                    .chars()
                    .next()
                    .map_or_else(|| "?".to_string(), |c| c.to_uppercase().to_string()),
                colour: cursor.color.clone(),
            });
        }
        out
    }

    /// The focused row, anchor and head, when the focus is on a row of this page.
    pub fn focus(&self) -> Option<(usize, usize, usize)> {
        let focus = self.focus.as_ref()?;
        Some((self.index_of(&focus.id)?, focus.anchor, focus.head))
    }

    /// The block ids in block mode, in display order.
    pub fn selection(&self) -> Vec<String> {
        self.selection_in_order()
    }

    pub fn insert_menu_open(&self) -> bool {
        self.slash.is_some()
    }

    /// The insert menu's entries for its query. Only the block types the registry allows under the trigger block's
    /// parent (`registry::allowed_child`), so a list item offers list blocks and nothing else.
    pub fn insert_menu_items(&self) -> Vec<InsertChoice> {
        let Some(trigger) = &self.slash else {
            return Vec::new();
        };
        // The nesting rule is about the parent's type, not its id.
        let parent = self.parent_of(&trigger.id).and_then(|id| {
            self.rows
                .iter()
                .find(|r| r.id == id)
                .map(|r| r.block_type.clone())
        });
        let query = trigger.query.to_lowercase();
        insert_catalogue(&self.locale)
            .into_iter()
            .filter(|c| registry::allowed_child(parent.as_deref(), &c.block_type))
            .filter(|c| {
                query.is_empty()
                    || c.label.to_lowercase().contains(&query)
                    || c.block_type.to_lowercase().contains(&query)
                    || c.keywords.iter().any(|k| k.contains(&query))
            })
            .collect()
    }

    /// The highlighted entry of the insert menu.
    pub fn insert_menu_selected(&self) -> usize {
        self.slash.as_ref().map_or(0, |t| t.selected)
    }

    pub fn mention_menu_open(&self) -> bool {
        self.mention.is_some()
    }

    pub fn mention_menu_items(&self) -> &[MentionChoice] {
        &self.mention_results
    }

    pub fn mention_menu_selected(&self) -> usize {
        self.mention.as_ref().map_or(0, |t| t.selected)
    }

    pub fn emoji_menu_open(&self) -> bool {
        self.emoji.is_some()
    }

    /// The emoji the `:` picker offers for the text typed after the colon.
    pub fn emoji_menu_items(&self) -> Vec<EmojiChoice> {
        let Some(trigger) = &self.emoji else {
            return Vec::new();
        };
        emoji::search(&trigger.query)
            .into_iter()
            .map(|(shortcode, emoji)| EmojiChoice {
                emoji: emoji.to_string(),
                shortcode: shortcode.to_string(),
            })
            .collect()
    }

    pub fn emoji_menu_selected(&self) -> usize {
        self.emoji.as_ref().map_or(0, |t| t.selected)
    }

    pub fn block_menu_open(&self) -> bool {
        self.block_menu.is_some()
    }

    pub fn block_menu_items(&self) -> Vec<BlockMenuChoice> {
        BLOCK_ACTIONS
            .iter()
            .map(|(action, label)| BlockMenuChoice {
                action: action.to_string(),
                label: label.to_string(),
            })
            .collect()
    }

    pub fn block_menu_selected(&self) -> usize {
        self.block_menu.as_ref().map_or(0, |m| m.selected)
    }

    /// The link prompt, when it is open.
    pub fn link_prompt(&self) -> Option<LinkPromptView> {
        let link = self.link.as_ref()?;
        Some(LinkPromptView {
            url: link.url.clone(),
            // UNSURE: the open-in-new-tab flag is not stored: rimelight_content link spans carry only a URL.
            new_tab: false,
            has_link: link.has_link,
        })
    }

    /// The locales for the switcher: every locale the document has text in (English first), then the offered ones
    /// it does not have yet.
    pub fn locale_choices(&self) -> Vec<LocaleChoice> {
        let present = self.doc.locales();
        let mut codes = present.clone();
        for (code, _) in OFFERED_LOCALES {
            if !codes.iter().any(|c| c == code) {
                codes.push(code.to_string());
            }
        }
        codes
            .into_iter()
            .map(|code| LocaleChoice {
                label: locale_label(&code),
                present: present.contains(&code),
                active: code == self.locale,
                code,
            })
            .collect()
    }

    /// The comment panel, when it is open.
    pub fn comment_panel(&self) -> Option<CommentPanelView> {
        let scope = self.comment_scope.clone()?;
        let in_scope: Vec<&CommentView> = self
            .comments
            .iter()
            .filter(|c| c.block_id == scope)
            .collect();
        let mut rows = Vec::new();
        for top in in_scope.iter().filter(|c| c.parent_id.is_none()) {
            rows.push(comment_row(top, 0));
            for reply in in_scope
                .iter()
                .filter(|r| r.parent_id.as_deref() == Some(top.id.as_str()))
            {
                rows.push(comment_row(reply, 1));
            }
        }
        // Replies whose parent is not in this scope are kept, as replies, at the end.
        for reply in in_scope.iter().filter(|r| {
            r.parent_id
                .as_deref()
                .is_some_and(|p| !in_scope.iter().any(|t| t.id == p))
        }) {
            rows.push(comment_row(reply, 1));
        }
        Some(CommentPanelView {
            scope,
            block: self.focus_block().map(|b| b.id).unwrap_or_default(),
            rows,
        })
    }

    /// The inspector fields of the focused block (or the first selected block): its registry fields, then visibility
    /// and roles. Locked keys are read-only.
    pub fn inspector_fields(&self) -> Vec<InspectorFieldView> {
        let Some(block) = self.focus_block() else {
            return Vec::new();
        };
        let mut out = vec![
            InspectorFieldView {
                key: "visibility".into(),
                label: "Visibility".into(),
                kind: "select",
                value: block
                    .access
                    .visibility
                    .clone()
                    .unwrap_or_else(|| "everyone".into()),
                // UNSURE: the visibility policy names. Confirm them against the content service.
                options: vec!["everyone".into(), "roles".into(), "permission".into()],
                read_only: block.templated,
            },
            InspectorFieldView {
                key: "allowedRoles".into(),
                label: "Allowed roles".into(),
                kind: "tags",
                value: block.access.allowed_roles.join(", "),
                options: Vec::new(),
                read_only: block.templated,
            },
        ];
        for spec in self
            .fields
            .get(&block.block_type)
            .cloned()
            .unwrap_or_default()
        {
            let read_only = match &block.locked_props {
                LockedProps::None => false,
                LockedProps::All => true,
                LockedProps::Keys(keys) => keys.contains(&spec.key),
            };
            out.push(InspectorFieldView {
                value: display_value(block.props.get(&spec.key), spec.kind),
                kind: spec.kind.name(),
                key: spec.key,
                label: spec.label,
                options: spec.options,
                read_only,
            });
        }
        out
    }

    // ----- Configuration -----

    /// The registry's inspector fields per block type. Replaces the previous set.
    pub fn set_field_specs(&mut self, fields: HashMap<String, Vec<FieldSpec>>) {
        self.fields = fields;
    }

    /// The width text is wrapped to for caret and line positions, in logical pixels (the editor's text column).
    pub fn set_layout_width(&mut self, width: f32) {
        self.layout_width = width;
    }

    /// The base URL of the website, for the embed route of web-only blocks. None hides their preview (the rows carry an
    /// empty `web_url`).
    pub fn set_web_base(&mut self, base: Option<String>) -> Vec<ControllerEffect> {
        self.web_base = base;
        vec![ControllerEffect::RowsChanged]
    }

    /// Switches the locale. Clears the block selection and the menus.
    pub fn set_locale(&mut self, locale: &str) -> Vec<ControllerEffect> {
        self.locale = locale.to_string();
        self.rows = self.doc.rows(locale);
        self.focus = None;
        self.selection.clear();
        self.slash = None;
        self.mention = None;
        self.mention_results.clear();
        self.emoji = None;
        self.block_menu = None;
        self.link = None;
        vec![
            ControllerEffect::RowsChanged,
            ControllerEffect::MenusChanged,
            ControllerEffect::LocaleChanged {
                locale: locale.to_string(),
            },
        ]
    }

    /// The locale switcher's choice (a chip, or "Add language"). The text appears once the user types in it.
    pub fn locale_chosen(&mut self, code: &str) -> Vec<ControllerEffect> {
        if code == self.locale {
            return Vec::new();
        }
        self.set_locale(code)
    }

    /// Reloads the rows after a change made outside the controller (a co-editor's update, an import). Does not send
    /// anything to the room.
    pub fn refresh(&mut self) -> Vec<ControllerEffect> {
        self.rows = self.doc.rows(&self.locale);
        let mut out = vec![ControllerEffect::RowsChanged];
        out.extend(self.refocus(Vec::new()));
        out
    }

    /// The co-editors' cursors, from `SyncSession::remote_cursors`.
    pub fn set_remote_cursors(&mut self, cursors: Vec<RemoteCursor>) -> Vec<ControllerEffect> {
        self.remote = cursors;
        vec![ControllerEffect::RowsChanged]
    }

    /// The comments of the page, from the app. Replaces the previous set.
    pub fn set_comments(&mut self, comments: Vec<CommentView>) -> Vec<ControllerEffect> {
        self.comments = comments;
        vec![
            ControllerEffect::RowsChanged,
            ControllerEffect::MenusChanged,
        ]
    }

    // ----- Text and caret (from the TextInput of a row) -----

    /// The caret of a row, reported before every key press and every text edit. Closes a menu left on another block.
    pub fn caret_moved(&mut self, row: usize, anchor: usize, head: usize) -> Vec<ControllerEffect> {
        let Some(block) = self.rows.get(row) else {
            return Vec::new();
        };
        let id = block.id.clone();
        let moved_away = self.focus.as_ref().is_some_and(|f| f.id != id);
        self.focus = Some(Focus {
            id: id.clone(),
            anchor,
            head,
        });
        let mut out = Vec::new();
        let closes = |t: &Option<Trigger>| t.as_ref().is_some_and(|t| t.id != id);
        if closes(&self.slash) {
            self.slash = None;
            out.push(ControllerEffect::MenusChanged);
        }
        if closes(&self.emoji) {
            self.emoji = None;
            out.push(ControllerEffect::MenusChanged);
        }
        if closes(&self.mention) {
            self.mention = None;
            self.mention_results.clear();
            out.push(ControllerEffect::MenusChanged);
        }
        if moved_away && !self.selection.is_empty() && !self.selection.contains(&id) {
            self.selection.clear();
            out.push(ControllerEffect::RowsChanged);
        }
        out
    }

    /// The row's text changed (the `TextInput` already shows it). Saves it, and opens or updates the `/`, `@` and `:`
    /// menus, or runs a markdown shortcut when the paragraph's text became exactly a marker. Call
    /// [`Self::caret_moved`] first.
    pub fn text_edited(&mut self, row: usize, new_text: &str) -> Vec<ControllerEffect> {
        let Some(block) = self.rows.get(row).cloned() else {
            return Vec::new();
        };
        let Some(field) = block.inline_field.clone() else {
            return Vec::new();
        };
        let old = self.plain(&block.id, &field);
        if old == new_text {
            return Vec::new();
        }
        if block.block_type == PARAGRAPH
            && let Some(kind) = shortcut(new_text)
        {
            return self.apply_shortcut(&block, kind);
        }
        let (start, _, new_end) = changed_region(&old, new_text);
        let inserted = &new_text[start..new_end];
        // Slint reports the edit before the caret move, so the caret is taken to sit after the inserted text.
        let head = new_end;
        if let Err(err) = self
            .doc
            .replace_text(&block.id, &field, &self.locale, new_text)
        {
            return error(err);
        }
        self.focus = Some(Focus {
            id: block.id.clone(),
            anchor: head,
            head,
        });
        let mut out = self.after_change();

        let before = new_text[..start].chars().next_back();
        let at_word_start = before.is_none_or(char::is_whitespace);
        if at_word_start && matches!(inserted, "/" | "@" | ":") {
            self.open_trigger(inserted, &block.id, &field, start);
            if inserted == "@" {
                out.push(ControllerEffect::MentionSearch {
                    query: String::new(),
                });
            }
        }

        if let Some(trigger) = self.slash.clone().filter(|t| t.id == block.id) {
            match Self::query_of(new_text, &trigger, head) {
                Some(query) => {
                    self.slash = Some(Trigger {
                        query,
                        selected: 0,
                        ..trigger
                    })
                }
                None => self.slash = None,
            }
            out.push(ControllerEffect::MenusChanged);
        }
        if let Some(trigger) = self.emoji.clone().filter(|t| t.id == block.id) {
            match Self::query_of(new_text, &trigger, head) {
                Some(query) => {
                    self.emoji = Some(Trigger {
                        query,
                        selected: 0,
                        ..trigger
                    })
                }
                None => self.emoji = None,
            }
            out.push(ControllerEffect::MenusChanged);
        }
        if let Some(trigger) = self.mention.clone().filter(|t| t.id == block.id) {
            match Self::query_of(new_text, &trigger, head) {
                Some(query) => {
                    out.push(ControllerEffect::MentionSearch {
                        query: query.clone(),
                    });
                    self.mention = Some(Trigger {
                        query,
                        selected: 0,
                        ..trigger
                    });
                }
                None => {
                    self.mention = None;
                    self.mention_results.clear();
                }
            }
            out.push(ControllerEffect::MenusChanged);
        }
        out
    }

    /// Opens the menu for a trigger character (`/`, `@` or `:`) typed at byte `at`, closing the others.
    fn open_trigger(&mut self, ch: &str, id: &str, field: &str, at: usize) {
        let trigger = Trigger {
            id: id.to_string(),
            field: field.to_string(),
            at,
            query: String::new(),
            selected: 0,
        };
        self.slash = None;
        self.mention = None;
        self.emoji = None;
        self.mention_results.clear();
        match ch {
            "/" => self.slash = Some(trigger),
            "@" => self.mention = Some(trigger),
            _ => self.emoji = Some(trigger),
        }
    }

    /// The text typed after a menu trigger, or None when the menu must close (the trigger was deleted, the caret left
    /// it, or the query has a space).
    fn query_of(text: &str, trigger: &Trigger, head: usize) -> Option<String> {
        if head <= trigger.at || !text.get(trigger.at..)?.starts_with(['/', '@', ':']) {
            return None;
        }
        let query = text.get(trigger.at + 1..head)?;
        if query.contains(char::is_whitespace) {
            return None;
        }
        Some(query.to_string())
    }

    /// The key pressed in a row. Returns whether the editor took it, and what changed. Call [`Self::caret_moved`] with
    /// the caret before the key.
    pub fn key(&mut self, row: usize, input: KeyInput) -> KeyResult {
        let Some(block) = self.rows.get(row).cloned() else {
            return KeyResult::pass();
        };
        let mods = input.mods;
        let ctrl = mods.control || mods.meta;
        let (anchor, head) = self.caret_in(&block.id);

        if let Some(result) = self.menu_key(&block.id, &input) {
            return result;
        }
        if !self.selection.is_empty()
            && let Some(result) = self.block_mode_key(row, &input)
        {
            return result;
        }
        if let Some(result) = self.fallback_key(&block, &input) {
            return result;
        }

        let text = match &block.inline_field {
            Some(field) => self.plain(&block.id, field),
            None => String::new(),
        };
        match &input.key {
            Key::Enter if !mods.shift && !ctrl && !mods.alt => {
                self.key_enter(&block, &text, anchor, head)
            }
            Key::Backspace if !ctrl && !mods.alt => self.key_backspace(&block, &text, anchor, head),
            Key::Delete if !ctrl && !mods.alt => self.key_delete(row, &text, anchor, head),
            Key::Tab if !ctrl && !mods.alt => {
                let result = if mods.shift {
                    self.doc.outdent(&block.id)
                } else {
                    self.doc.indent(&block.id)
                };
                match result {
                    Ok(()) => {
                        let eff = self.after_change();
                        KeyResult::done(self.land(&block.id, anchor, head, eff))
                    }
                    // Not allowed here (the first block cannot be indented): nothing happens, the focus stays.
                    Err(_) => KeyResult::done(Vec::new()),
                }
            }
            Key::Escape if !ctrl && !mods.alt && !mods.shift => {
                self.selection = vec![block.id.clone()];
                KeyResult::done(vec![ControllerEffect::RowsChanged])
            }
            Key::Up | Key::Down if mods.alt && mods.shift && !ctrl => {
                let up = input.key == Key::Up;
                self.move_block(&block.id, up, anchor, head)
            }
            Key::Up | Key::Down if !ctrl && !mods.alt => {
                let up = input.key == Key::Up;
                self.key_vertical(row, &block, &text, head, up, mods.shift)
            }
            Key::Char(c) if ctrl => self.key_control(&block, anchor, head, *c, mods),
            _ => KeyResult::pass(),
        }
    }

    // ----- Keys, by group -----

    fn key_enter(&mut self, block: &BlockRow, text: &str, anchor: usize, head: usize) -> KeyResult {
        let Some(field) = block.inline_field.clone() else {
            return KeyResult::done(self.insert_after(block, PARAGRAPH));
        };
        let (lo, hi) = (anchor.min(head), anchor.max(head));
        let mut at = head;
        if lo != hi {
            if let Err(err) =
                self.doc
                    .replace_text(&block.id, &field, &self.locale, &without(text, lo..hi))
            {
                return KeyResult::done(error(err));
            }
            at = lo;
        }
        match self.doc.split_block(&block.id, &field, &self.locale, at) {
            Ok(new_id) => {
                let eff = self.after_change();
                KeyResult::done(self.land(&new_id, 0, 0, eff))
            }
            Err(err) => KeyResult::done(error(err)),
        }
    }

    fn key_backspace(
        &mut self,
        block: &BlockRow,
        text: &str,
        anchor: usize,
        head: usize,
    ) -> KeyResult {
        // A selection, or a caret that is not at the start: the TextInput deletes, and text_edited saves it.
        if anchor != head || head != 0 {
            return KeyResult::pass();
        }
        let Some(field) = block.inline_field.clone() else {
            return KeyResult::pass();
        };
        if block.block_type == LIST_ITEM && text.is_empty() {
            return match self.doc.outdent(&block.id) {
                Ok(()) => {
                    let eff = self.after_change();
                    KeyResult::done(self.land(&block.id, 0, 0, eff))
                }
                Err(err) => KeyResult::done(error(err)),
            };
        }
        match self
            .doc
            .merge_into_previous(&block.id, &field, &self.locale)
        {
            Ok(Some((target, caret))) => {
                let eff = self.after_change();
                KeyResult::done(self.land(&target, caret, caret, eff))
            }
            // The first text block: nothing before it to merge into.
            Ok(None) => KeyResult::done(Vec::new()),
            Err(err) => KeyResult::done(error(err)),
        }
    }

    fn key_delete(&mut self, row: usize, text: &str, anchor: usize, head: usize) -> KeyResult {
        if anchor != head || head != text.len() {
            return KeyResult::pass();
        }
        let Some(next) = self.rows.get(row + 1).cloned() else {
            return KeyResult::done(Vec::new());
        };
        let Some(next_field) = next.inline_field.clone() else {
            return KeyResult::done(Vec::new());
        };
        match self
            .doc
            .merge_into_previous(&next.id, &next_field, &self.locale)
        {
            Ok(Some((target, caret))) => {
                let eff = self.after_change();
                KeyResult::done(self.land(&target, caret, caret, eff))
            }
            Ok(None) => KeyResult::done(Vec::new()),
            Err(err) => KeyResult::done(error(err)),
        }
    }

    /// Up and Down. At the first or last visual line (soft-wrapped lines count, from the layout) the caret moves to the
    /// neighbouring block, keeping its horizontal position. Within the block the TextInput moves it. Shift at the edge
    /// selects the block (block mode).
    fn key_vertical(
        &mut self,
        row: usize,
        block: &BlockRow,
        text: &str,
        head: usize,
        up: bool,
        shift: bool,
    ) -> KeyResult {
        let (first, last) = self.edges(block, text, head);
        if (up && !first) || (!up && !last) {
            return KeyResult::pass();
        }
        let target = if up {
            row.checked_sub(1)
        } else {
            Some(row + 1).filter(|r| *r < self.rows.len())
        };
        let Some(target) = target else {
            return KeyResult::done(Vec::new());
        };
        let neighbour = self.rows[target].clone();
        if shift {
            self.selection = vec![block.id.clone(), neighbour.id.clone()];
            return KeyResult::done(vec![ControllerEffect::RowsChanged]);
        }
        // The x position of the caret in this block, found on the neighbour's line nearest to it.
        let column = match &neighbour.inline_field {
            Some(field) => {
                let x = caret_rect(
                    text,
                    font_size(&block.block_type, block.heading_level),
                    self.layout_width,
                    clamp(text, head),
                )
                .0;
                let neighbour_text = self.plain(&neighbour.id, field);
                // Up lands on the neighbour's last line, Down on its first.
                offset_at(
                    &neighbour_text,
                    font_size(&neighbour.block_type, neighbour.heading_level),
                    self.layout_width,
                    x,
                    if up { usize::MAX } else { 0 },
                )
            }
            None => 0,
        };
        KeyResult::done(self.land(&neighbour.id, column, column, Vec::new()))
    }

    /// Ctrl or Meta with a letter. Ctrl+Alt+M comments; other Alt chords are left alone.
    fn key_control(
        &mut self,
        block: &BlockRow,
        anchor: usize,
        head: usize,
        letter: char,
        mods: Mods,
    ) -> KeyResult {
        let lower = letter.to_ascii_lowercase();
        if mods.alt {
            return if lower == 'm' {
                KeyResult::done(self.comment_for_selection())
            } else {
                KeyResult::pass()
            };
        }
        let range = anchor.min(head)..anchor.max(head);
        let mark = match lower {
            'b' => Some(MarkKind::Bold),
            'i' => Some(MarkKind::Italic),
            'u' => Some(MarkKind::Underline),
            'e' => Some(MarkKind::Code),
            'x' if mods.shift => Some(MarkKind::Strikethrough),
            _ => None,
        };
        if let Some(mark) = mark {
            if block.inline_field.is_none() {
                return KeyResult::done(Vec::new());
            }
            return KeyResult::done(self.apply_mark(block, anchor, head, range, mark));
        }
        match lower {
            'k' if block.inline_field.is_some() => KeyResult::done(self.open_link()),
            'd' if !mods.shift => {
                if !self.selection.is_empty() {
                    return KeyResult::done(self.duplicate_selection());
                }
                match self.doc.duplicate(&block.id) {
                    Ok(new_id) => {
                        let eff = self.after_change();
                        KeyResult::done(self.land(&new_id, anchor, head, eff))
                    }
                    Err(err) => KeyResult::done(error(err)),
                }
            }
            'z' => {
                if mods.shift {
                    self.doc.redo();
                } else {
                    self.doc.undo();
                }
                let eff = self.after_change();
                KeyResult::done(self.refocus(eff))
            }
            'y' => {
                self.doc.redo();
                let eff = self.after_change();
                KeyResult::done(self.refocus(eff))
            }
            _ => KeyResult::pass(),
        }
    }

    /// Typing into a row whose text is empty in this locale (it shows the `en` text, read-only). The first character
    /// creates the locale's text: the `en` text is copied in, then the character is added at its end.
    fn fallback_key(&mut self, block: &BlockRow, input: &KeyInput) -> Option<KeyResult> {
        if self.locale == "en" {
            return None;
        }
        let field = block.inline_field.clone()?;
        if !self.is_fallback(&block.id, &field) {
            return None;
        }
        let ctrl = input.mods.control || input.mods.meta;
        match &input.key {
            Key::Char(c) if !ctrl && !input.mods.alt => {
                let base = self.doc.text(&block.id, &field, "en").plain;
                let typed = format!("{base}{c}");
                if let Err(err) = self
                    .doc
                    .replace_text(&block.id, &field, &self.locale, &typed)
                {
                    return Some(KeyResult::done(error(err)));
                }
                let caret = typed.len();
                let eff = self.after_change();
                Some(KeyResult::done(self.land(&block.id, caret, caret, eff)))
            }
            // Read-only until the user types: these keys do nothing.
            Key::Enter | Key::Backspace | Key::Delete => Some(KeyResult::done(Vec::new())),
            _ => None,
        }
    }

    /// Whether a field has no text in the current locale while `en` has some to show (the row is a fallback).
    fn is_fallback(&self, id: &str, field: &str) -> bool {
        self.locale != "en" && self.doc.text(id, field, &self.locale).plain.is_empty()
    }

    /// Keys while a menu is open. None when the key is not the menu's.
    fn menu_key(&mut self, id: &str, input: &KeyInput) -> Option<KeyResult> {
        if let Some(trigger) = self.slash.clone().filter(|t| t.id == id) {
            let count = self.insert_menu_items().len();
            match menu_step(&input.key, trigger.selected, count) {
                MenuStep::Pass => {}
                MenuStep::Move(to) => {
                    if let Some(t) = self.slash.as_mut() {
                        t.selected = to;
                    }
                    return Some(KeyResult::done(vec![ControllerEffect::MenusChanged]));
                }
                MenuStep::Close => {
                    self.slash = None;
                    return Some(KeyResult::done(vec![ControllerEffect::MenusChanged]));
                }
                MenuStep::Choose(index) => {
                    let block_type = self.insert_menu_items()[index].block_type.clone();
                    return Some(KeyResult::done(self.insert_menu_chosen(&block_type)));
                }
            }
        }
        if let Some(trigger) = self.mention.clone().filter(|t| t.id == id) {
            let count = self.mention_results.len();
            match menu_step(&input.key, trigger.selected, count) {
                MenuStep::Pass => {}
                MenuStep::Move(to) => {
                    if let Some(t) = self.mention.as_mut() {
                        t.selected = to;
                    }
                    return Some(KeyResult::done(vec![ControllerEffect::MenusChanged]));
                }
                MenuStep::Close => {
                    self.mention = None;
                    self.mention_results.clear();
                    return Some(KeyResult::done(vec![ControllerEffect::MenusChanged]));
                }
                MenuStep::Choose(index) => {
                    return Some(KeyResult::done(self.mention_chosen(index)));
                }
            }
        }
        if let Some(trigger) = self.emoji.clone().filter(|t| t.id == id) {
            let count = self.emoji_menu_items().len();
            match menu_step(&input.key, trigger.selected, count) {
                MenuStep::Pass => {}
                MenuStep::Move(to) => {
                    if let Some(t) = self.emoji.as_mut() {
                        t.selected = to;
                    }
                    return Some(KeyResult::done(vec![ControllerEffect::MenusChanged]));
                }
                MenuStep::Close => {
                    self.emoji = None;
                    return Some(KeyResult::done(vec![ControllerEffect::MenusChanged]));
                }
                MenuStep::Choose(index) => return Some(KeyResult::done(self.emoji_chosen(index))),
            }
        }
        if let Some(menu) = self.block_menu.clone().filter(|m| m.id == id) {
            let count = BLOCK_ACTIONS.len();
            match menu_step(&input.key, menu.selected, count) {
                MenuStep::Pass => {}
                MenuStep::Move(to) => {
                    if let Some(m) = self.block_menu.as_mut() {
                        m.selected = to;
                    }
                    return Some(KeyResult::done(vec![ControllerEffect::MenusChanged]));
                }
                MenuStep::Close => {
                    self.block_menu = None;
                    return Some(KeyResult::done(vec![ControllerEffect::MenusChanged]));
                }
                MenuStep::Choose(index) => {
                    return Some(KeyResult::done(
                        self.block_menu_chosen(BLOCK_ACTIONS[index].0),
                    ));
                }
            }
        }
        None
    }

    /// Keys in block mode. None when the key falls through to the text mode rules.
    fn block_mode_key(&mut self, row: usize, input: &KeyInput) -> Option<KeyResult> {
        let mods = input.mods;
        let ctrl = mods.control || mods.meta;
        match &input.key {
            Key::Escape => {
                self.selection.clear();
                Some(KeyResult::done(vec![ControllerEffect::RowsChanged]))
            }
            Key::Backspace | Key::Delete => Some(KeyResult::done(self.delete_selection())),
            Key::Up | Key::Down if mods.alt && mods.shift => {
                let up = input.key == Key::Up;
                Some(KeyResult::done(self.move_selection(up)))
            }
            Key::Char(c) if ctrl && c.eq_ignore_ascii_case(&'d') => {
                Some(KeyResult::done(self.duplicate_selection()))
            }
            Key::Enter if !mods.shift && !ctrl => {
                let first = self.selection_in_order().into_iter().next()?;
                self.selection.clear();
                let caret = self
                    .block_by_id(&first)
                    .and_then(|b| b.inline_field.map(|f| self.plain(&first, &f).len()))
                    .unwrap_or(0);
                let eff = self.after_change();
                Some(KeyResult::done(self.land(&first, caret, caret, eff)))
            }
            Key::Up | Key::Down => {
                // Leave block mode and move to the neighbour.
                let neighbour = if input.key == Key::Up {
                    row.checked_sub(1)
                } else {
                    Some(row + 1).filter(|r| *r < self.rows.len())
                };
                self.selection.clear();
                let mut eff = vec![ControllerEffect::RowsChanged];
                if let Some(target) = neighbour {
                    let id = self.rows[target].id.clone();
                    eff = self.land(&id, 0, 0, eff);
                }
                Some(KeyResult::done(eff))
            }
            Key::Char(_) if !ctrl && !mods.alt => {
                // Typing leaves block mode; the character goes into the focused text.
                self.selection.clear();
                Some(KeyResult {
                    handled: false,
                    effects: vec![ControllerEffect::RowsChanged],
                })
            }
            _ => None,
        }
    }

    // ----- Menus, toolbar, inspector and pointer actions -----

    /// The insert menu's entry was chosen. Replaces the `/query` text with the new block: a block that is left empty
    /// turns into the type, otherwise the new block follows this one.
    pub fn insert_menu_chosen(&mut self, block_type: &str) -> Vec<ControllerEffect> {
        let Some(trigger) = self.slash.take() else {
            return self.insert_after_focus(block_type);
        };
        let Some(block) = self.block_by_id(&trigger.id) else {
            return Vec::new();
        };
        let head = self.caret_in(&trigger.id).1;
        let text = self.plain(&trigger.id, &trigger.field);
        let end = clamp(&text, head.max(trigger.at));
        let remaining = without(&text, trigger.at..end);
        if let Err(err) =
            self.doc
                .replace_text(&trigger.id, &trigger.field, &self.locale, &remaining)
        {
            return error(err);
        }
        let turned = remaining.is_empty() && self.doc.turn_into(&block.id, block_type).is_ok();
        let focus_id = if turned {
            block.id.clone()
        } else {
            match self
                .doc
                .insert_block(block.parent.as_deref(), Some(&block.id), block_type)
            {
                Ok(new_id) => new_id,
                Err(err) => return error(err),
            }
        };
        let eff = self.after_change();
        self.land(&focus_id, 0, 0, eff)
    }

    /// The insert menu was closed by a pointer press outside it.
    pub fn insert_menu_dismissed(&mut self) -> Vec<ControllerEffect> {
        self.slash = None;
        vec![ControllerEffect::MenusChanged]
    }

    /// The mention picker's own filter changed.
    pub fn mention_query(&mut self, text: &str) -> Vec<ControllerEffect> {
        if self.mention.is_none() {
            return Vec::new();
        }
        vec![ControllerEffect::MentionSearch {
            query: text.to_string(),
        }]
    }

    /// The app's answer to a `MentionSearch`.
    pub fn set_mention_results(&mut self, results: Vec<MentionChoice>) -> Vec<ControllerEffect> {
        self.mention_results = results;
        vec![ControllerEffect::MenusChanged]
    }

    /// A mention was chosen. Replaces `@query` with the mention, as an atomic run of text.
    pub fn mention_chosen(&mut self, index: usize) -> Vec<ControllerEffect> {
        let Some(choice) = self.mention_results.get(index).cloned() else {
            return Vec::new();
        };
        let Some(trigger) = self.mention.take() else {
            return Vec::new();
        };
        self.mention_results.clear();
        let head = self.caret_in(&trigger.id).1;
        let text = self.plain(&trigger.id, &trigger.field);
        let end = clamp(&text, head.max(trigger.at));
        let remaining = without(&text, trigger.at..end);
        if let Err(err) =
            self.doc
                .replace_text(&trigger.id, &trigger.field, &self.locale, &remaining)
        {
            return error(err);
        }
        let mention = Mention {
            entry_id: choice.entry_id,
            collection: choice.collection,
            slug: choice.slug,
            title: Some(choice.title),
        };
        match self.doc.insert_mention(
            &trigger.id,
            &trigger.field,
            &self.locale,
            trigger.at,
            mention,
        ) {
            Ok(caret) => {
                let eff = self.after_change();
                self.land(&trigger.id, caret, caret, eff)
            }
            Err(err) => error(err),
        }
    }

    /// The mention picker was closed by a pointer press outside it.
    pub fn mention_dismissed(&mut self) -> Vec<ControllerEffect> {
        self.mention = None;
        self.mention_results.clear();
        vec![ControllerEffect::MenusChanged]
    }

    /// An emoji was chosen from the `:` picker. Replaces `:query` with the emoji.
    pub fn emoji_chosen(&mut self, index: usize) -> Vec<ControllerEffect> {
        let Some(choice) = self.emoji_menu_items().get(index).cloned() else {
            return Vec::new();
        };
        let Some(trigger) = self.emoji.take() else {
            return Vec::new();
        };
        let head = self.caret_in(&trigger.id).1;
        let text = self.plain(&trigger.id, &trigger.field);
        let end = clamp(&text, head.max(trigger.at));
        let remaining = without(&text, trigger.at..end);
        if let Err(err) =
            self.doc
                .replace_text(&trigger.id, &trigger.field, &self.locale, &remaining)
        {
            return error(err);
        }
        match self.doc.insert_emoji(
            &trigger.id,
            &trigger.field,
            &self.locale,
            trigger.at,
            &choice.emoji,
            &choice.shortcode,
        ) {
            Ok(caret) => {
                let eff = self.after_change();
                self.land(&trigger.id, caret, caret, eff)
            }
            Err(err) => error(err),
        }
    }

    /// The emoji picker was closed by a pointer press outside it.
    pub fn emoji_dismissed(&mut self) -> Vec<ControllerEffect> {
        self.emoji = None;
        vec![ControllerEffect::MenusChanged]
    }

    /// A row's handle was clicked: the block becomes the selection (block mode).
    pub fn handle_clicked(&mut self, row: usize) -> Vec<ControllerEffect> {
        let Some(block) = self.rows.get(row) else {
            return Vec::new();
        };
        self.selection = vec![block.id.clone()];
        vec![ControllerEffect::RowsChanged]
    }

    /// The page's title or description (`key` is "title" or "description") in the current locale, else English.
    pub fn meta_text(&self, key: &str) -> String {
        let Ok(page) = self.doc.page() else {
            return String::new();
        };
        let value = &page["meta"][key];
        value[self.locale.as_str()]
            .as_str()
            .or_else(|| value["en"].as_str())
            .or_else(|| value.as_str())
            .unwrap_or_default()
            .to_string()
    }

    /// The title or description was edited in the current locale.
    pub fn meta_edited(&mut self, key: &str, text: &str) -> Vec<ControllerEffect> {
        match self.doc.set_meta_text(key, &self.locale, text) {
            Ok(()) => vec![ControllerEffect::SyncNeeded],
            Err(err) => error(err),
        }
    }

    /// The page's collection properties (`meta.properties`), for the page panel.
    pub fn properties(&self) -> serde_json::Map<String, Value> {
        self.doc
            .page()
            .ok()
            .and_then(|p| p["content"]["properties"].as_object().cloned())
            .unwrap_or_default()
    }

    /// One collection property changed in the page panel.
    pub fn property_changed(&mut self, key: &str, value: Value) -> Vec<ControllerEffect> {
        match self.doc.set_property(key, value) {
            Ok(()) => vec![ControllerEffect::SyncNeeded],
            Err(err) => error(err),
        }
    }

    /// The row's + button (the web editor's "add block below"): inserts an empty block after the row and opens the
    /// insert menu on it. Inside a list the new block is a list item, elsewhere a paragraph.
    pub fn add_below(&mut self, row: usize) -> Vec<ControllerEffect> {
        let Some(block) = self.rows.get(row).cloned() else {
            return Vec::new();
        };
        let kind = if block.block_type == LIST_ITEM {
            LIST_ITEM
        } else {
            PARAGRAPH
        };
        match self
            .doc
            .insert_block(block.parent.as_deref(), Some(&block.id), kind)
        {
            Ok(new_id) => {
                let eff = self.after_change();
                let mut out = self.land(&new_id, 0, 0, eff);
                out.extend(self.type_at_caret("/"));
                out
            }
            Err(err) => error(err),
        }
    }

    /// A row was long-pressed (phone grip) or right-clicked (desktop handle): opens the block menu for it.
    pub fn long_pressed(&mut self, row: usize) -> Vec<ControllerEffect> {
        let Some(block) = self.rows.get(row) else {
            return Vec::new();
        };
        self.selection = vec![block.id.clone()];
        self.block_menu = Some(BlockMenuState {
            id: block.id.clone(),
            selected: 0,
        });
        vec![
            ControllerEffect::RowsChanged,
            ControllerEffect::MenusChanged,
        ]
    }

    /// The block menu's entry was chosen (an action name from [`BLOCK_ACTIONS`]).
    pub fn block_menu_chosen(&mut self, action: &str) -> Vec<ControllerEffect> {
        let Some(menu) = self.block_menu.take() else {
            return Vec::new();
        };
        let id = menu.id;
        let mut out = match action {
            "comment" => self.open_comments(Some(id.clone()), vec![id.clone()]),
            "duplicate" => match self.doc.duplicate(&id) {
                Ok(new_id) => {
                    let eff = self.after_change();
                    self.land(&new_id, 0, 0, eff)
                }
                Err(err) => error(err),
            },
            "delete" => {
                self.selection = vec![id.clone()];
                self.delete_selection()
            }
            "indent" | "outdent" | "move-up" | "move-down" => {
                let result = match action {
                    "indent" => self.doc.indent(&id),
                    "outdent" => self.doc.outdent(&id),
                    "move-up" => self.doc.move_up(&id),
                    _ => self.doc.move_down(&id),
                };
                match result {
                    Ok(()) => {
                        let eff = self.after_change();
                        self.land(&id, 0, 0, eff)
                    }
                    Err(err) => error(err),
                }
            }
            _ => Vec::new(),
        };
        out.push(ControllerEffect::MenusChanged);
        out
    }

    /// The block menu was closed by a pointer press outside it.
    pub fn block_menu_dismissed(&mut self) -> Vec<ControllerEffect> {
        self.block_menu = None;
        vec![ControllerEffect::MenusChanged]
    }

    /// A block was dropped before row `to`, at `depth`. Its parent is the nearest row above the drop point one level
    /// shallower; its index is the number of siblings above the drop point.
    pub fn drag_dropped(&mut self, from: usize, to: usize, depth: usize) -> Vec<ControllerEffect> {
        let Some(block) = self.rows.get(from).cloned() else {
            return Vec::new();
        };
        let above = &self.rows[..to.min(self.rows.len())];
        let parent = if depth == 0 {
            None
        } else {
            above
                .iter()
                .rev()
                .find(|r| r.depth + 1 == depth)
                .map(|r| r.id.clone())
        };
        let index = above
            .iter()
            .filter(|r| r.parent == parent && r.id != block.id)
            .count();
        self.block_menu = None;
        match self.doc.move_block(&block.id, parent.as_deref(), index) {
            Ok(()) => {
                self.selection = vec![block.id.clone()];
                self.after_change()
            }
            Err(err) => error(err),
        }
    }

    /// A toolbar action: `bold`, `italic`, `underline`, `strike`, `code`, `link`, `mention`, `emoji`, `insert`,
    /// `block-menu`, `indent`, `outdent`, `move-up`, `move-down`, `comment` or `done`. Unknown names do nothing.
    pub fn toolbar(&mut self, action: &str) -> Vec<ControllerEffect> {
        // Undo and redo of this user's own changes (the web toolbar's arrows).
        if action == "undo" || action == "redo" {
            let changed = if action == "undo" {
                self.doc.undo()
            } else {
                self.doc.redo()
            };
            return if changed {
                self.after_change()
            } else {
                Vec::new()
            };
        }
        let mark = match action {
            "bold" => Some(MarkKind::Bold),
            "italic" => Some(MarkKind::Italic),
            "underline" => Some(MarkKind::Underline),
            "strike" => Some(MarkKind::Strikethrough),
            "code" => Some(MarkKind::Code),
            _ => None,
        };
        if let Some(mark) = mark {
            let Some((block, anchor, head)) = self.focus_with_caret() else {
                return Vec::new();
            };
            let range = anchor.min(head)..anchor.max(head);
            return self.apply_mark(&block, anchor, head, range, mark);
        }
        match action {
            "link" => self.open_link(),
            "mention" => self.type_at_caret("@"),
            "emoji" => self.type_at_caret(":"),
            "insert" => self.type_at_caret("/"),
            "block-menu" => match self.focus_block().and_then(|b| self.index_of(&b.id)) {
                Some(row) => self.long_pressed(row),
                None => Vec::new(),
            },
            "indent" | "outdent" | "move-up" | "move-down" => {
                let Some((block, anchor, head)) = self.focus_with_caret() else {
                    return Vec::new();
                };
                match action {
                    "indent" => match self.doc.indent(&block.id) {
                        Ok(()) => {
                            let eff = self.after_change();
                            self.land(&block.id, anchor, head, eff)
                        }
                        Err(_) => Vec::new(),
                    },
                    "outdent" => match self.doc.outdent(&block.id) {
                        Ok(()) => {
                            let eff = self.after_change();
                            self.land(&block.id, anchor, head, eff)
                        }
                        Err(_) => Vec::new(),
                    },
                    _ => {
                        let up = action == "move-up";
                        self.move_block(&block.id, up, anchor, head).effects
                    }
                }
            }
            "comment" => self.comment_for_selection(),
            "done" => {
                if let Some(block) = self.focus_block() {
                    self.selection = vec![block.id];
                }
                vec![ControllerEffect::Done, ControllerEffect::RowsChanged]
            }
            _ => Vec::new(),
        }
    }

    /// Ctrl+K or the toolbar's Link: opens the link prompt for the selected text. It fills in the existing link when
    /// the selection already has one.
    fn open_link(&mut self) -> Vec<ControllerEffect> {
        let Some((block, anchor, head)) = self.focus_with_caret() else {
            return Vec::new();
        };
        let Some(field) = block.inline_field.clone() else {
            return Vec::new();
        };
        let range = anchor.min(head)..anchor.max(head);
        if range.is_empty() {
            return error("Select the text to link first");
        }
        let existing = self
            .doc
            .text(&block.id, &field, &self.locale)
            .spans
            .into_iter()
            .find(|s| s.range.start < range.end && range.start < s.range.end)
            .and_then(|s| s.link);
        self.link = Some(LinkState {
            id: block.id,
            field,
            range,
            anchor,
            head,
            has_link: existing.is_some(),
            url: existing.unwrap_or_default(),
        });
        vec![ControllerEffect::MenusChanged]
    }

    /// The link prompt's Apply. An empty URL removes the link. `new_tab` is accepted for the markup's toggle; the
    /// document model has no flag for it yet, so it is not stored.
    pub fn link_submit(&mut self, url: &str, new_tab: bool) -> Vec<ControllerEffect> {
        let _ = new_tab; // UNSURE: no new-tab flag in rimelight_content link spans yet.
        let Some(link) = self.link.take() else {
            return Vec::new();
        };
        let url = url.trim();
        let target = (!url.is_empty()).then_some(url);
        match self.doc.set_link(
            &link.id,
            &link.field,
            &self.locale,
            link.range.clone(),
            target,
        ) {
            Ok(()) => {
                let eff = self.after_change();
                let mut eff = self.land(&link.id, link.anchor, link.head, eff);
                eff.push(ControllerEffect::MenusChanged);
                eff
            }
            Err(err) => error(err),
        }
    }

    /// The link prompt's Remove link.
    pub fn link_remove(&mut self) -> Vec<ControllerEffect> {
        self.link_submit("", false)
    }

    /// The link prompt was cancelled.
    pub fn link_dismissed(&mut self) -> Vec<ControllerEffect> {
        self.link = None;
        vec![ControllerEffect::MenusChanged]
    }

    /// Opens the comment panel for the selected blocks (the first one scopes the panel), or for the page.
    fn comment_for_selection(&mut self) -> Vec<ControllerEffect> {
        let ids = if self.selection.is_empty() {
            self.focus_block().map(|b| vec![b.id]).unwrap_or_default()
        } else {
            self.selection_in_order()
        };
        let scope = ids.first().cloned();
        self.open_comments(scope, ids)
    }

    fn open_comments(&mut self, scope: Option<String>, ids: Vec<String>) -> Vec<ControllerEffect> {
        self.comment_scope = Some(scope.unwrap_or_default());
        vec![
            ControllerEffect::CommentRequested { ids },
            ControllerEffect::MenusChanged,
        ]
    }

    /// The panel's scope changed: `""` for the page, else a block id.
    pub fn comment_scope_changed(&mut self, scope: &str) -> Vec<ControllerEffect> {
        let ids = if scope.is_empty() {
            Vec::new()
        } else {
            vec![scope.to_string()]
        };
        self.open_comments(Some(scope.to_string()), ids)
    }

    /// The comment panel was closed.
    pub fn comment_closed(&mut self) -> Vec<ControllerEffect> {
        self.comment_scope = None;
        vec![ControllerEffect::MenusChanged]
    }

    /// A reply or a new thread. `parent_id` empty starts a thread; `block_id` empty is a page-level comment.
    pub fn comment_submit(
        &mut self,
        block_id: &str,
        parent_id: &str,
        body: &str,
    ) -> Vec<ControllerEffect> {
        let body = body.trim();
        if body.is_empty() {
            return error("A comment needs some text");
        }
        vec![ControllerEffect::Comment {
            request: CommentRequest::Submit {
                block_id: block_id.to_string(),
                parent_id: (!parent_id.is_empty()).then(|| parent_id.to_string()),
                body: body.to_string(),
            },
        }]
    }

    pub fn comment_resolve(&mut self, id: &str) -> Vec<ControllerEffect> {
        vec![ControllerEffect::Comment {
            request: CommentRequest::Resolve { id: id.to_string() },
        }]
    }

    pub fn comment_reopen(&mut self, id: &str) -> Vec<ControllerEffect> {
        vec![ControllerEffect::Comment {
            request: CommentRequest::Reopen { id: id.to_string() },
        }]
    }

    /// The panel's "Show": focuses the block a comment is anchored to.
    pub fn comment_jump(&mut self, block_id: &str) -> Vec<ControllerEffect> {
        if self.index_of(block_id).is_none() {
            return Vec::new();
        }
        self.land(block_id, 0, 0, Vec::new())
    }

    /// An inspector field changed. `key` is the field key. `raw` is the new value as text: the JSON text for a JSON
    /// field, the typed text for the others (booleans "true" or "false", tags comma-separated). The editor converts it
    /// by the field's kind.
    pub fn inspector_changed(&mut self, key: &str, raw: &str) -> Vec<ControllerEffect> {
        let Some(block) = self.focus_block() else {
            return Vec::new();
        };
        let result = match key {
            "visibility" => self.doc.set_access(
                &block.id,
                BlockAccess {
                    visibility: if raw.is_empty() || raw == "everyone" {
                        None
                    } else {
                        Some(raw.to_string())
                    },
                    allowed_roles: block.access.allowed_roles.clone(),
                    required_permission: block.access.required_permission.clone(),
                },
            ),
            "allowedRoles" => self.doc.set_access(
                &block.id,
                BlockAccess {
                    visibility: block.access.visibility.clone(),
                    allowed_roles: parse_tags(raw),
                    required_permission: block.access.required_permission.clone(),
                },
            ),
            _ => {
                let kind = self
                    .fields
                    .get(&block.block_type)
                    .and_then(|specs| specs.iter().find(|s| s.key == key))
                    .map(|s| s.kind);
                let Some(kind) = kind else {
                    return error(format!("No inspector field '{key}' on this block"));
                };
                match value_for(kind, raw) {
                    Ok(value) => self.doc.set_prop(&block.id, key, value),
                    Err(message) => return error(message),
                }
            }
        };
        match result {
            Ok(()) => {
                let eff = self.after_change();
                self.refocus(eff)
            }
            Err(err) => error(err),
        }
    }

    /// Undo of the local user's own changes.
    pub fn undo(&mut self) -> Vec<ControllerEffect> {
        self.doc.undo();
        let eff = self.after_change();
        self.refocus(eff)
    }

    /// Redo of the local user's own changes.
    pub fn redo(&mut self) -> Vec<ControllerEffect> {
        self.doc.redo();
        let eff = self.after_change();
        self.refocus(eff)
    }

    // ----- Block operations shared by keys, toolbar and menus -----

    /// Applies a markdown shortcut to a paragraph whose text is exactly the marker.
    fn apply_shortcut(&mut self, block: &BlockRow, kind: Shortcut) -> Vec<ControllerEffect> {
        if let Some(field) = &block.inline_field
            && let Err(err) = self.doc.replace_text(&block.id, field, &self.locale, "")
        {
            return error(err);
        }
        let same = |r: Result<(), rimelight_content::Error>| r.map(|()| block.id.clone());
        let result = match kind {
            Shortcut::Section => same(self.doc.turn_into(&block.id, SECTION)),
            Shortcut::Divider => same(self.doc.turn_into(&block.id, DIVIDER)),
            Shortcut::Code => same(self.doc.turn_into(&block.id, CODE)),
            Shortcut::Quote => same(self.doc.turn_into(&block.id, QUOTE)),
            Shortcut::UnorderedList => self.wrap_in_list(block, UNORDERED_LIST),
            Shortcut::OrderedList => self.wrap_in_list(block, ORDERED_LIST),
        };
        match result {
            Ok(target) => {
                let eff = self.after_change();
                self.land(&target, 0, 0, eff)
            }
            Err(err) => error(err),
        }
    }

    /// Makes `block` a list item of a list of `list_type` and returns the item to put the caret in. Inside a list already,
    /// the block becomes an item of that list. Elsewhere a new list takes the block's place: a paragraph cannot sit
    /// inside a list, so the list's first (initial) item receives the block's text and the block is removed.
    fn wrap_in_list(
        &mut self,
        block: &BlockRow,
        list_type: &str,
    ) -> Result<String, rimelight_content::Error> {
        let in_list = block
            .parent
            .as_deref()
            .and_then(|p| self.rows.iter().find(|r| r.id == p))
            .is_some_and(|p| LIST_TYPES.contains(&p.block_type.as_str()));
        if in_list {
            self.doc.turn_into(&block.id, LIST_ITEM)?;
            return Ok(block.id.clone());
        }
        let text = block
            .inline_field
            .as_deref()
            .map(|field| self.doc.text(&block.id, field, &self.locale).plain)
            .unwrap_or_default();
        let list = self
            .doc
            .insert_block(block.parent.as_deref(), Some(&block.id), list_type)?;
        let item = self
            .doc
            .rows(&self.locale)
            .into_iter()
            .find(|r| r.parent.as_deref() == Some(list.as_str()) && r.block_type == LIST_ITEM)
            .map(|r| r.id)
            .ok_or_else(|| rimelight_content::Error::NotFound(format!("list item of {list}")))?;
        if !text.is_empty() {
            self.doc
                .replace_text(&item, "content", &self.locale, &text)?;
        }
        self.doc.delete_blocks(std::slice::from_ref(&block.id))?;
        Ok(item)
    }

    fn insert_after(&mut self, block: &BlockRow, block_type: &str) -> Vec<ControllerEffect> {
        match self
            .doc
            .insert_block(block.parent.as_deref(), Some(&block.id), block_type)
        {
            Ok(new_id) => {
                let eff = self.after_change();
                self.land(&new_id, 0, 0, eff)
            }
            Err(err) => error(err),
        }
    }

    /// Inserts after the focused block, or at the end of the page when nothing is focused.
    fn insert_after_focus(&mut self, block_type: &str) -> Vec<ControllerEffect> {
        if let Some(block) = self.focus_block() {
            return self.insert_after(&block, block_type);
        }
        let last_root = self.rows.iter().rev().find(|r| r.depth == 0).cloned();
        match last_root {
            Some(block) => self.insert_after(&block, block_type),
            None => match self.doc.insert_block(None, None, block_type) {
                Ok(new_id) => {
                    let eff = self.after_change();
                    self.land(&new_id, 0, 0, eff)
                }
                Err(err) => error(err),
            },
        }
    }

    /// Types `ch` at the caret (replacing a selection), then runs the same rules as a typed key. Used by the toolbar.
    fn type_at_caret(&mut self, ch: &str) -> Vec<ControllerEffect> {
        let Some((block, anchor, head)) = self.focus_with_caret() else {
            return Vec::new();
        };
        let Some(field) = block.inline_field.clone() else {
            return Vec::new();
        };
        let (lo, hi) = (anchor.min(head), anchor.max(head));
        let text = self.plain(&block.id, &field);
        let removed = without(&text, lo..hi);
        let mut typed = removed.clone();
        typed.insert_str(lo, ch);
        let caret = lo + ch.len();
        let Some(row) = self.index_of(&block.id) else {
            return Vec::new();
        };
        self.focus = Some(Focus {
            id: block.id.clone(),
            anchor: caret,
            head: caret,
        });
        let mut out = self.text_edited(row, &typed);
        out.insert(0, ControllerEffect::RowsChanged);
        out
    }

    fn apply_mark(
        &mut self,
        block: &BlockRow,
        anchor: usize,
        head: usize,
        range: Range<usize>,
        mark: MarkKind,
    ) -> Vec<ControllerEffect> {
        let Some(field) = block.inline_field.clone() else {
            return Vec::new();
        };
        if range.is_empty() {
            return Vec::new();
        }
        match self
            .doc
            .toggle_mark(&block.id, &field, &self.locale, range, mark)
        {
            Ok(()) => {
                let eff = self.after_change();
                self.land(&block.id, anchor, head, eff)
            }
            Err(err) => error(err),
        }
    }

    /// Moves a block up or down among its siblings, keeping the caret.
    fn move_block(&mut self, id: &str, up: bool, anchor: usize, head: usize) -> KeyResult {
        let result = if up {
            self.doc.move_up(id)
        } else {
            self.doc.move_down(id)
        };
        match result {
            Ok(()) => {
                let eff = self.after_change();
                KeyResult::done(self.land(id, anchor, head, eff))
            }
            Err(err) => KeyResult::done(error(err)),
        }
    }

    /// Moves every selected block one step, in display order (the last one first when moving down).
    fn move_selection(&mut self, up: bool) -> Vec<ControllerEffect> {
        let mut ids = self.selection_in_order();
        if !up {
            ids.reverse();
        }
        for id in &ids {
            let result = if up {
                self.doc.move_up(id)
            } else {
                self.doc.move_down(id)
            };
            if let Err(err) = result {
                let mut eff = self.after_change();
                eff.extend(error(err));
                return eff;
            }
        }
        self.after_change()
    }

    fn duplicate_selection(&mut self) -> Vec<ControllerEffect> {
        let ids = self.selection_in_order();
        let mut copies = Vec::new();
        for id in &ids {
            match self.doc.duplicate(id) {
                Ok(new_id) => copies.push(new_id),
                Err(err) => return error(err),
            }
        }
        self.selection = copies;
        self.after_change()
    }

    /// Deletes the selected blocks. The focus goes to the block that now sits where the first one was.
    fn delete_selection(&mut self) -> Vec<ControllerEffect> {
        let ids = self.selection_in_order();
        let first = ids.first().and_then(|id| self.index_of(id)).unwrap_or(0);
        self.selection.clear();
        if let Err(err) = self.doc.delete_blocks(&ids) {
            return error(err);
        }
        let mut eff = self.after_change();
        if let Some(block) = self
            .rows
            .get(first.min(self.rows.len().saturating_sub(1)))
            .cloned()
        {
            eff = self.land(&block.id, 0, 0, eff);
        } else {
            self.focus = None;
        }
        eff
    }

    // ----- Helpers -----

    /// After a document change: the rows are read again and the room is told.
    fn after_change(&mut self) -> Vec<ControllerEffect> {
        self.rows = self.doc.rows(&self.locale);
        vec![ControllerEffect::RowsChanged, ControllerEffect::SyncNeeded]
    }

    /// Moves the focus to a block at a caret, and tells the app.
    fn land(
        &mut self,
        id: &str,
        anchor: usize,
        head: usize,
        mut eff: Vec<ControllerEffect>,
    ) -> Vec<ControllerEffect> {
        self.focus = Some(Focus {
            id: id.to_string(),
            anchor,
            head,
        });
        if let Some(row) = self.index_of(id) {
            eff.push(ControllerEffect::Focus { row, anchor, head });
        }
        eff
    }

    /// Puts the focus back on the focused block after a change that kept it, clamping the caret to the text.
    fn refocus(&mut self, mut eff: Vec<ControllerEffect>) -> Vec<ControllerEffect> {
        let Some(focus) = self.focus.clone() else {
            return eff;
        };
        let Some(row) = self.index_of(&focus.id) else {
            self.focus = None;
            return eff;
        };
        let text = self.rows[row]
            .inline_field
            .clone()
            .map(|f| self.plain(&focus.id, &f))
            .unwrap_or_default();
        let anchor = clamp(&text, focus.anchor);
        let head = clamp(&text, focus.head);
        self.focus = Some(Focus {
            id: focus.id,
            anchor,
            head,
        });
        eff.push(ControllerEffect::Focus { row, anchor, head });
        eff
    }

    fn index_of(&self, id: &str) -> Option<usize> {
        self.rows.iter().position(|r| r.id == id)
    }

    fn block_by_id(&self, id: &str) -> Option<BlockRow> {
        self.rows.iter().find(|r| r.id == id).cloned()
    }

    fn parent_of(&self, id: &str) -> Option<String> {
        self.rows
            .iter()
            .find(|r| r.id == id)
            .and_then(|r| r.parent.clone())
    }

    fn open_comment_count(&self, id: &str) -> usize {
        self.comments
            .iter()
            .filter(|c| c.block_id == id && c.parent_id.is_none() && !c.resolved)
            .count()
    }

    fn plain(&self, id: &str, field: &str) -> String {
        self.doc.text(id, field, &self.locale).plain
    }

    /// The caret of a block: the focus when it is on that block, otherwise the start.
    fn caret_in(&self, id: &str) -> (usize, usize) {
        match &self.focus {
            Some(f) if f.id == id => (f.anchor, f.head),
            _ => (0, 0),
        }
    }

    /// The focused block (or the first selected block) with its caret.
    fn focus_with_caret(&self) -> Option<(BlockRow, usize, usize)> {
        if let Some(focus) = &self.focus
            && let Some(block) = self.block_by_id(&focus.id)
        {
            return Some((block, focus.anchor, focus.head));
        }
        let block = self.focus_block()?;
        Some((block, 0, 0))
    }

    /// The focused block, or the first selected block.
    fn focus_block(&self) -> Option<BlockRow> {
        if let Some(focus) = &self.focus
            && let Some(block) = self.block_by_id(&focus.id)
        {
            return Some(block);
        }
        self.selection_in_order()
            .first()
            .and_then(|id| self.block_by_id(id))
    }

    fn selection_in_order(&self) -> Vec<String> {
        self.rows
            .iter()
            .filter(|r| self.selection.contains(&r.id))
            .map(|r| r.id.clone())
            .collect()
    }

    /// Whether the caret is on the first and on the last visual line of the block's text, from the layout (soft-wrapped
    /// lines count). A block without text has one line.
    fn edges(&self, block: &BlockRow, text: &str, head: usize) -> (bool, bool) {
        if block.inline_field.is_none() {
            return (true, true);
        }
        line_edges(
            text,
            font_size(&block.block_type, block.heading_level),
            self.layout_width,
            clamp(text, head),
        )
    }
}

/// One comment as a panel row.
fn comment_row(c: &CommentView, depth: usize) -> CommentRow {
    CommentRow {
        id: c.id.clone(),
        block_id: c.block_id.clone(),
        depth,
        author: c.author.clone(),
        body: c.body.clone(),
        created: c.created_at.clone(),
        resolved: c.resolved,
    }
}

/// The name of a locale for the switcher.
fn locale_label(code: &str) -> String {
    OFFERED_LOCALES
        .iter()
        .find(|(c, _)| *c == code)
        .map_or_else(|| code.to_string(), |(_, label)| label.to_string())
}

/// The inspector's display text of one prop value.
fn display_value(value: Option<&Value>, kind: FieldKind) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Array(items)) if kind == FieldKind::Tags => items
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect::<Vec<_>>()
            .join(", "),
        Some(other) if kind == FieldKind::Json => {
            serde_json::to_string_pretty(other).unwrap_or_default()
        }
        Some(other) => other.to_string(),
    }
}

/// Comma-separated text as a list of non-empty tags.
fn parse_tags(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// The JSON value of an inspector field's text, by the field's kind.
fn value_for(kind: FieldKind, raw: &str) -> Result<Value, String> {
    match kind {
        FieldKind::Boolean => Ok(Value::Bool(raw == "true")),
        FieldKind::Number => raw
            .trim()
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .ok_or_else(|| format!("'{raw}' is not a number")),
        FieldKind::Tags => Ok(Value::Array(
            parse_tags(raw).into_iter().map(Value::String).collect(),
        )),
        FieldKind::Json => serde_json::from_str(raw).map_err(|e| format!("Not valid JSON: {e}")),
        FieldKind::Text
        | FieldKind::TextArea
        | FieldKind::Select
        | FieldKind::Url
        | FieldKind::Media => Ok(Value::String(raw.to_string())),
    }
}
