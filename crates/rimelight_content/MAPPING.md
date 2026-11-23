# Content mapping: block JSON and Loro

The contract between the Rust side (`rimelight_content`, used by the Slint editor) and the TypeScript side (the
content service in the `rimelight` website repo, used by the `DocRoom` Durable Object for import and publish). Both
sides implement exactly this and are tested on the same fixtures (`fixtures/*.json`). Decision 0049 and
`rimelight-space/docs/cms-editor-design.md` explain why.

Libraries: `loro` (Rust) and `loro-crdt` (JS), same major version. All Loro keys below are exact strings.

## 1. Published form (block JSON)

Unchanged from `@rimelight/cms` `core/types`:

```ts
interface Block {
  id: string;            // UUID v4
  type: string;          // registry type, e.g. "ParagraphBlock"
  props: Record<string, any>;
  children?: Block[];
  isTemplated?: boolean;
  templatedProps?: boolean | string[];
  allowedRoles?: string[];
  requiredPermission?: string;
  visibility?: "internal" | "public";   // new (decision 0049 answer 5/7); absent = inherit the page
}
interface PageContent { blocks: Block[]; properties: Record<string, any>; }
interface PageMeta { slug: string; type: string; title: Record<string,string>; description?: Record<string,string>;
                     tags?: Record<string,string>[]; templateId?: string|null; templateVersion?: number; }
```

Rich text (`RichTextValue`) is `InlineNode[]` or a per-locale map `{ [locale]: InlineNode[] }`. Inline nodes:
`text {text, marks?}`, `link {url, children: text[], openInNewTab?}`, `mention {entryId, collection, slug, title?}`
(`color`/`href` are delivery-only and never stored), `emoji {emoji, shortcode?}`, `icon {name, label?}`. Marks:
`bold`, `italic`, `underline`, `strikethrough`, `code`.

## 2. Loro document

Text style config, applied to every document on both sides before any import or edit:

| Style key | Expand |
|---|---|
| `bold`, `italic`, `underline`, `strikethrough` | `after` |
| `code`, `link`, `mention`, `emoji`, `icon` | `none` |

Root containers:

| Root | Type | Content |
|---|---|---|
| `meta` | map | `slug` (string), `type` (string), `templateId` (string or null), `templateVersion` (number), `tags` (JSON value), `title:<locale>` and `description:<locale>` (Loro text, plain), `properties` (child map: key -> JSON value) |
| `blocks` | tree | fractional index enabled (jitter 0). One node per block. Root-level nodes are the page's top-level blocks |

Each tree node's data map (`tree.getMeta(node)` / `get_meta`) holds:

| Key | Value |
|---|---|
| `id` | block UUID string |
| `type` | block type string |
| `isTemplated`, `templatedProps`, `allowedRoles`, `requiredPermission`, `visibility` | plain values, only when present in the JSON |
| `p:<prop>` | a non-text prop, as a plain JSON value (last write wins) |
| `t:<prop>` | a text prop that is NOT localized: Loro text |
| `t:<prop>:<locale>` | a text prop that IS localized (a per-locale map in JSON): one Loro text per locale |
| `rows` | TableBlock only, see below |

**Which props are text.** From the block registry (`registry/blocks.json`): a prop is *rich text* if its field type
is `richtext`, or it is the block's `inlineText` and the block has `richText: true`. A prop is *plain text* if its
field type is `text` or `textarea`, or it is the block's `inlineText` without `richText`, or it is `CodeBlock.code`.
A plain-text prop whose JSON value is not a string or a per-locale map of strings is stored as `p:` instead. Every
other prop, every prop of an unknown block type, and every prop not in the registry is a `p:` plain value (lossless
fallback). `children` is never a prop: it is the tree.

**Localized detection.** A JSON value that is an object (not an array) whose keys are all locale codes
(`/^[a-z]{2}(-[A-Z]{2})?$/`) is localized. Rich text arrays and plain strings are not.

**TableBlock.** `columns` stays a `p:columns` plain value. `rows` is a Loro list of maps; each map has one Loro text
(plain) per column key. Export turns it back into `Record<string,string>[]`.

## 3. Rich text in Loro text

Import (inline nodes -> text with marks), in order:

| Node | Inserted text | Marks set on that range |
|---|---|---|
| `text` | `text` | each of its `marks` with value `true` |
| `link` | the concatenated children texts (each child's own marks applied to its part) | `link` = `{ "url": url, "openInNewTab": bool }` (`openInNewTab` only if true) |
| `mention` | `title`, else `slug` (never empty: fall back to `"@"`) | `mention` = `{ "entryId", "collection", "slug", "title"? }` |
| `emoji` | `emoji` | `emoji` = `shortcode` or `""` |
| `icon` | `"\u{FFFC}"` (one object replacement character) | `icon` = `{ "name", "label"? }` |

Export (Loro text delta -> inline nodes):

1. Read the delta (`toDelta` / `to_delta`): runs of `{insert, attributes}`.
2. A run with `mention` becomes one `mention` node from the mark value; consecutive runs with an equal `mention`
   value become ONE node; the run text is discarded (editor treats mentions as atomic).
3. A run with `icon` becomes one `icon` node per `\u{FFFC}` in it (other characters in it are dropped).
4. A run with `emoji` becomes one `emoji` node `{emoji: <run text>, shortcode}` (`shortcode` omitted if `""`);
   consecutive runs with equal value merge.
5. A run with `link` contributes a `text` child to a `link` node; consecutive runs with an equal `link` value share
   one `link` node.
6. Other runs become `text` nodes with `marks` = the true-valued style keys among bold, italic, underline,
   strikethrough, code, in that order; `marks` is omitted when empty.

## 4. Normal form and the round-trip rule

`export(import(x)) == normalize(x)` for every fixture, on both sides, and the Rust and TypeScript exports of the same
Loro bytes are equal. `normalize`:

- drops `color` and `href` from mentions, and empty `text` nodes;
- merges adjacent `text` nodes (also inside a link) with identical marks;
- sorts `marks` as bold, italic, underline, strikethrough, code; omits empty `marks`;
- omits `children` when empty, and `openInNewTab` when false;
- keeps object key order irrelevant (compare as JSON values, not strings).

## 5. Drafts

Each page has a main document (the published line). An author's draft is `doc.fork()` of main. Publishing imports the
draft's updates into main (`main.import(draft.export({mode: "update", from: main.oplogVersion()}))`), then exports
main to JSON. A draft catches up with `draft.import(main.export({mode: "update", from: draft.oplogVersion()}))`.

## 6. Fixtures

`fixtures/<name>.page.json`: `{ "meta": PageMeta, "content": PageContent }`, already in normal form, together covering
every registry block type, every inline node type and mark, localized and non-localized rich and plain text, nested
sections and lists, a table, template locks and visibility, and an unknown block type. The TypeScript side (the website app, `apps/rimelight.com/src/space/content/`) keeps a
byte-identical copy (`apps/rimelight.com/test/fixtures/content/`).
