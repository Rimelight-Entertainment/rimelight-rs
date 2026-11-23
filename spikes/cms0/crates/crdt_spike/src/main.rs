// CRDT spike: Rust side. Same document model in Yrs (Yjs-compatible) and Loro.
//
// Usage: cargo run --bin crdt_spike -- <yrs|loro> <init|merge> <dir>
//
// Normalized JSON shape for every lib:
//   [{ "id": str, "type": str, "text": [{ "insert": str, "attributes"?: {..} }] }]
//
// Files (per lib, <lib> = yrs | loro):
//   init  writes <dir>/<lib>-rust-1.bin          (full state, base for both sides)
//   merge reads  <dir>/<lib>-rust-1.bin
//                writes <dir>/<lib>-rust-2.bin   (Rust's own concurrent edit, as an update)
//                reads  <dir>/<lib>-js-1.bin     (JS edit, as an update from the same base)
//                writes <dir>/<lib>-rust-final.json
//
// NOTE: API names marked "UNSURE" were written from memory and must be checked at build time.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{json, Map, Value};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        eprintln!("usage: crdt_spike <yrs|loro> <init|merge> <dir>");
        std::process::exit(2);
    }
    let lib = args[1].as_str();
    let step = args[2].as_str();
    let dir = PathBuf::from(&args[3]);
    fs::create_dir_all(&dir).expect("create dir");

    match (lib, step) {
        ("yrs", "init") => yrs_impl::init(&dir),
        ("yrs", "merge") => yrs_impl::merge(&dir),
        ("loro", "init") => loro_impl::init(&dir),
        ("loro", "merge") => loro_impl::merge(&dir),
        _ => {
            eprintln!("unknown lib/step: {lib} {step}");
            std::process::exit(2);
        }
    }
}

fn print_sizes(dir: &Path, names: &[&str]) {
    for n in names {
        let p = dir.join(n);
        match fs::metadata(&p) {
            Ok(m) => println!("size {:>8} bytes  {}", m.len(), p.display()),
            Err(_) => println!("size      (missing)  {}", p.display()),
        }
    }
}

fn write_final(dir: &Path, lib: &str, blocks: &[Value]) {
    let text = serde_json::to_string_pretty(&Value::Array(blocks.to_vec())).unwrap();
    let path = dir.join(format!("{lib}-rust-final.json"));
    fs::write(&path, &text).expect("write final json");
    println!("{text}");
    println!("wrote {}", path.display());
}

// ---------------------------------------------------------------------------
// Yrs
// ---------------------------------------------------------------------------
mod yrs_impl {
    use super::*;
    use yrs::Out;
    use yrs::types::{Attrs, Change};
    use yrs::updates::decoder::Decode;
    use yrs::{
        Any, Array, ArrayRef, Doc, GetString, Map, MapPrelim, MapRef, ReadTxn, StateVector,
        Text, TextPrelim, TextRef, Transact, TransactionMut, Update, WriteTxn,
    };

    const LIB: &str = "yrs";

    // Document model: root array "blocks" of MapRef { id, type, text: TextRef }.
    // Text carries formatting attributes: bold (bool), italic (bool), mention (string = entry id).

    fn attrs(pairs: &[(&str, Any)]) -> Attrs {
        // UNSURE: Attrs = HashMap<Arc<str>, Any> in yrs 0.2x.
        pairs
            .iter()
            .map(|(k, v)| (Arc::<str>::from(*k), v.clone()))
            .collect()
    }

    fn push_block(
        blocks: &ArrayRef,
        txn: &mut TransactionMut,
        id: &str,
        ty: &str,
    ) -> TextRef {
        // UNSURE: push_back returns V::Return (MapRef for MapPrelim).
        let map: MapRef = blocks.push_back(txn, MapPrelim::default());
        map.insert(txn, "id", id);
        map.insert(txn, "type", ty);
        // UNSURE: Map::insert returns V::Return, i.e. TextRef for TextPrelim.
        map.insert(txn, "text", TextPrelim::new(""))
    }

