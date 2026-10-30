#!/usr/bin/env node
/**
 * Prove that adopting the Rust i18n engine changes no rendered string
 * (spec 004-rust-i18n, SC-003).
 *
 * Compares, byte for byte:
 *   - the JS library the app ships today (`i18next@26.3.1`), initialised exactly
 *     as `src/i18n/index.ts` initialises it, against
 *   - the shipped web artifact's `I18n`.
 *
 * This is the large-scale differential behind SC-001. The committed corpus holds
 * 18,975 fixed cases so `cargo test` needs no Node; this script goes **beyond**
 * them, fuzzing option bundles the corpus does not enumerate — which is the point.
 * A fixed corpus proves the engine matches on the inputs someone thought of.
 *
 * And the catalogs as the native apps ship them (pass 0): every file under
 * assets/i18n — the on-demand `multisig/` ones included — declared by each
 * app's bundle (iOS's .xcfilelists, Android's Gradle Sync, the desktop's
 * `include_str!`s), and every on-demand key compared once more through an
 * engine of its catalog alone, as the desktop resolves it.
 *
 * Determinism: seeds come from a fixed xorshift PRNG with a hardcoded seed, so a
 * failure is reproducible and a re-run is not a different experiment.
 *
 * Usage:
 *   node scripts/verify-i18n-parity.mjs          # 17,115 exhaustive + 50,000 fuzzed
 *   node scripts/verify-i18n-parity.mjs 200000   # a bigger fuzz sweep
 */

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';

import { loadShippedCore } from '../rust/scripts/load-wasm-node.mjs';

const REPO_ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
// Resolved from this file's own location: the oracle is installed by the
// tooling package in scripts/ (spec 039), not at the repository root.
const require = createRequire(import.meta.url);

// Full ICU is load-bearing: it is what makes the oracle MODE A. A small-icu Node
// would silently compare the port against the DEGRADED behaviour it exists to fix.
if (!process.versions.icu || Intl.PluralRules.supportedLocalesOf(['ru']).length !== 1) {
  throw new Error('verify-i18n: this Node lacks full ICU — the comparison would be against the wrong oracle');
}

const FUZZ_CASES = Number(process.argv[2] ?? 50_000);

// ---------------------------------------------------------------------------
// The two sides
// ---------------------------------------------------------------------------

const LOCALES = ['en', 'zh', 'zh-TW', 'zh-HK', 'ja', 'ko', 'vi', 'id', 'tr', 'es-MX', 'pt-BR', 'fr', 'de', 'ru', 'it'];
const NAMESPACE_FILES = [
  'home', 'send', 'receive', 'assets', 'addToken', 'tokenDetail', 'history',
  'onboarding', 'connect', 'about', 'clearSigning', 'componentsTx',
  'componentsUi', 'settingsModals', 'contacts', 'explore',
];
const CORPUS = join(REPO_ROOT, 'rust/crates/vela-core/i18n/locales');

const readJson = (p) => JSON.parse(readFileSync(p, 'utf8'));
function mergeLocale(lng) {
  let out = { ...readJson(join(CORPUS, `${lng}.json`)) };
  for (const ns of NAMESPACE_FILES) out = { ...out, ...readJson(join(CORPUS, lng, `${ns}.json`)) };
  return out;
}

const resources = {};
for (const lng of LOCALES) resources[lng] = { translation: mergeLocale(lng) };

const i18next = require('i18next');
const oracle = i18next.createInstance();
oracle.init({
  resources,
  lng: 'en',
  fallbackLng: 'en',
  supportedLngs: LOCALES,
  load: 'currentOnly',
  interpolation: { escapeValue: false },
  returnNull: false,
});

// The Rust side, through the artifact the web app actually ships.
//
// Spec 017 stopped embedding the module as base64 and moved it to `public/`
// under a source-fingerprinted name (the D7 route), so the bytes are loaded the
// way every other Node consumer loads them — `loadShippedCore()` — rather than
// from the retired `vela_core_bg.base64.js`. Same artifact, same guarantee: the
// oracle is compared against exactly what a browser runs.
const wasm = await loadShippedCore();

