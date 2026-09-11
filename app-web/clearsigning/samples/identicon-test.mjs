// The avatar must be byte-identical to the wallet's, or it stops being a
// recognition signal. Reference: vela-core's compiled wasm.
//
//   node samples/identicon-test.mjs
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..');
const repo = join(root, '..', '..');

globalThis.window = globalThis;
for (const file of ['lib/identicon-features.js', 'lib/identicon.js']) {
  (0, eval)(readFileSync(join(root, file), 'utf8'));
}
const ours = globalThis.VelaCS.identicon;

const core = await import(join(repo, 'rust/pkg-web/vela_core.js'));
core.initSync({ module: readFileSync(join(repo, 'assets/wasm/vela_core_bg.1b6c8ce4be03.wasm')) });

const seeds = [
  '0x88cca0f8b4e1f0dc0e7c4f9a2b3d5e6f7a8b6894',
  '0xaf5e8917831ef08a64e18b2cde9f8f5d32c7b3e1',
  '0x9a8b7c6d5e4f3a2b1c0d9e8f7a6b5c4d3e2f1a09',
  '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48',
  '0x0000000000000000000000000000000000000000',
  '0xffffffffffffffffffffffffffffffffffffffff',
  'vela',
  '大表哥',
];

let pass = 0;
for (const seed of seeds) {
  const mine = ours.svgCircular(seed);
  let reference;
  try {
    reference = core.identiconSvgCircular(seed);
  } catch (error) {
    console.log(`SKIP  ${seed} — core refused: ${error.message || error}`);
    continue;
  }
  const same = mine === reference;
  if (same) pass += 1;
  console.log(`${same ? 'PASS' : 'FAIL'}  ${seed}`);
  if (!same) {
    console.log('      hash ours:', ours.makeHash(seed), ' core:', core.identiconMakeHash(seed));
    for (let i = 0; i < Math.max(mine.length, reference.length); i++) {
      if (mine[i] !== reference[i]) {
        console.log('      first difference at', i);
        console.log('      ours:', JSON.stringify(mine.slice(Math.max(0, i - 40), i + 40)));
        console.log('      core:', JSON.stringify(reference.slice(Math.max(0, i - 40), i + 40)));
        break;
      }
    }
  }
}

// A thousand random addresses, because the interesting failures are in the
// float formatting and only show up statistically.
let bulk = 0;
let bulkFail = null;
for (let i = 0; i < 1000; i++) {
  const address = '0x' + [...crypto.getRandomValues(new Uint8Array(20))]
    .map((b) => b.toString(16).padStart(2, '0')).join('');
  try {
    if (ours.svgCircular(address) === core.identiconSvgCircular(address)) bulk += 1;
    else if (!bulkFail) bulkFail = address;
  } catch {
    if (!bulkFail) bulkFail = address + ' (threw)';
  }
}
console.log(`${bulk === 1000 ? 'PASS' : 'FAIL'}  1000 random addresses agree — ${bulk}/1000` +
  (bulkFail ? ` (first divergence: ${bulkFail})` : ''));

const total = seeds.length + 1;
const passed = pass + (bulk === 1000 ? 1 : 0);
console.log(`\n${passed}/${total} checks passed`);
process.exitCode = passed === total ? 0 : 1;