    fn build_initial(doc: &Doc) {
        let blocks = doc.get_or_insert_array("blocks");
        let mut txn = doc.transact_mut();

        // Block 0: paragraph "Hello world" with "world" bold.
        let t0 = push_block(&blocks, &mut txn, "b1", "paragraph");
        t0.insert(&mut txn, 0, "Hello ");
        t0.insert_with_attributes(&mut txn, 6, "world", attrs(&[("bold", Any::Bool(true))]));

        // Block 1: heading "Title".
        let t1 = push_block(&blocks, &mut txn, "b2", "heading");
        t1.insert(&mut txn, 0, "Title");

        // Block 2: paragraph with a mention on "@Ana".
        let t2 = push_block(&blocks, &mut txn, "b3", "paragraph");
        t2.insert_with_attributes(
            &mut txn,
            0,
            "@Ana",
            attrs(&[("mention", Any::String(Arc::from("entry-123")))]),
        );
    }

    fn encode_full(doc: &Doc) -> Vec<u8> {
        doc.transact().encode_state_as_update_v1(&StateVector::default())
    }

    pub fn init(dir: &Path) {
        let doc = Doc::new();
        build_initial(&doc);
        let bytes = encode_full(&doc);
        let path = dir.join(format!("{LIB}-rust-1.bin"));
        fs::write(&path, &bytes).expect("write base");
        println!("wrote {} ({} bytes)", path.display(), bytes.len());
        write_final(dir, LIB, &to_json(&doc));
        print_sizes(dir, &[&format!("{LIB}-rust-1.bin")]);
    }

    pub fn merge(dir: &Path) {
        let base = fs::read(dir.join(format!("{LIB}-rust-1.bin"))).expect("read base");
        let doc = Doc::new();
        {
            let update = Update::decode_v1(&base).expect("decode base");
            doc.transact_mut().apply_update(update).expect("yrs apply_update");
        }
        let blocks = doc.get_or_insert_array("blocks");

        // Remember the state before our concurrent edit so we can export only that edit.
        let before: StateVector = doc.transact().state_vector();

        // Concurrent edit A: " from Rust" (italic) at end of block 0.
        {
            let mut txn = doc.transact_mut();
            if let Some(Out::YMap(m)) = blocks.get(&txn, 0) {
                if let Some(Out::YText(t)) = m.get(&txn, "text") {
                    let len = t.len(&txn);
                    t.insert_with_attributes(
                        &mut txn,
                        len,
                        " from Rust",
                        attrs(&[("italic", Any::Bool(true))]),
                    );
                }
            }
        }
        let rust_edit = doc.transact().encode_state_as_update_v1(&before);
        let p2 = dir.join(format!("{LIB}-rust-2.bin"));
        fs::write(&p2, &rust_edit).expect("write rust edit");
        println!("wrote {} ({} bytes)", p2.display(), rust_edit.len());

        // Import the JS edit (update from the same base).
        let js = fs::read(dir.join(format!("{LIB}-js-1.bin"))).expect("read js update");
        let update = Update::decode_v1(&js).expect("decode js update");
        doc.transact_mut().apply_update(update).expect("yrs apply_update");

        write_final(dir, LIB, &to_json(&doc));
        print_sizes(
            dir,
            &[
                &format!("{LIB}-rust-1.bin"),
                &format!("{LIB}-rust-2.bin"),
                &format!("{LIB}-js-1.bin"),
            ],
        );
    }

    fn any_to_json(a: &Any) -> Value {
        match a {
            Any::Bool(b) => json!(b),
            Any::String(s) => json!(s.to_string()),
            Any::Number(n) => json!(n),
            Any::Null | Any::Undefined => Value::Null,
            other => json!(format!("{other:?}")),
        }
    }

