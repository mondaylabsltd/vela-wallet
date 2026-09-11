/**
 * Load the shipped web artifact from Node.
 *
 * Since spec 017 the module is not embedded in the bundle: it lives in
 * `public/` under a source-fingerprinted name (the D7 route). Browsers fetch
 * it; Node consumers — the conformance replay, the benchmark, the static
 * export pass — read it from disk through here, so all of them exercise the
 * SAME bytes that reach a browser.
 */

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const RUST_DIR = dirname(dirname(fileURLToPath(import.meta.url)));
const REPO_ROOT = dirname(RUST_DIR);

/** The committed wasm bytes, as named by `rust/pkg-web/vela_core_wasm_url.js`. */
export async function readShippedWasm() {
  const { WASM_URL } = await import(join(RUST_DIR, 'pkg-web', 'vela_core_wasm_url.js'));
  return readFileSync(join(REPO_ROOT, 'assets', 'wasm', WASM_URL.replace(/^\//, '')));
}

/** Import the glue and initialize it with the shipped bytes. */
export async function loadShippedCore() {
  const { initSync, ...wasm } = await import(join(RUST_DIR, 'pkg-web', 'vela_core.js'));
  initSync({ module: await readShippedWasm() });
  return wasm;
}
