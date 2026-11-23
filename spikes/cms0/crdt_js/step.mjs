// Usage: node step.mjs <yrs|loro> <dir>
//
// 1. Load <dir>/<lib>-rust-1.bin (base state from Rust init).
// 2. First run only (no <lib>-js-1.bin yet): make the JS edit, export only that edit
//    as an update to <lib>-js-1.bin. Later runs: import the existing <lib>-js-1.bin.
// 3. Import <lib>-rust-2.bin (Rust's edit from merge) if present.
// 4. Write the final normalized JSON to <lib>-js-final.json.

import { writeFileSync } from 'node:fs';
import * as Y from 'yjs';
import { LoroDoc, LoroMap, LoroText } from 'loro-crdt';
import { FILES, readBytes, writeJson, normalizeDelta, existsSync } from './lib-common.mjs';

const [lib, dir] = process.argv.slice(2);
if (!['yrs', 'loro'].includes(lib) || !dir) {
  console.error('usage: node step.mjs <yrs|loro> <dir>');
  process.exit(2);
}
const f = FILES(dir, lib);

// ---------------------------------------------------------------------------
function runYjs() {
  const doc = new Y.Doc();
  Y.applyUpdate(doc, readBytes(f.base));
  const blocks = doc.getArray('blocks');

  if (!existsSync(f.jsUpdate)) {
    const before = Y.encodeStateVector(doc);

    // Edit 1: "Hey " at start of block 2 (the mention block).
    const b2 = blocks.get(2);
    b2.get('text').insert(0, 'Hey ');

    // Edit 2: new block "From JS" at the end.
    // Push first so the map is integrated, then attach and fill the Y.Text.
    const m = new Y.Map();
    blocks.push([m]);
    m.set('id', 'b4');
    m.set('type', 'paragraph');
    const t = new Y.Text();
    m.set('text', t);
    t.insert(0, 'From JS');

    writeFileSync(f.jsUpdate, Y.encodeStateAsUpdate(doc, before));
    console.log(`wrote ${f.jsUpdate}`);
  } else {
    Y.applyUpdate(doc, readBytes(f.jsUpdate));
    console.log(`imported existing ${f.jsUpdate}`);
  }

  if (existsSync(f.rustUpdate)) {
    Y.applyUpdate(doc, readBytes(f.rustUpdate));
    console.log(`imported ${f.rustUpdate}`);
  }

  const out = doc.getArray('blocks').toArray().map((m) => ({
    id: m.get('id'),
    type: m.get('type'),
    text: normalizeDelta(m.get('text').toDelta()),
  }));
  writeJson(f.jsFinal, out);
  console.log(JSON.stringify(out, null, 2));
  console.log(`wrote ${f.jsFinal}`);
}

// ---------------------------------------------------------------------------
function runLoro() {
  const doc = new LoroDoc();
  // UNSURE: configTextStyle name and shape. Must match Rust's config exactly.
  doc.configTextStyle({
    bold: { expand: 'after' },
    italic: { expand: 'after' },
    mention: { expand: 'none' },
  });
  doc.import(readBytes(f.base));
  const list = doc.getList('blocks');

  if (!existsSync(f.jsUpdate)) {
    // UNSURE: oplogVersion() returns a VersionVector usable as export `from`.
    const before = doc.oplogVersion();

    // Edit 1: "Hey " at start of block 2.
    const t2 = list.get(2).get('text');
    t2.insert(0, 'Hey ');

    // Edit 2: new block "From JS" at the end.
    // UNSURE: LoroList.insertContainer(index, new LoroMap()) and
    // LoroMap.setContainer('text', new LoroText()) return the child handle.
    const m = list.insertContainer(list.length, new LoroMap());
    m.set('id', 'b4');
    m.set('type', 'paragraph');
    const t = m.setContainer('text', new LoroText());
    t.insert(0, 'From JS');
    doc.commit();

    // UNSURE: export({ mode: 'update', from: vv }) gives only ops after `from`.
    const bytes = doc.export({ mode: 'update', from: before });
    writeFileSync(f.jsUpdate, bytes);
    console.log(`wrote ${f.jsUpdate} (${bytes.length} bytes)`);
  } else {
    doc.import(readBytes(f.jsUpdate));
    console.log(`imported existing ${f.jsUpdate}`);
  }

  if (existsSync(f.rustUpdate)) {
    doc.import(readBytes(f.rustUpdate));
    console.log(`imported ${f.rustUpdate}`);
  }

  const cur = doc.getList('blocks');
  const out = [];
  for (let i = 0; i < cur.length; i++) {
    const m = cur.get(i);
    out.push({
      id: m.get('id'),
      type: m.get('type'),
      text: normalizeDelta(m.get('text').toDelta()),
    });
  }
  writeJson(f.jsFinal, out);
  console.log(JSON.stringify(out, null, 2));
  console.log(`wrote ${f.jsFinal}`);
}

if (lib === 'yrs') runYjs();
else runLoro();