// Per-locale assets, exactly what a shell loads: the main catalog, with the
// on-demand namespaces (`assets/i18n/<name>/<lng>.json`, gen-i18n's `ON_DEMAND`)
// deep-merged in, as the web app merges them for the screens that draw them —
// so every key the corpus holds is compared, wherever it loads from.
const ON_DEMAND = ['multisig'];
/** The key prefix each on-demand catalog answers (gen-i18n's `ON_DEMAND` paths). */
const ON_DEMAND_PREFIX = { multisig: 'home.multisig.' };
const deepMerge = (a, b) => {
  const out = { ...a };
  for (const [k, v] of Object.entries(b)) {
    out[k] = v && typeof v === 'object' && out[k] && typeof out[k] === 'object' ? deepMerge(out[k], v) : v;
  }
  return out;
};
const assets = Object.fromEntries(
  LOCALES.map((l) => {
    let doc = readJson(join(REPO_ROOT, 'assets/i18n', `${l}.json`));
    for (const name of ON_DEMAND) doc = deepMerge(doc, readJson(join(REPO_ROOT, 'assets/i18n', name, `${l}.json`)));
    return [l, new TextEncoder().encode(JSON.stringify(doc))];
  }),
);

// ---------------------------------------------------------------------------
// Pass 0 — what each native app bundles (the catalogs above, as they ship)
// ---------------------------------------------------------------------------
//
// The natives keep no copy of the catalogs: each takes assets/i18n at build
// time through its own declaration — iOS's two .xcfilelists, Android's Gradle
// Sync, the desktop's `include_str!`s (its main catalogs are compiled in from
// the Rust corpus, `i18n-all`). A declaration that misses a file ships an app
// that echoes keys in one language, which no comparison of the files would
// notice. So: every catalog file is declared by every app that bundles its
// namespace, in all 15 languages — the main catalog by every app, an
// on-demand one (`assets/i18n/<name>/`) by each app that bundles any of it.
// An app that bundles none of an on-demand namespace is said, not failed: its
// screens are not built there yet.

const bundleProblems = [];
const notBundled = [];
const catalogFiles = [
  ...LOCALES.map((l) => `${l}.json`),
  ...ON_DEMAND.flatMap((name) => LOCALES.map((l) => `${name}/${l}.json`)),
];
const namespaceOf = (rel) => (rel.includes('/') ? rel.split('/')[0] : 'main');
const linesOf = (path) =>
  readFileSync(join(REPO_ROOT, path), 'utf8')
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line !== '' && !line.startsWith('#'));

/** `bundled(rel)` per app: does its declaration ship `assets/i18n/<rel>`? */
const nativeBundles = {
  ios: (() => {
    const inputs = linesOf('app-ios/scripts/catalogs-input.xcfilelist');
    const outputs = linesOf('app-ios/scripts/catalogs-output.xcfilelist');
    if (inputs.length !== outputs.length) {
      bundleProblems.push(`ios: ${inputs.length} declared reads vs ${outputs.length} writes`);
    }
    const shipped = new Set();
    inputs.forEach((input, i) => {
      const rel = input.replace('$(SRCROOT)/../../assets/i18n/', '');
      const want = `$(TARGET_BUILD_DIR)/$(UNLOCALIZED_RESOURCES_FOLDER_PATH)/${rel}`;
      if (outputs[i] !== want) bundleProblems.push(`ios: ${rel} is read but written as ${outputs[i]}`);
      else shipped.add(rel);
    });
    return (rel) => shipped.has(rel);
  })(),
  android: (() => {
    const gradle = readFileSync(join(REPO_ROOT, 'app-android/vela-wallet/app/build.gradle.kts'), 'utf8');
    const task = gradle.match(/register<Sync>\("syncVelaI18nAssets"\)\s*\{([\s\S]*?)\n\}/);
    if (!task) {
      bundleProblems.push('android: no syncVelaI18nAssets task in app/build.gradle.kts');
      return () => false;
    }
    if (!/from\(velaRepoRoot\.resolve\("assets\/i18n"\)\)/.test(task[1])) {
      bundleProblems.push('android: syncVelaI18nAssets does not copy from assets/i18n');
    }
    // Ant patterns, as Gradle reads them: `*` within one path segment.
    const globs = [...task[1].matchAll(/include\(([^)]*)\)/g)].flatMap((m) =>
      [...m[1].matchAll(/"([^"]+)"/g)].map((q) => new RegExp(`^${q[1].replace(/[.]/g, '\\.').replace(/\*/g, '[^/]*')}$`)),
    );
    return (rel) => globs.some((glob) => glob.test(rel));
  })(),
  desktop: (() => {
    const loc = readFileSync(join(REPO_ROOT, 'app-desktop/vela-wallet/src/loc.rs'), 'utf8');
    const embedded = new Set(
      [...loc.matchAll(/include_str!\("(?:\.\.\/)+assets\/i18n\/([^"]+)"\)/g)].map((m) => m[1]),
    );
    // The main catalogs are compiled in (`i18n-all`), from the same corpus.
    return (rel) => namespaceOf(rel) === 'main' || embedded.has(rel);
  })(),
};

