#!/usr/bin/env node
/**
 * Build vela-core for the web app.
 *
 * Pipeline: wasm-pack (--target web) → size gate → base64-embed the module and
 * neutralize wasm-bindgen's `import.meta.url` fallback → emit rust/pkg-web/.
 *
 * Why base64 + synchronous initSync instead of shipping a .wasm asset
 * (specs/001-rust-core-bindings/research.md D7). The shape was chosen for the
 * Expo web build's bundler — Metro had no wasm-as-ESM support, choked on
 * `import.meta`, and `wrangler pages deploy` dropped node_modules asset paths.
 * That app is gone (spec 039), but every remaining reader depends on the shape:
 *   - app-web/vela-wallet's build-time engine `initSync`s the glue in Node and
 *     `scripts/sync-wasm.mjs` copies the fingerprinted asset into static/;
 *   - rust/scripts/verify-web.mjs and scripts/onchain/ load the SHIPPED
 *     artifact directly through `load-wasm-node.mjs`;
 *   - synchronous init means the facade needs no async gate before first use.
 *
 * Usage:
 *   node rust/scripts/build-web.mjs           build and write rust/pkg-web/
 *   node rust/scripts/build-web.mjs --check   verify the committed output is
 *                                             current (CI); non-zero on drift
 */

import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync, mkdirSync, existsSync, rmSync, readdirSync } from 'node:fs';
import { homedir } from 'node:os';
import { createHash } from 'node:crypto';
import { dirname, join, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const RUST_DIR = dirname(dirname(fileURLToPath(import.meta.url)));
const CRATE_DIR = join(RUST_DIR, 'crates', 'vela-core-wasm');
const STAGING_DIR = join(RUST_DIR, 'target', 'pkg-web-staging');
const OUT_DIR = join(RUST_DIR, 'pkg-web');
const PUBLIC_DIR = join(dirname(RUST_DIR), 'assets', 'wasm');

/**
 * Hard ceiling on the wasm-opt'd module.
 *
 * Since spec 017 wave B the module is NOT bundled: it ships as a fingerprinted
 * asset in `assets/wasm/` and loads asynchronously (the 011 research.md D7 route,
 * triggered when the full machine roadmap blew past the 2 MB embedded-base64
 * ceiling at 2,930,927 bytes). The budget is therefore network transfer, not
 * bundle size: at 4 MB raw the brotli'd wire cost is roughly 1.3 MB, fetched
 * once and cached immutably under its source-fingerprint name.
 */
//
// 4.1 MB since spec 071: the Clear Signer (request, verification, the
// `sign_pref` machine, and the loopback connection the page's suite runs)
// added 77 KB raw — 12 KB brotli'd (916,776 → ~929 KB on the wire). Raised
// by a measured step, not removed: growth still has to be argued for.
const MAX_WASM_BYTES = 4_100_000;

const CHECK_ONLY = process.argv.includes('--check');

function buildWasm() {
  if (existsSync(STAGING_DIR)) rmSync(STAGING_DIR, { recursive: true, force: true });
  // Panic-location strings from registry crates otherwise embed the builder's
  // absolute $CARGO_HOME (56 of them). That makes the artifact machine-dependent
  // — so `--check` could never pass on a CI runner — and ships a developer's
  // home directory to wallet.getvela.app. Remap so any machine produces the
  // same bytes. (`profile.trim-paths` is still unstable on 1.97.)
  const cargoHome = process.env.CARGO_HOME ?? join(homedir(), '.cargo');
  const rustflags = [process.env.RUSTFLAGS, `--remap-path-prefix=${cargoHome}=/cargo`]
    .filter(Boolean)
    .join(' ');
  execFileSync(
    'wasm-pack',
    ['build', '--release', '--target', 'web', '--out-dir', STAGING_DIR, '--out-name', 'vela_core', CRATE_DIR],
    { cwd: RUST_DIR, stdio: 'inherit', env: { ...process.env, RUSTFLAGS: rustflags } },
  );
}

/**
 * Replace the `import.meta.url` default-module-path fallback with a throw.
 * With base64 embedding the fallback is dead code — but it must be REMOVED,
 * not just unused (originally because Metro could not parse import.meta;
 * now because a silently-live fallback path is a second loading mode nobody
 * tests).
 *
 * Asserts the expected pattern exists first: if a wasm-bindgen upgrade changes
 * the glue, this fails loudly instead of silently shipping unpatched code.
 */
function patchGlue(js) {
  const pattern = /new URL\('vela_core_bg\.wasm', import\.meta\.url\)/g;
  const hits = js.match(pattern);
  if (!hits || hits.length === 0) {
    throw new Error(
      "build-web: wasm-bindgen glue no longer contains the expected `new URL('vela_core_bg.wasm', import.meta.url)` fallback — " +
        're-check the --target web output shape before shipping (research.md D7).',
    );
  }
  const patched = js.replace(
    pattern,
    "(() => { throw new Error('vela-core: module path lookup is unavailable in this bundle; call initSync with the embedded module'); })()",
  );
  if (/import\.meta/.test(patched)) {
    throw new Error(
      'build-web: `import.meta` still present in the patched glue — a live default-module-path fallback is a second loading mode nobody tests. Inspect the remaining occurrences.',
    );
  }
  return patched;
}

/**
 * A fingerprint of the SOURCE the artifact was built from: every Rust file of
 * the crates this module is COMPILED FROM — `vela-core` and `vela-core-wasm`
 * — plus the workspace manifests and the lockfile.
 *
 * Narrowed from "every crate in the workspace" (spec 075): the workspace also
 * holds the uniffi shell, the dev fixtures and the relay, none of which the
 * wasm links. Hashing those renamed this 4 MB asset — and demanded a rebuild
 * and a commit of it — every time a relay file changed.
 *
 * This is what `--check` compares, instead of the wasm bytes. The wasm is NOT
 * reproducible across machines and demanding that it be was a mistake:
 * wasm-pack installs a per-platform wasm-bindgen CLI (a macOS build reports
 * `0.2.126`, a Linux one `0.2.126 (21ac804a9)`) and shells out to wasm-opt, a
 * native binaryen binary. Measured on identical toolchain pins (rustc 1.97.1,
 * wasm-pack 0.15.0, same Cargo.lock): macOS arm64 produced 415,030 bytes and
 * Linux x86_64 414,970, with different function counts and 92% of bytes
 * differing. No amount of flag-tweaking makes those equal.
 *
 * Hashing the source instead keeps the property that actually matters — the
 * committed artifact was built from the current Rust — and it is trivially
 * reproducible anywhere, because it only hashes text this repo controls.
 */
function sourceFingerprint() {
  const roots = [
    join(RUST_DIR, 'crates', 'vela-core'),
    join(RUST_DIR, 'crates', 'vela-core-wasm'),
  ];
  const files = [];
  const walk = (dir) => {
    for (const entry of readdirSync(dir, { withFileTypes: true }).sort((a, b) => (a.name < b.name ? -1 : 1))) {
      const path = join(dir, entry.name);
      if (entry.isDirectory()) {
        if (entry.name === 'target' || entry.name === 'bindings') continue;
        walk(path);
      } else if (entry.name.endsWith('.rs') || entry.name === 'Cargo.toml') {
        files.push(path);
      }
    }
  };
  for (const root of roots) walk(root);
  for (const name of ['Cargo.toml', 'Cargo.lock']) {
    const path = join(RUST_DIR, name);
    if (existsSync(path)) files.push(path);
  }
  files.sort();

  const hash = createHash('sha256');
  for (const path of files) {
    // Include the path so a rename is a change, and normalise line endings so
    // a Windows checkout does not report a false drift.
    hash.update(relative(RUST_DIR, path).split(sep).join('/'));
    hash.update('\0');
    hash.update(readFileSync(path, 'utf8').replace(/\r\n/g, '\n'));
    hash.update('\0');
  }
  return { digest: hash.digest('hex'), fileCount: files.length };
}

/** Export/import names and kinds — the artifact's ABI, parsed from the wasm. */
function wasmInterface(wasm) {
  const uleb = (buf, i) => {
    let result = 0;
    let shift = 0;
    for (;;) {
      const byte = buf[i++];
      result |= (byte & 0x7f) << shift;
      shift += 7;
      if (!(byte & 0x80)) return [result, i];
    }
  };
  const exports = [];
  const imports = [];
  let off = 8; // skip magic + version
  while (off < wasm.length) {
    const id = wasm[off++];
    let size;
    [size, off] = uleb(wasm, off);
    const body = wasm.subarray(off, off + size);
    if (id === 7 || id === 2) {
      let count;
      let j;
      [count, j] = uleb(body, 0);
      for (let n = 0; n < count && j < body.length; n++) {
        const read = () => {
          let len;
          [len, j] = uleb(body, j);
          const s = body.subarray(j, j + len).toString('utf8');
          j += len;
          return s;
        };
        if (id === 7) {
          const name = read();
          const kind = body[j++];
          [, j] = uleb(body, j);
          exports.push(`${name}:${kind}`);
        } else {
          const mod = read();
          const name = read();
          const kind = body[j++];
          // Skip the type payload; its encoding varies by kind and we only
          // need the names to detect an ABI change.
          if (kind === 0) [, j] = uleb(body, j);
          else if (kind === 1) {
            j += 1;
            const lim = body[j++];
            [, j] = uleb(body, j);
            if (lim === 1) [, j] = uleb(body, j);
          } else if (kind === 2) {
            const lim = body[j++];
            [, j] = uleb(body, j);
            if (lim === 1) [, j] = uleb(body, j);
          } else j += 2;
          imports.push(`${mod}.${name}:${kind}`);
        }
      }
    }
    off += size;
  }
  return { exports: exports.sort(), imports: imports.sort() };
}

function emit() {
  const wasm = readFileSync(join(STAGING_DIR, 'vela_core_bg.wasm'));
  if (wasm.byteLength > MAX_WASM_BYTES) {
    throw new Error(
      `build-web: wasm is ${wasm.byteLength} bytes, over the ${MAX_WASM_BYTES} ceiling. ` +
        'Switch to the async public/-dir loading route (research.md D7 fallback) instead of raising this limit.',
    );
  }
  const glue = patchGlue(readFileSync(join(STAGING_DIR, 'vela_core.js'), 'utf8'));
  const dts = readFileSync(join(STAGING_DIR, 'vela_core.d.ts'), 'utf8');
  const fingerprint = sourceFingerprint();
  // The asset name carries the SOURCE fingerprint, not a byte hash: wasm bytes
  // are not reproducible across machines, but the source hash is — so every
  // machine agrees on the committed NAME while the bytes stay exempt from
  // byte-comparison (same policy the base64 payload had).
  const wasmAssetName = `vela_core_bg.${fingerprint.digest.slice(0, 12)}.wasm`;

  const files = {
    'vela_core.js': glue,
    'vela_core.d.ts': dts,
    // Where the browser fetches the module from. A module (not JSON) so any
    // bundler resolves it with zero configuration; the fingerprinted name
    // doubles as an immutable cache key.
    'vela_core_wasm_url.js':
      '// @generated by rust/scripts/build-web.mjs — do not edit.\n' +
      `export const WASM_URL = '/${wasmAssetName}';\n`,
    'vela_core_wasm_url.d.ts': 'export declare const WASM_URL: string;\n',
    // Makes Node treat the generated .js as ESM so the corpus replay
    // (verify-web.mjs) can load the SHIPPED artifact directly.
    'package.json': JSON.stringify({ type: 'module', private: true }, null, 2) + '\n',
    // What `--check` actually verifies. See sourceFingerprint().
    //
    // Every field here MUST be platform-independent, or this file reintroduces
    // the exact failure it exists to avoid. The wasm's byte size does not
    // qualify (415,774 here vs 414,970 on linux/amd64) and is deliberately
    // absent; the ABI does (verified identical on both).
    'build-info.json':
      JSON.stringify(
        {
          note: 'Generated by rust/scripts/build-web.mjs. `--check` compares the source hash and the wasm ABI, NOT the wasm bytes — those are not reproducible across machines.',
          source: fingerprint.digest,
          sourceFiles: fingerprint.fileCount,
          wasmInterface: wasmInterface(wasm),
        },
        null,
        2,
      ) + '\n',
    'README.md':
      '# vela-core web build (generated)\n\n' +
      'Generated by `node rust/scripts/build-web.mjs` from `rust/crates/vela-core-wasm`.\n' +
      'Committed on purpose. The wasm module itself ships as a fingerprinted asset\n' +
      'in `public/` (async fetch + initSync — the D7 route); this directory holds the\n' +
      'glue, its types, and the asset URL module. Do not hand-edit — CI\n' +
      '(`node rust/scripts/build-web.mjs --check`) fails if these differ from a fresh build.\n',
  };

  if (CHECK_ONLY) {
    // Every pkg-web file is text generated from sources this repo controls and
    // reproduces byte-for-byte. The wasm asset in public/ is the one artifact
    // exempt from byte comparison (not reproducible across machines) — its
    // NAME and ABI are checked below instead.
    const BYTE_COMPARED = files;
    const drift = Object.entries(BYTE_COMPARED).filter(([name, content]) => {
      const path = join(OUT_DIR, name);
      return !existsSync(path) || readFileSync(path, 'utf8') !== content;
    });
    const extra = existsSync(OUT_DIR)
      ? readdirSync(OUT_DIR).filter((n) => !(n in files))
      : [];

    // Tie build-info.json to the wasm actually committed in public/. Without
    // this the two could be committed as a mismatched pair — a current
    // build-info vouching for a stale asset — and every other check here
    // would still pass.
    const committedAsset = join(PUBLIC_DIR, wasmAssetName);
    if (!drift.length) {
      if (!existsSync(committedAsset)) {
        throw new Error(
          `build-web --check: assets/wasm/${wasmAssetName} is missing — the committed asset was ` +
            'built from different Rust source. Run `npm run build:wasm` and commit the result.',
        );
      }
      const committedWasm = readFileSync(committedAsset);
      const want = JSON.stringify(wasmInterface(wasm));
      const got = JSON.stringify(wasmInterface(committedWasm));
      if (want !== got) {
        throw new Error(
          'build-web --check: the committed wasm exposes a different ABI than a fresh build.\n' +
            `  fresh:     ${want.slice(0, 200)}…\n` +
            `  committed: ${got.slice(0, 200)}…\n` +
            'Run `npm run build:wasm` and commit the result.',
        );
      }
    }

    if (drift.length || extra.length) {
      // Say WHERE it differs. "Out of date" alone cannot distinguish the common
      // case (someone changed Rust and forgot to rebuild) from a build that is
      // simply not reproducible on this machine — and those need opposite
      // fixes. Diagnosing the latter otherwise costs a container and an hour.
      const detail = drift.map(([name, fresh]) => {
        const path = join(OUT_DIR, name);
        if (!existsSync(path)) return `  ${name}: missing from rust/pkg-web`;
        const committed = readFileSync(path, 'utf8');
        if (committed.length !== fresh.length) {
          return `  ${name}: ${committed.length} bytes committed vs ${fresh.length} fresh`;
        }
        let at = 0;
        while (at < fresh.length && fresh[at] === committed[at]) at++;
        let differing = 0;
        for (let i = 0; i < fresh.length; i++) if (fresh[i] !== committed[i]) differing++;
        const pct = ((differing / fresh.length) * 100).toFixed(2);
        return (
          `  ${name}: same length (${fresh.length}), ${differing} chars differ (${pct}%), ` +
          `first at ${at}\n` +
          `    committed: …${committed.slice(Math.max(0, at - 12), at + 12)}…\n` +
          `    fresh:     …${fresh.slice(Math.max(0, at - 12), at + 12)}…`
        );
      });
      const stale = drift.some(([n]) => n === 'build-info.json');
      const hint = stale
        ? '\n\nbuild-info.json drifted, so the committed artifact was NOT built from the ' +
          'current Rust source. This is the stale-artifact case the check exists for.'
        : '';
      throw new Error(
        `build-web --check: rust/pkg-web does not match a fresh build.\n` +
          detail.join('\n') +
          (extra.length ? `\n  unexpected files: ${extra.join(', ')}` : '') +
          hint +
          '\n\nRun `npm run build:wasm` and commit the result.',
      );
    }
    console.log(`build-web --check: rust/pkg-web is current (wasm ${wasm.byteLength} bytes)`);
    return;
  }

  mkdirSync(OUT_DIR, { recursive: true });
  // Remove stale files first (e.g. the retired base64 payload): a leftover
  // module still typechecks and could silently keep an old loading path alive.
  for (const name of readdirSync(OUT_DIR).filter((n) => !(n in files))) {
    rmSync(join(OUT_DIR, name));
  }
  for (const [name, content] of Object.entries(files)) {
    writeFileSync(join(OUT_DIR, name), content);
  }
  mkdirSync(PUBLIC_DIR, { recursive: true });
  for (const name of readdirSync(PUBLIC_DIR).filter(
    (n) => /^vela_core_bg\..*\.wasm$/.test(n) && n !== wasmAssetName,
  )) {
    rmSync(join(PUBLIC_DIR, name));
  }
  writeFileSync(join(PUBLIC_DIR, wasmAssetName), wasm);
  console.log(
    `build-web: wrote rust/pkg-web + assets/wasm/${wasmAssetName} (wasm ${wasm.byteLength} bytes)`,
  );
}

buildWasm();
emit();
