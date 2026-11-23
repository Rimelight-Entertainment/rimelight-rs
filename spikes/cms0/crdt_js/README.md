# CRDT spike: JS side

Compares Yrs (Rust) with Yjs (JS), and Loro (Rust) with loro-crdt (JS), on the same block document.
The Rust crate is `../crates/crdt_spike`. Nothing here has been built or run yet.

## Setup

Run from `crdt_js`:

    npm i

Then run the sequence below. Use one data folder, `data`, which is created by the Rust `init` step.

## Run order

Run from `spikes\cms0` (workspace root). The data folder is `..\crdt_js\data`:

    cd C:\Users\idant\RustRoverProjects\rimelight-rs\spikes\cms0
    cargo run --bin crdt_spike -- yrs init  ..\crdt_js\data
    node ..\crdt_js\step.mjs yrs ..\crdt_js\data
    cargo run --bin crdt_spike -- yrs merge ..\crdt_js\data
    node ..\crdt_js\step.mjs yrs ..\crdt_js\data
    node ..\crdt_js\compare.mjs ..\crdt_js\data

    cargo run --bin crdt_spike -- loro init  ..\crdt_js\data
    node ..\crdt_js\step.mjs loro ..\crdt_js\data
    cargo run --bin crdt_spike -- loro merge ..\crdt_js\data
    node ..\crdt_js\step.mjs loro ..\crdt_js\data
    node ..\crdt_js\compare.mjs ..\crdt_js\data

Bundle sizes (after both libs):

    node ..\crdt_js\size.mjs

## Flow per lib

1. Rust `init` writes `<lib>-rust-1.bin` (full state, shared base).
2. JS `step` (first run) applies a JS edit to the base and writes `<lib>-js-1.bin` (update only), and writes `<lib>-js-final.json`.
3. Rust `merge` applies its own edit, writes `<lib>-rust-2.bin` (update only), imports `<lib>-js-1.bin`, and writes `<lib>-rust-final.json`.
4. JS `step` (second run) imports the existing `<lib>-js-1.bin` and `<lib>-rust-2.bin`, and rewrites `<lib>-js-final.json`.
5. `compare` checks both final JSON files for equality.

Both final files use the shape `[{ id, type, text: [{ insert, attributes? }] }]`.

## Known gaps (to fix at build time)

- Step 2 in JS and step 3 in Rust both make edits. The Rust merge also writes `<lib>-rust-2.bin`, which the spec did not list. It is needed so the JS side can import Rust's edit.
- Yrs and Loro mark config (bold/italic/mention expand rules) must match on both sides. Loro configures it in code on both sides. Yrs attributes are per-insert, so no config is needed.
- Yjs `step.mjs` pushes the map before attaching its Y.Text. This matches the Rust side's block layout.
- `size.mjs` needs `loro-crdt` installed in `node_modules`. It searches that folder for `.wasm` files.

## Unsure API names

See the comments with `UNSURE` in `..\crates\crdt_spike\src\main.rs` and in `step.mjs`, and check:

- yrs: `TextRef::diff` signature and `Change` type; `Attrs` type alias; `Any::String` payload type; `push_back` and `Map::insert` return values; `Update::decode_v1`; `apply_update`.
- loro (Rust): `StyleConfigMap` / `StyleConfig` / `ExpandType` and `LoroDoc::config_text_style`; `ExportMode::updates`; `oplog_vv`; `LoroText::to_delta` and `TextDelta::Insert`; `ValueOrContainer` and `Container` variants; `t.len_unicode()`.
- loro-crdt (JS): `configTextStyle`; `oplogVersion`; `export({ mode: 'update', from })`; `insertContainer`; `setContainer`; `toDelta`; `list.length`.
- yjs (JS): `Y.encodeStateVector`, `Y.encodeStateAsUpdate(doc, sv)`, `Y.applyUpdate`, `Y.Text.toDelta`. These are believed correct.
