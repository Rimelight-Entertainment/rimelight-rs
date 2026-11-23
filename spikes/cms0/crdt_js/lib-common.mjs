// Shared helpers for the JS side. Not a CLI.
import { readFileSync, existsSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

export const FILES = (dir, lib) => ({
  base: join(dir, `${lib}-rust-1.bin`),   // Rust init output (full state)
  jsUpdate: join(dir, `${lib}-js-1.bin`), // JS edit (update from base)
  rustUpdate: join(dir, `${lib}-rust-2.bin`), // Rust edit (update from base), from merge
  jsFinal: join(dir, `${lib}-js-final.json`),
  rustFinal: join(dir, `${lib}-rust-final.json`),
});

export const readBytes = (p) => new Uint8Array(readFileSync(p));
export const writeJson = (p, v) => writeFileSync(p, JSON.stringify(v, null, 2) + '\n');

// Normalize a delta array to the shared shape: [{ insert, attributes? }].
export function normalizeDelta(delta) {
  const runs = [];
  for (const d of delta) {
    if (typeof d.insert !== 'string') continue; // skip non-text embeds
    const run = { insert: d.insert };
    if (d.attributes && Object.keys(d.attributes).length > 0) {
      run.attributes = Object.fromEntries(
        Object.entries(d.attributes).sort(([a], [b]) => a.localeCompare(b)),
      );
    }
    runs.push(run);
  }
  return runs;
}

export { existsSync };
