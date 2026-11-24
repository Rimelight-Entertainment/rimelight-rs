# Editor API (CMS.5)

The editing surface of `rimelight_content` that the Slint block editor (`rimelight_editor`) and the apps build on. It sits
on top of the document mapping in `MAPPING.md` and never changes it: every operation edits the Loro document in the
layout MAPPING.md defines, so `export_page` and the TypeScript side keep working unchanged. Design:
`rimelight-space/docs/cms-editor-design.md`.

All text positions are **byte offsets into the plain text** of one field (what Slint's `TextInput` reports). The editor
converts them to Loro's unicode positions internally.

## Module `editor` (file `src/editor.rs`)

```rust
pub struct EditorDoc { /* LoroDoc + UndoManager */ }

impl EditorDoc {
    pub fn from_doc(doc: LoroDoc) -> Self;                 // configure() is applied; undo tracks local changes only
    pub fn from_snapshot(bytes: &[u8]) -> Result<Self, Error>;
    pub fn doc(&self) -> &LoroDoc;

    // Reading
    pub fn rows(&self, locale: &str) -> Vec<BlockRow>;     // the tree flattened in display order (depth-first)
    pub fn row(&self, id: &str, locale: &str) -> Option<BlockRow>;
    pub fn text(&self, id: &str, field: &str, locale: &str) -> TextValue;
    pub fn page(&self) -> Result<serde_json::Value, Error>; // export_page
    pub fn locales(&self) -> Vec<String>;                  // every locale any localized field or the title uses, sorted, "en" first

    // Text (every call commits)
    pub fn replace_text(&mut self, id: &str, field: &str, locale: &str, new_plain: &str) -> Result<(), Error>;
        // diff by common prefix and suffix, apply one delete and one insert; inserted text takes the marks the
        // configured expand rules give it (MAPPING section 2); a mention run is atomic: an edit that touches part of
        // one deletes the whole run
    pub fn toggle_mark(&mut self, id: &str, field: &str, locale: &str, range: Range<usize>, mark: MarkKind) -> Result<(), Error>;
    pub fn set_link(&mut self, id: &str, field: &str, locale: &str, range: Range<usize>, url: Option<&str>) -> Result<(), Error>;
    pub fn insert_mention(&mut self, id: &str, field: &str, locale: &str, at: usize, mention: Mention) -> Result<usize, Error>;  // returns caret after it
    pub fn insert_emoji(&mut self, id: &str, field: &str, locale: &str, at: usize, emoji: &str, shortcode: &str) -> Result<usize, Error>;

    // Blocks (every call commits; registry rules in registry.rs are enforced)
    pub fn insert_block(&mut self, parent: Option<&str>, after: Option<&str>, block_type: &str) -> Result<String, Error>;
        // registry defaultProps and initialChildren (fresh UUID v4 ids); `after` None = first child
    pub fn split_block(&mut self, id: &str, field: &str, locale: &str, at: usize) -> Result<String, Error>;
        // text after `at` moves to a new block after this one; a SectionBlock title splits into a ParagraphBlock as its
        // first child; returns the new id
    pub fn merge_into_previous(&mut self, id: &str, field: &str, locale: &str) -> Result<Option<(String, usize)>, Error>;
        // appends this block's text to the previous text block and deletes this one; returns (target id, caret)
    pub fn delete_blocks(&mut self, ids: &[String]) -> Result<(), Error>;
    pub fn move_block(&mut self, id: &str, parent: Option<&str>, index: usize) -> Result<(), Error>;
    pub fn move_up(&mut self, id: &str) -> Result<(), Error>;
    pub fn move_down(&mut self, id: &str) -> Result<(), Error>;
    pub fn indent(&mut self, id: &str) -> Result<(), Error>;   // becomes the last child of its previous sibling, if allowed
    pub fn outdent(&mut self, id: &str) -> Result<(), Error>;  // becomes the next sibling of its parent
    pub fn turn_into(&mut self, id: &str, block_type: &str) -> Result<(), Error>; // keeps the inline text and children that fit
    pub fn duplicate(&mut self, id: &str) -> Result<String, Error>;
    pub fn set_prop(&mut self, id: &str, key: &str, value: serde_json::Value) -> Result<(), Error>;
    pub fn set_access(&mut self, id: &str, access: BlockAccess) -> Result<(), Error>;
    pub fn set_meta_text(&mut self, key: &str, locale: &str, value: &str) -> Result<(), Error>; // "title" / "description"
    pub fn set_property(&mut self, key: &str, value: serde_json::Value) -> Result<(), Error>;   // meta.properties

    // Tables
    pub fn table_set_cell(&mut self, id: &str, row: usize, column: &str, value: &str) -> Result<(), Error>;
    pub fn table_insert_row(&mut self, id: &str, at: usize) -> Result<(), Error>;
    pub fn table_delete_row(&mut self, id: &str, at: usize) -> Result<(), Error>;

    // History (local changes only)
    pub fn undo(&mut self) -> bool;
    pub fn redo(&mut self) -> bool;
}

pub struct BlockRow {
    pub id: String,
    pub block_type: String,
    pub depth: usize,
    pub parent: Option<String>,
    pub index: usize,                 // position among its siblings
    pub has_children: bool,
    pub inline_field: Option<String>, // the registry's inlineText (CodeBlock: "code")
    pub rich: bool,                   // inline field is rich text
    pub heading_level: Option<u8>,    // SectionBlock: 2 + section depth, capped at 6
    pub props: serde_json::Map<String, serde_json::Value>, // the plain (`p:`) props
    pub access: BlockAccess,
    pub templated: bool,              // isTemplated
    pub locked_props: LockedProps,
    pub web_only: bool,               // registry webOnly
}

pub struct BlockAccess { pub visibility: Option<String>, pub allowed_roles: Vec<String>, pub required_permission: Option<String> }
pub enum LockedProps { None, All, Keys(Vec<String>) }

pub struct TextValue { pub plain: String, pub spans: Vec<Span> }  // spans cover every byte, in order
pub struct Span { pub range: Range<usize>, pub marks: Vec<MarkKind>, pub link: Option<String>, pub mention: Option<Mention>, pub emoji: bool, pub icon: Option<String> }
pub enum MarkKind { Bold, Italic, Underline, Strikethrough, Code }
pub struct Mention { pub entry_id: String, pub collection: String, pub slug: String, pub title: Option<String> }

// Error gains: Locked (isTemplated or a templatedProps key), NotAllowed (registry nesting), NotFound, InvalidRange
```

## Module `sync` (file `src/sync.rs`)

The DocRoom wire protocol (website `apps/rimelight.com/src/space/content/doc-room-logic.ts`) and a transport-free session.

```rust
pub enum Frame { Snapshot(Vec<u8>), Update(Vec<u8>), Presence(Vec<u8>), Event(serde_json::Value) }
pub fn encode(frame: &Frame) -> Vec<u8>;            // first byte 0x00, 0x01, 0x02, 0x03
pub fn decode(bytes: &[u8]) -> Result<Frame, Error>;

pub struct SyncSession { /* last sent version, presence store */ }
impl SyncSession {
    pub fn new(user_id: &str, name: &str, color: &str) -> Self;
    pub fn receive(&mut self, doc: &EditorDoc, frame: Frame) -> Received;  // imports Snapshot/Update into the doc
    pub fn outgoing(&mut self, doc: &EditorDoc) -> Vec<Frame>;            // local changes since the last call, then presence if it changed
    pub fn set_cursor(&mut self, cursor: Option<Cursor>);                  // this user's caret/selection
    pub fn remote_cursors(&self) -> Vec<RemoteCursor>;                     // others, expired after 30 s without an update
}
pub enum Received { DocChanged, Presence, Event(serde_json::Value), Nothing }
pub struct Cursor { pub block: String, pub field: String, pub locale: String, pub anchor: usize, pub head: usize }
pub struct RemoteCursor { pub user_id: String, pub name: String, pub color: String, pub cursor: Option<Cursor> }
```

Presence payload: JSON `{ "user", "name", "color", "cursor", "at" }` (milliseconds) per peer, carried in Loro's ephemeral
store encoding so the room can relay it unchanged; if the ephemeral store API is not available, plain JSON bytes of that
object are used instead (the room relays bytes either way).
