// Usage: node compare.mjs <dir>
// For each lib, deep-compare <lib>-rust-final.json with <lib>-js-final.json.

import { readFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { isDeepStrictEqual } from 'node:util';

const dir = process.argv[2];
if (!dir) {
  console.error('usage: node compare.mjs <dir>');
  process.exit(2);
}

let failed = false;
for (const lib of ['yrs', 'loro']) {
  const rustPath = join(dir, `${lib}-rust-final.json`);
  const jsPath = join(dir, `${lib}-js-final.json`);
  if (!existsSync(rustPath) || !existsSync(jsPath)) {
    console.log(`[${lib}] SKIP missing ${!existsSync(rustPath) ? rustPath : jsPath}`);
    failed = true;
    continue;
  }
  const rust = JSON.parse(readFileSync(rustPath, 'utf8'));
  const js = JSON.parse(readFileSync(jsPath, 'utf8'));

  if (isDeepStrictEqual(rust, js)) {
    console.log(`[${lib}] PASS (${rust.length} blocks)`);
    continue;
  }
  failed = true;
  console.log(`[${lib}] FAIL`);
  const a = JSON.stringify(rust, null, 2).split('\n');
  const b = JSON.stringify(js, null, 2).split('\n');
  const n = Math.max(a.length, b.length);
  for (let i = 0; i < n; i++) {
    if (a[i] !== b[i]) {
      console.log(`  line ${i + 1}`);
      console.log(`  - rust: ${a[i] ?? '(none)'}`);
      console.log(`  + js:   ${b[i] ?? '(none)'}`);
    }
  }
}
process.exit(failed ? 1 : 0);
