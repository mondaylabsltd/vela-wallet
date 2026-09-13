// One-off fixture generator — NOT part of the app, and not a build step.
//
// It runs the same encoder the page ships and writes samples/intents.json, so
// the gallery can load its data from outside itself. Run it only when a
// scenario changes:  node samples/make-intents.mjs
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..');
globalThis.window = globalThis;

for (const file of ['lib/keccak.js', 'lib/abi.js', 'lib/encode.js', 'lib/registry.js', 'samples/scenarios-source.js']) {
  // eslint-disable-next-line no-eval
  (0, eval)(readFileSync(join(root, file), 'utf8'));
}

const cases = window.VelaCS.scenarios.map((s) => ({
  code: s.code,
  title: s.title,
  intent: s.intent,
  context: s.ctx,
  options: s.options,
}));

const json = JSON.stringify({ version: 1, cases }, (key, value) =>
  typeof value === 'bigint' ? value.toString() : value, 2);

writeFileSync(join(root, 'samples/intents.json'), json + '\n');
console.log('wrote samples/intents.json —', cases.length, 'cases');
