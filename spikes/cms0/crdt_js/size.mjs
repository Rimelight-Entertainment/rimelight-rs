// Usage: node size.mjs
// Bundles each room sketch with esbuild (esm, neutral, minified) and reports
// minified and gzip sizes, plus the loro-crdt .wasm size and its gzip size.

import { build } from 'esbuild';
import { gzipSync } from 'node:zlib';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const kb = (n) => `${(n / 1024).toFixed(1)} KB`;

async function bundle(entry) {
  const res = await build({
    entryPoints: [join(here, entry)],
    bundle: true,
    format: 'esm',
    platform: 'neutral',
    minify: true,
    write: false,
    outdir: join(here, '.size-out'),
    // loro-crdt may import its .wasm; keep it as a separate output so it is measured below.
    loader: { '.wasm': 'file' },
    logLevel: 'silent',
  });
  const js = res.outputFiles.filter((f) => f.path.endsWith('.js') || f.path.endsWith('.mjs'));
  const assets = res.outputFiles.filter((f) => !js.includes(f));
  let total = 0;
  let gz = 0;
  for (const f of js) {
    total += f.contents.length;
    gz += gzipSync(f.contents).length;
  }
  const assetBytes = assets.reduce((s, f) => s + f.contents.length, 0);
  console.log(
    `${entry}: js ${kb(total)} minified, ${kb(gz)} gzip` +
      (assetBytes ? `, extra assets ${kb(assetBytes)} (${assets.map((a) => a.path).join(', ')})` : ''),
  );
}

function findWasm(dir) {
  const hits = [];
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) hits.push(...findWasm(p));
    else if (name.endsWith('.wasm')) hits.push(p);
  }
  return hits;
}

await bundle('room-yjs.mjs');
await bundle('room-loro.mjs');

const wasmFiles = findWasm(join(here, 'node_modules', 'loro-crdt'));
if (wasmFiles.length === 0) {
  console.log('no .wasm found under node_modules/loro-crdt');
}
for (const w of wasmFiles) {
  const bytes = readFileSync(w);
  console.log(`wasm ${w}: ${kb(bytes.length)}, gzip ${kb(gzipSync(bytes).length)}`);
}

console.log('Cloudflare Workers limit: 3 MB compressed (free), 10 MB compressed (paid).');
