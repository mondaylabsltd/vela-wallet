#!/usr/bin/env node
/**
 * Build vela-qr (rust/vela-qr) for the web app's scanner.
 *
 * Pipeline: cargo (wasm32-unknown-unknown, rust/vela-qr/wasm — a plain C ABI,
 * no wasm-bindgen) → wasm-opt -Oz → drop the producers section → size gate →
 * emit rust/pkg-web-qr/.
 *
 * The app imports `rust/pkg-web-qr/vela_qr.wasm?url`: Vite fingerprints it and
 * the scanner fetches it only when it opens (spec 028 D45 — the decoder never
 * rides on any other page's load).
 *
 * The module is committed, like the core's. Its bytes are not reproducible
 * across machines (toolchain, wasm-opt), so `--check` compares what can be:
 * the hash of every source that goes into it, and the module's ABI.
 *
 * Usage:
 *   node rust/scripts/build-web-qr.mjs           build and write rust/pkg-web-qr/
 *   node rust/scripts/build-web-qr.mjs --check   committed output current? (CI;
 *                                                needs no Rust toolchain)
 */

import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync, mkdirSync, existsSync, readdirSync } from 'node:fs';
import { homedir } from 'node:os';
import { createHash } from 'node:crypto';
import { brotliCompressSync, constants as zlib } from 'node:zlib';
import { dirname, join, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const RUST_DIR = dirname(dirname(fileURLToPath(import.meta.url)));
const QR_DIR = join(RUST_DIR, 'vela-qr');
const WASM_CRATE = join(QR_DIR, 'wasm');
const OUT_DIR = join(RUST_DIR, 'pkg-web-qr');
const BUILT = join(WASM_CRATE, 'target', 'wasm32-unknown-unknown', 'release', 'vela_qr_wasm.wasm');

/** The wire budget: brotli q11, as every reader downloads it. Measured 28 KB (2026-11-14). */
const MAX_WASM_BROTLI_BYTES = 48_000;
const MAX_WASM_BYTES = 160_000;

/** The ABI app-web/vela-wallet/src/lib/services/qr-wasm.ts calls. */
const EXPECTED_EXPORTS = ['memory', 'qr_alloc', 'qr_free', 'qr_read_luma', 'qr_read_rgba', 'qr_result_ptr'];

const CHECK_ONLY = process.argv.includes('--check');

function fail(message) {
  console.error(`build-web-qr: ${message}`);
  process.exit(1);
}

/** Every file that determines the module: both crates' sources and manifests, and the lockfile. */
function sourceFingerprint() {
  const files = [];
  const walk = (dir) => {
    for (const entry of readdirSync(dir, { withFileTypes: true }).sort((a, b) => (a.name < b.name ? -1 : 1))) {
      const path = join(dir, entry.name);
      if (entry.isDirectory()) {
        if (['target', 'fuzz', 'tests'].includes(entry.name)) continue;
        walk(path);
      } else if (entry.name.endsWith('.rs') || entry.name === 'Cargo.toml') {
        files.push(path);
      }
    }
  };
  walk(QR_DIR);
  files.push(join(WASM_CRATE, 'Cargo.lock'), join(RUST_DIR, 'rust-toolchain.toml'));
  files.sort();
  const hash = createHash('sha256');
  for (const path of files) {
    if (!existsSync(path)) fail(`${relative(RUST_DIR, path)} missing`);
    hash.update(relative(RUST_DIR, path).split(sep).join('/'));
    hash.update('\0');
    hash.update(readFileSync(path, 'utf8').replace(/\r\n/g, '\n'));
    hash.update('\0');
  }
  return { digest: hash.digest('hex'), files: files.length };
}

function uleb(buf, i) {
  let value = 0;
  let shift = 0;
  for (;;) {
    const byte = buf[i++];
    value += (byte & 0x7f) * 2 ** shift;
    shift += 7;
    if (!(byte & 0x80)) return [value, i];
  }
}

/** Module sections as [id, start, bodyStart, end]. */
function sections(wasm) {
  const out = [];
  let off = 8;
  while (off < wasm.length) {
    const start = off;
    const id = wasm[off++];
    const [size, body] = uleb(wasm, off);
    off = body + size;
    out.push([id, start, body, off]);
  }
  return out;
}

function wasmInterface(wasm) {
  const exports = [];
  const imports = [];
  for (const [id, , body, end] of sections(wasm)) {
    if (id !== 7 && id !== 2) continue;
    const b = wasm.subarray(body, end);
    let [count, j] = uleb(b, 0);
    const name = () => {
      let len;
      [len, j] = uleb(b, j);
      const s = b.subarray(j, j + len).toString('utf8');
      j += len;
      return s;
    };
    for (let n = 0; n < count; n++) {
      if (id === 7) {
        exports.push(name());
        j += 1;
        [, j] = uleb(b, j);
      } else {
        imports.push(`${name()}.${name()}`);
        const kind = b[j++];
        if (kind === 0) [, j] = uleb(b, j);
        else if (kind === 1) {
          j += 1;
          const lim = b[j++];
          [, j] = uleb(b, j);
          if (lim & 1) [, j] = uleb(b, j);
        } else if (kind === 2) {
          const lim = b[j++];
          [, j] = uleb(b, j);
          if (lim & 1) [, j] = uleb(b, j);
        } else j += 2;
      }
    }
  }
  return { exports: exports.sort(), imports: imports.sort() };
}

/** The `producers` custom section names the toolchain that built it: not ours to ship. */
function withoutProducers(wasm) {
  const kept = [wasm.subarray(0, 8)];
  for (const [id, start, body, end] of sections(wasm)) {
    if (id === 0) {
      const [len, at] = uleb(wasm, body);
      if (wasm.subarray(at, at + len).toString('utf8') === 'producers') continue;
    }
    kept.push(wasm.subarray(start, end));
  }
  return Buffer.concat(kept);
}

function brotli(bytes) {
  return brotliCompressSync(bytes, {
    params: {
      [zlib.BROTLI_PARAM_QUALITY]: 11,
      [zlib.BROTLI_PARAM_LGWIN]: 24,
      [zlib.BROTLI_PARAM_SIZE_HINT]: bytes.byteLength,
    },
  }).byteLength;
}

function findWasmOpt() {
  if (process.env.WASM_OPT) return process.env.WASM_OPT;
  try {
    return execFileSync('which', ['wasm-opt'], { encoding: 'utf8' }).trim();
  } catch {
    // wasm-pack's own download, which building the core leaves behind.
    for (const cache of [join(homedir(), 'Library/Caches/.wasm-pack'), join(homedir(), '.cache/.wasm-pack')]) {
      if (!existsSync(cache)) continue;
      const dir = readdirSync(cache).find((d) => d.startsWith('wasm-opt-'));
      if (dir) return join(cache, dir, 'bin', 'wasm-opt');
    }
  }
  fail('wasm-opt not found: install binaryen, set WASM_OPT, or build the core once (wasm-pack downloads it)');
}

/** The same path remapping as the core's build: no machine path in the module. */
function remapFlags() {
  const cargoHome = process.env.CARGO_HOME ?? join(homedir(), '.cargo');
  const rustc = (args) => execFileSync('rustc', args, { cwd: RUST_DIR, encoding: 'utf8' }).trim();
  const sysroot = rustc(['--print', 'sysroot']);
  const commit = /commit-hash: ([0-9a-f]+)/.exec(rustc(['-vV']))?.[1];
  if (!commit) fail('`rustc -vV` names no commit-hash to remap std paths to');
  return [
    `--remap-path-prefix=${cargoHome}=/cargo`,
    `--remap-path-prefix=${join(sysroot, 'lib', 'rustlib', 'src', 'rust')}=/rustc/${commit}`,
    `--remap-path-prefix=${dirname(RUST_DIR)}=/vela`,
  ];
}

function files(wasm, fingerprint) {
  return {
    'build-info.json':
      JSON.stringify(
        {
          note: 'Generated by rust/scripts/build-web-qr.mjs. `--check` compares the source hash and the wasm ABI, not the wasm bytes (not reproducible across machines).',
          source: fingerprint.digest,
          sourceFiles: fingerprint.files,
          wasmInterface: wasmInterface(wasm),
        },
        null,
        2,
      ) + '\n',
    'README.md':
      '# vela-qr web build (generated)\n\n' +
      'Generated by `node rust/scripts/build-web-qr.mjs` from `rust/vela-qr/wasm`.\n' +
      'Committed on purpose: the web scanner imports `vela_qr.wasm?url` (fetched only\n' +
      'when a scanner opens). Do not hand-edit — CI (`--check`) fails when the sources\n' +
      'changed without a rebuild.\n',
  };
}

if (CHECK_ONLY) {
  const wasmPath = join(OUT_DIR, 'vela_qr.wasm');
  if (!existsSync(wasmPath)) fail('rust/pkg-web-qr/vela_qr.wasm missing — run `node rust/scripts/build-web-qr.mjs`');
  const wasm = readFileSync(wasmPath);
  const expected = files(wasm, sourceFingerprint());
  for (const [name, content] of Object.entries(expected)) {
    const path = join(OUT_DIR, name);
    if (!existsSync(path) || readFileSync(path, 'utf8') !== content) {
      fail(`rust/pkg-web-qr/${name} is stale — the vela-qr sources changed; run \`node rust/scripts/build-web-qr.mjs\``);
    }
  }
  const { exports } = wasmInterface(wasm);
  if (JSON.stringify(exports) !== JSON.stringify(EXPECTED_EXPORTS)) {
    fail(`vela_qr.wasm exports ${exports.join(', ')}; qr-wasm.ts calls ${EXPECTED_EXPORTS.join(', ')}`);
  }
  console.log(`build-web-qr: rust/pkg-web-qr is current (${wasm.length} bytes, ${brotli(wasm)} brotli'd)`);
  process.exit(0);
}

const rustflags = [process.env.RUSTFLAGS, ...remapFlags()].filter(Boolean).join(' ');
execFileSync('cargo', ['build', '--release', '--locked', '--target', 'wasm32-unknown-unknown'], {
  cwd: WASM_CRATE,
  stdio: 'inherit',
  env: { ...process.env, RUSTFLAGS: rustflags },
});
const optimized = join(WASM_CRATE, 'target', 'vela_qr.opt.wasm');
execFileSync(
  findWasmOpt(),
  ['-Oz', '--enable-bulk-memory', '--enable-sign-ext', '--enable-nontrapping-float-to-int', '--enable-mutable-globals', BUILT, '-o', optimized],
  { stdio: 'inherit' },
);
const wasm = withoutProducers(readFileSync(optimized));
const wire = brotli(wasm);
if (wasm.length > MAX_WASM_BYTES) fail(`wasm is ${wasm.length} bytes raw, over the ${MAX_WASM_BYTES} sanity cap`);
if (wire > MAX_WASM_BROTLI_BYTES) {
  fail(`wasm is ${wire} bytes brotli'd, over the ${MAX_WASM_BROTLI_BYTES} budget — find what grew (twiggy) before raising it`);
}
const { exports, imports } = wasmInterface(wasm);
if (JSON.stringify(exports) !== JSON.stringify(EXPECTED_EXPORTS)) fail(`unexpected exports: ${exports.join(', ')}`);
if (imports.length) fail(`the module imports ${imports.join(', ')} — it must be self-contained`);
const machinePath = [homedir(), dirname(RUST_DIR)].find((p) => wasm.includes(Buffer.from(p)));
if (machinePath) fail(`the module embeds a machine path (${machinePath}) — check the remap flags`);

mkdirSync(OUT_DIR, { recursive: true });
writeFileSync(join(OUT_DIR, 'vela_qr.wasm'), wasm);
for (const [name, content] of Object.entries(files(wasm, sourceFingerprint()))) {
  writeFileSync(join(OUT_DIR, name), content);
}
console.log(`build-web-qr: vela_qr.wasm ${wasm.length} bytes raw, ${wire} brotli'd (budget ${MAX_WASM_BROTLI_BYTES})`);