for (const [app, bundled] of Object.entries(nativeBundles)) {
  for (const namespace of ['main', ...ON_DEMAND]) {
    const files = catalogFiles.filter((rel) => namespaceOf(rel) === namespace);
    const shipped = files.filter(bundled);
    if (shipped.length === 0 && namespace !== 'main') {
      notBundled.push(`${app}: ${namespace}`);
      continue;
    }
    for (const rel of files) {
      if (!bundled(rel)) bundleProblems.push(`${app}: assets/i18n/${rel} is not bundled`);
    }
  }
}

// The desktop resolves an on-demand key from an engine of that catalog alone
// (`loc.rs`, `on_demand_engine`: its language over `en`), not from a merged
// one — so each on-demand leaf is compared through such an engine too.
const onDemandEngines = Object.fromEntries(
  ON_DEMAND.map((name) => {
    const bytes = (l) => new TextEncoder().encode(readFileSync(join(REPO_ROOT, 'assets/i18n', name, `${l}.json`), 'utf8'));
    return [
      name,
      Object.fromEntries(
        LOCALES.map((lng) => {
          const e = new wasm.I18n(bytes('en'));
          if (lng !== 'en') e.loadCatalog(lng, bytes(lng));
          e.changeLanguage(lng);
          return [lng, e];
        }),
      ),
    ];
  }),
);

const engines = Object.fromEntries(
  LOCALES.map((lng) => {
    const e = new wasm.I18n(assets.en);
    if (lng !== 'en') e.loadCatalog(lng, assets[lng]);
    e.changeLanguage(lng);
    return [lng, e];
  }),
);

// ---------------------------------------------------------------------------
// Deterministic PRNG (xorshift32, hardcoded seed — same device as the identicon
// parity script, for the same reason: a failure must be reproducible)
// ---------------------------------------------------------------------------

let state = 0x9e3779b9;
function rand() {
  state ^= state << 13;
  state >>>= 0;
  state ^= state >> 17;
  state ^= state << 5;
  state >>>= 0;
  return state / 0x1_0000_0000;
}
const pick = (arr) => arr[Math.floor(rand() * arr.length) % arr.length];

// ---------------------------------------------------------------------------
// Pass 1 — the full cross-product, no options
// ---------------------------------------------------------------------------

function leafPaths(obj, prefix, out) {
  for (const k of Object.keys(obj)) {
    const v = obj[k];
    const p = prefix ? `${prefix}.${k}` : k;
    if (v && typeof v === 'object' && !Array.isArray(v)) leafPaths(v, p, out);
    else out.push(p);
  }
  return out;
}
const KEYS = [...new Set(LOCALES.flatMap((l) => leafPaths(resources[l].translation, '', [])))].sort();

const divergences = [];
let compared = 0;

let onDemandCompared = 0;
for (const lng of LOCALES) {
  oracle.changeLanguage(lng);
  const rust = engines[lng];
  for (const key of KEYS) {
    compared++;
    const want = oracle.t(key);
    const got = rust.t(key, undefined);
    if (want !== got && divergences.length < 25) {
      divergences.push(`[exhaustive] ${lng}::${key}\n  i18next: ${JSON.stringify(want)}\n  rust:    ${JSON.stringify(got)}`);
    }
    for (const name of ON_DEMAND) {
      if (!key.startsWith(ON_DEMAND_PREFIX[name])) continue;
      onDemandCompared++;
      const alone = onDemandEngines[name][lng].t(key, undefined);
      if (want !== alone && divergences.length < 25) {
        divergences.push(`[on-demand ${name}] ${lng}::${key}\n  i18next: ${JSON.stringify(want)}\n  rust:    ${JSON.stringify(alone)}`);
      }
    }
  }
}
console.log(`verify-i18n: pass 1 — ${compared} exhaustive (locale, key) pairs compared`);
console.log(`verify-i18n: pass 1 — ${onDemandCompared} of them again through the on-demand catalogs alone (the desktop's engine)`);
console.log(
  `verify-i18n: pass 0 — ${catalogFiles.length} catalogs against ${Object.keys(nativeBundles).length} native bundle declarations` +
    (notBundled.length ? ` (not bundled, no screens there yet: ${notBundled.join('; ')})` : ''),
);
if (bundleProblems.length) {
  console.error(`\nverify-i18n: ${bundleProblems.length} NATIVE BUNDLE PROBLEM(S):\n`);
  for (const p of bundleProblems.slice(0, 25)) console.error(`  ${p}`);
  console.error(
    '\nA native app would ship without these catalogs and echo keys in those languages.\n' +
      'iOS: node app-ios/scripts/gen-catalog-filelists.mjs · Android: syncVelaI18nAssets in app/build.gradle.kts · ' +
      'desktop: MULTISIG_CATALOGS in src/loc.rs.',
  );
  process.exit(1);
}

