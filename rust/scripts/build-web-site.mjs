#!/usr/bin/env node
/**
 * Build getvela.app's Rust modules for the web. Same pipeline as
 * build-web-qr.mjs: cargo (wasm32-unknown-unknown, a plain C ABI, no
 * wasm-bindgen) → wasm-opt -Oz → drop the producers section → size gate →
 * emit rust/pkg-web-<module>/.
 *
 *   site-crypto  rust/vela-site-crypto → Keccak-256 + secp256k1 (k256), for the
 *                registry and chain-setup pages (app-web/getvela.app/src/lib/evm).
 *   og           rust/vela-og → the social-preview images, rendered at build
 *                time (app-web/getvela.app/scripts/og/). Never shipped to a browser.
 *
 * The modules are committed. Their bytes are not reproducible across machines
 * (toolchain, wasm-opt), so `--check` compares what can be: the hash of every
 * source that goes into each, and its ABI.
 *
 * Usage:
 *   node rust/scripts/build-web-site.mjs [module…]          build (default: all)
 *   node rust/scripts/build-web-site.mjs --check [module…]  committed output current? (CI;
 *                                                           needs no Rust toolchain)
 */

import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync, mkdirSync, existsSync, readdirSync } from 'node:fs';
import { homedir } from 'node:os';
import { createHash } from 'node:crypto';
import { brotliCompressSync, constants as zlib } from 'node:zlib';
import { dirname, join, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const RUST_DIR = dirname(dirname(fileURLToPath(import.meta.url)));

const MODULES = {
  'site-crypto': {
    crate: 'vela-site-crypto',
    artifact: 'vela_site_crypto',
    out: 'pkg-web-site-crypto',
    // Fetched by the registry and chain-setup pages: kept small.
    maxBrotli: 48_000,
    maxRaw: 160_000,
    exports: ['memory', 'sc_address', 'sc_alloc', 'sc_free', 'sc_keccak256', 'sc_sign'],
    caller: 'app-web/getvela.app/src/lib/evm/wasm.ts',
  },
  og: {
    crate: 'vela-og',
    artifact: 'vela_og',
    out: 'pkg-web-og',
    // Build time only (two embedded Inter subsets, ~127 KB each): the cap is
    // a sanity check, not a wire budget.
    maxBrotli: 260_000,
    maxRaw: 480_000,
    exports: ['memory', 'og_alloc', 'og_check', 'og_font', 'og_fonts_clear', 'og_free', 'og_render', 'og_result_ptr', 'og_subset'],
    caller: 'app-web/getvela.app/scripts/og/render.js',
  },
};

const CHECK_ONLY = process.argv.includes('--check');
const asked = process.argv.slice(2).filter((a) => !a.startsWith('--'));
for (const name of asked) if (!MODULES[name]) fail(`unknown module ${name} (${Object.keys(MODULES).join(', ')})`);
const selected = asked.length ? asked : Object.keys(MODULES);

function fail(message) {
  console.error(`build-web-site: ${message}`);
  process.exit(1);
}

/** Every file that determines a module: its sources and manifests, its lockfile, its fonts, the toolchain. */
function sourceFingerprint(crateDir) {
  const files = [];
  const walk = (dir) => {
    for (const entry of readdirSync(dir, { withFileTypes: true }).sort((a, b) => (a.name < b.name ? -1 : 1))) {
      const path = join(dir, entry.name);
      if (entry.isDirectory()) {
        if (['target', 'tests'].includes(entry.name)) continue;
        walk(path);
      } else if (/\.(rs|ttf)$/.test(entry.name) || entry.name === 'Cargo.toml' || entry.name === 'Cargo.lock') {
        files.push(path);
      }
    }
  };
  walk(crateDir);
  files.push(join(RUST_DIR, 'rust-toolchain.toml'));
  files.sort();
  const hash = createHash('sha256');
  for (const path of files) {
    if (!existsSync(path)) fail(`${relative(RUST_DIR, path)} missing`);
    hash.update(relative(RUST_DIR, path).split(sep).join('/'));
    hash.update('\0');
    // Text normalised for line endings; fonts (embedded) hashed as bytes.
    hash.update(path.endsWith('.ttf') ? readFileSync(path) : readFileSync(path, 'utf8').replace(/\r\n/g, '\n'));
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

function files(name, mod, wasm, fingerprint) {
  return {
    'build-info.json':
      JSON.stringify(
        {
          note: 'Generated by rust/scripts/build-web-site.mjs. `--check` compares the source hash and the wasm ABI, not the wasm bytes (not reproducible across machines).',
          source: fingerprint.digest,
          sourceFiles: fingerprint.files,
          wasmInterface: wasmInterface(wasm),
        },
        null,
        2,
      ) + '\n',
    'README.md':
      `# ${mod.crate} web build (generated)\n\n` +
      `Generated by \`node rust/scripts/build-web-site.mjs ${name}\` from \`rust/${mod.crate}\`.\n` +
      `Committed on purpose, so building the site needs no Rust toolchain; its caller is\n` +
      `\`${mod.caller}\`. Do not hand-edit — CI (\`--check\`) fails when the sources\n` +
      'changed without a rebuild.\n',
  };
}

function check(name, mod) {
  const outDir = join(RUST_DIR, mod.out);
  const wasmPath = join(outDir, `${mod.artifact}.wasm`);
  if (!existsSync(wasmPath)) fail(`rust/${mod.out}/${mod.artifact}.wasm missing — run \`node rust/scripts/build-web-site.mjs ${name}\``);
  const wasm = readFileSync(wasmPath);
  const expected = files(name, mod, wasm, sourceFingerprint(join(RUST_DIR, mod.crate)));
  for (const [file, content] of Object.entries(expected)) {
    const path = join(outDir, file);
    if (!existsSync(path) || readFileSync(path, 'utf8') !== content) {
      fail(`rust/${mod.out}/${file} is stale — the ${mod.crate} sources changed; run \`node rust/scripts/build-web-site.mjs ${name}\``);
    }
  }
  const { exports } = wasmInterface(wasm);
  if (JSON.stringify(exports) !== JSON.stringify(mod.exports)) {
    fail(`${mod.artifact}.wasm exports ${exports.join(', ')}; ${mod.caller} calls ${mod.exports.join(', ')}`);
  }
  console.log(`build-web-site: rust/${mod.out} is current (${wasm.length} bytes, ${brotli(wasm)} brotli'd)`);
}

function build(name, mod) {
  const crateDir = join(RUST_DIR, mod.crate);
  const outDir = join(RUST_DIR, mod.out);
  const built = join(crateDir, 'target', 'wasm32-unknown-unknown', 'release', `${mod.artifact}.wasm`);
  const rustflags = [process.env.RUSTFLAGS, ...remapFlags()].filter(Boolean).join(' ');
  execFileSync('cargo', ['build', '--release', '--locked', '--lib', '--target', 'wasm32-unknown-unknown'], {
    cwd: crateDir,
    stdio: 'inherit',
    env: { ...process.env, RUSTFLAGS: rustflags },
  });
  const optimized = join(crateDir, 'target', `${mod.artifact}.opt.wasm`);
  execFileSync(
    findWasmOpt(),
    ['-Oz', '--enable-bulk-memory', '--enable-sign-ext', '--enable-nontrapping-float-to-int', '--enable-mutable-globals', built, '-o', optimized],
    { stdio: 'inherit' },
  );
  const wasm = withoutProducers(readFileSync(optimized));
  const wire = brotli(wasm);
  if (wasm.length > mod.maxRaw) fail(`${name}: wasm is ${wasm.length} bytes raw, over the ${mod.maxRaw} sanity cap`);
  if (wire > mod.maxBrotli) {
    fail(`${name}: wasm is ${wire} bytes brotli'd, over the ${mod.maxBrotli} budget — find what grew (twiggy) before raising it`);
  }
  const { exports, imports } = wasmInterface(wasm);
  if (JSON.stringify(exports) !== JSON.stringify(mod.exports)) fail(`${name}: unexpected exports: ${exports.join(', ')}`);
  if (imports.length) fail(`${name}: the module imports ${imports.join(', ')} — it must be self-contained`);
  const machinePath = [homedir(), dirname(RUST_DIR)].find((p) => wasm.includes(Buffer.from(p)));
  if (machinePath) fail(`${name}: the module embeds a machine path (${machinePath}) — check the remap flags`);
  mkdirSync(outDir, { recursive: true });
  writeFileSync(join(outDir, `${mod.artifact}.wasm`), wasm);
  for (const [file, content] of Object.entries(files(name, mod, wasm, sourceFingerprint(crateDir)))) {
    writeFileSync(join(outDir, file), content);
  }
  console.log(`build-web-site: wrote rust/${mod.out} (${wasm.length} bytes, ${wire} brotli'd)`);
}

for (const name of selected) (CHECK_ONLY ? check : build)(name, MODULES[name]);