    fn to_json(doc: &Doc) -> Vec<Value> {
        let blocks = doc.get_or_insert_array("blocks");
        let txn = doc.transact();
        let mut out = Vec::new();
        for i in 0..blocks.len(&txn) {
            let Some(Out::YMap(m)) = blocks.get(&txn, i) else { continue };
            let id = match m.get(&txn, "id") {
                Some(Out::Any(Any::String(s))) => s.to_string(),
                _ => String::new(),
            };
            let ty = match m.get(&txn, "type") {
                Some(Out::Any(Any::String(s))) => s.to_string(),
                _ => String::new(),
            };
            let mut runs = Vec::new();
            if let Some(Out::YText(t)) = m.get(&txn, "text") {
                // UNSURE: TextRef::diff(&txn, YChange::identity) -> Vec<Diff<YChange>>
                // with fields `insert: Out` and `attributes: Option<Box<Attrs>>`.
                for d in t.diff(&txn, |_: yrs::types::text::YChange| ()) {
                    let insert = match d.insert {
                        Out::Any(Any::String(s)) => s.to_string(),
                        _ => continue,
                    };
                    let mut run = serde_json::Map::new();
                    run.insert("insert".into(), json!(insert));
                    if let Some(attrs) = d.attributes {
                        if !attrs.is_empty() {
                            let mut a = serde_json::Map::new();
                            for (k, v) in attrs.iter() {
                                a.insert(k.to_string(), any_to_json(v));
                            }
                            run.insert("attributes".into(), Value::Object(a));
                        }
                    }
                    runs.push(Value::Object(run));
                }
            }
            out.push(json!({ "id": id, "type": ty, "text": runs }));
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Loro
// ---------------------------------------------------------------------------
mod loro_impl {
    use super::*;
    use loro::{
        ExpandType, ExportMode, LoroDoc, LoroMap, LoroText, LoroValue, StyleConfig,
        StyleConfigMap, TextDelta, ValueOrContainer,
    };

    const LIB: &str = "loro";

    // Document model: LoroList "blocks" of LoroMap { id, type, text: LoroText }.
    // Marks: bold, italic (expand after), mention (expand none).

    fn configure(doc: &LoroDoc) {
        // UNSURE: StyleConfigMap::new().insert(key, StyleConfig { expand }) and
        // LoroDoc::config_text_style(map). Must be identical on every peer.
        let mut cfg = StyleConfigMap::new();
        cfg.insert("bold".into(), StyleConfig { expand: ExpandType::After });
        cfg.insert("italic".into(), StyleConfig { expand: ExpandType::After });
        cfg.insert("mention".into(), StyleConfig { expand: ExpandType::None });
        doc.config_text_style(cfg);
    }

    fn push_block(doc: &LoroDoc, index: usize, id: &str, ty: &str) -> LoroText {
        let list = doc.get_list("blocks");
        // UNSURE: LoroList::insert_container(index, LoroMap::new()) -> Result<LoroMap, _>.
        let map: LoroMap = list.insert_container(index, LoroMap::new()).expect("insert block");
        map.insert("id", id).expect("set id");
        map.insert("type", ty).expect("set type");
        // UNSURE: LoroMap::insert_container("text", LoroText::new()) -> Result<LoroText, _>.
        map.insert_container("text", LoroText::new()).expect("set text")
    }

    fn build_initial(doc: &LoroDoc) {
        configure(doc);

        let t0 = push_block(doc, 0, "b1", "paragraph");
        t0.insert(0, "Hello world").expect("insert");
        // "world" is bytes/unicode index 6..11.
        t0.mark(6..11, "bold", true).expect("mark bold");

        let t1 = push_block(doc, 1, "b2", "heading");
        t1.insert(0, "Title").expect("insert");

        let t2 = push_block(doc, 2, "b3", "paragraph");
        t2.insert(0, "@Ana").expect("insert");
        t2.mark(0..4, "mention", "entry-123").expect("mark mention");

        doc.commit();
    }

    pub fn init(dir: &Path) {
        let doc = LoroDoc::new();
        build_initial(&doc);
        // UNSURE: ExportMode::Snapshot is a unit variant in loro 1.x.
        let bytes = doc.export(ExportMode::Snapshot).expect("export snapshot");
        let path = dir.join(format!("{LIB}-rust-1.bin"));
        fs::write(&path, &bytes).expect("write base");
        println!("wrote {} ({} bytes)", path.display(), bytes.len());
        write_final(dir, LIB, &to_json(&doc));
        print_sizes(dir, &[&format!("{LIB}-rust-1.bin")]);
    }

    pub fn merge(dir: &Path) {
        let base = fs::read(dir.join(format!("{LIB}-rust-1.bin"))).expect("read base");
        let doc = LoroDoc::new();
        configure(&doc);
        doc.import(&base).expect("import base");

        // Version before our concurrent edit, so we export only that edit.
        // UNSURE: LoroDoc::oplog_vv() returns VersionVector.
        let before = doc.oplog_vv();

        // Concurrent edit A: " from Rust" (italic) at end of block 0.
        let list = doc.get_list("blocks");
        if let Some(ValueOrContainer::Container(loro::Container::Map(m))) = list.get(0) {
            if let Some(ValueOrContainer::Container(loro::Container::Text(t))) = m.get("text") {
                let len = t.len_unicode();
                t.insert(len, " from Rust").expect("insert");
                t.mark(len..len + " from Rust".chars().count(), "italic", true)
                    .expect("mark italic");
            }
        }
        doc.commit();

        // UNSURE: ExportMode::updates(&vv) -> ExportMode::Updates { from } in loro 1.x.
        let rust_edit = doc
            .export(ExportMode::updates(&before))
            .expect("export rust edit");
        let p2 = dir.join(format!("{LIB}-rust-2.bin"));
        fs::write(&p2, &rust_edit).expect("write rust edit");
        println!("wrote {} ({} bytes)", p2.display(), rust_edit.len());

        // Import JS edit.
        let js = fs::read(dir.join(format!("{LIB}-js-1.bin"))).expect("read js update");
        doc.import(&js).expect("import js update");

        write_final(dir, LIB, &to_json(&doc));
        print_sizes(
            dir,
            &[
                &format!("{LIB}-rust-1.bin"),
                &format!("{LIB}-rust-2.bin"),
                &format!("{LIB}-js-1.bin"),
            ],
        );
    }

    fn value_to_json(v: &LoroValue) -> Value {
        match v {
            LoroValue::Bool(b) => json!(b),
            LoroValue::String(s) => json!(s.to_string()),
            LoroValue::I64(n) => json!(n),
            LoroValue::Double(n) => json!(n),
            LoroValue::Null => Value::Null,
            other => json!(format!("{other:?}")),
        }
    }

    fn to_json(doc: &LoroDoc) -> Vec<Value> {
        let list = doc.get_list("blocks");
        let mut out = Vec::new();
        for i in 0..list.len() {
            let Some(ValueOrContainer::Container(loro::Container::Map(m))) = list.get(i) else {
                continue;
            };
            let id = str_of(m.get("id"));
            let ty = str_of(m.get("type"));
            let mut runs = Vec::new();
            if let Some(ValueOrContainer::Container(loro::Container::Text(t))) = m.get("text") {
                // UNSURE: LoroText::to_delta() -> Vec<TextDelta>, where
                // TextDelta::Insert { insert: String, attributes: Option<HashMap<String, LoroValue>> }.
                for d in t.to_delta() {
                    if let TextDelta::Insert { insert, attributes } = d {
                        let mut run = serde_json::Map::new();
                        run.insert("insert".into(), json!(insert));
                        if let Some(attrs) = attributes {
                            if !attrs.is_empty() {
                                let mut a = serde_json::Map::new();
                                for (k, v) in attrs.iter() {
                                    a.insert(k.clone(), value_to_json(v));
                                }
                                run.insert("attributes".into(), Value::Object(a));
                            }
                        }
                        runs.push(Value::Object(run));
                    }
                }
            }
            out.push(json!({ "id": id, "type": ty, "text": runs }));
        }
        out
    }

    fn str_of(v: Option<ValueOrContainer>) -> String {
        match v {
            Some(ValueOrContainer::Value(LoroValue::String(s))) => s.to_string(),
            _ => String::new(),
        }
    }
}