// ---------------------------------------------------------------------------
// Pass 2 — fuzzed option bundles the committed corpus does not enumerate
// ---------------------------------------------------------------------------

// The documented exactness boundary (research.md D1): the port matches ICU for
// `|count| < 1e18`. At or above it ICU switches to a scientific representation
// (i=1, e=21), so `fr` selects `one` for 1e21 where the literal CLDR rule text
// computes `many`. D1 recorded that rather than fixing it — no wallet count is a
// recipient tally of 10^21 — so the sweep probes RIGHT UP TO the boundary and
// stops there. 5e17 is included precisely because it is the largest value the
// port still claims to get right.
const COUNTS = [-1, 0, 1, 1.5, 2, 5, 11, 21, 101, 1e6, 5e17, -0, NaN, Infinity, -Infinity, 0.5, 1.0005, 999_999];
const PLURAL_BASES = ['send.recipientCount', 'send.batchApply', 'send.batchRejected', 'contacts.sends', 'contacts.groupMembers'];
const DEFAULTS = [
  undefined, '', 'FALLBACK', '{{count}} thing', 'A $t(common.cancel) B',
  'unbalanced {{ open', 'V=[{{v}}] W=[{{w}}]', '$t(zz.nope)',
];
const VALUES = ['x', '', 'Ω', '<b>&</b>', '$&', '{{v}}', 0, 1, -0, 1.5, 1e21, true, false, null];
const KEY_POOL = [...PLURAL_BASES, 'common.cancel', 'home', 'zz.missing', '', 'a.b.c', 'Hello, world. How are you?'];

let fuzzCompared = 0;
for (let i = 0; i < FUZZ_CASES; i++) {
  const lng = pick(LOCALES);
  const key = pick(KEY_POOL);
  const opts = {};
  if (rand() < 0.7) opts.count = pick(COUNTS);
  if (rand() < 0.4) {
    const d = pick(DEFAULTS);
    if (d !== undefined) opts.defaultValue = d;
  }
  if (rand() < 0.5) opts.v = pick(VALUES);
  if (rand() < 0.2) opts.w = pick(VALUES);
  if (rand() < 0.1) opts.context = pick(['male', 'female', '']);

  oracle.changeLanguage(lng);
  let want;
  let wantThrew = false;
  try {
    want = oracle.t(key, opts);
  } catch {
    wantThrew = true;
  }

  const rust = engines[lng];
  let got;
  let gotThrew = false;
  try {
    got = rust.t(key, opts);
  } catch {
    gotThrew = true;
  }

  fuzzCompared++;
  // A JS throw counts as a pass ONLY where Rust also refuses — the corpus's
  // divergence register is the enumeration of those, and this sweep must not
  // silently widen it.
  if (wantThrew !== gotThrew) {
    if (divergences.length < 25) {
      divergences.push(
        `[fuzz] ${lng}::${key} ${JSON.stringify(opts)}\n  i18next ${wantThrew ? 'THREW' : 'ok'}, rust ${gotThrew ? 'THREW' : 'ok'}`,
      );
    }
    continue;
  }
  if (!wantThrew && typeof want === 'string' && want !== got && divergences.length < 25) {
    divergences.push(
      `[fuzz] ${lng}::${key} ${JSON.stringify(opts)}\n  i18next: ${JSON.stringify(want)}\n  rust:    ${JSON.stringify(got)}`,
    );
  }
}

console.log(`verify-i18n: pass 2 — ${fuzzCompared} fuzzed option bundles compared (seed 0x9e3779b9)`);

if (divergences.length) {
  console.error(`\nverify-i18n: ${divergences.length} DIVERGENCE(S) — showing up to 25:\n`);
  for (const d of divergences) console.error(`${d}\n`);
  console.error(
    'A divergence here is a release blocker, not a flaky test: it means the Rust engine\n' +
      'renders a different string from the i18next the app ships today.',
  );
  process.exit(1);
}

console.log(`\nverify-i18n: ${compared + fuzzCompared} comparisons, zero divergences from i18next@26.3.1.`);
