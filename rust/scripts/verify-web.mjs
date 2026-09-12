#!/usr/bin/env node
/**
 * Replay the conformance corpus through the SHIPPED web artifact.
 *
 * `cargo test` proves the Rust crate matches the corpus; this proves the thing
 * that actually reaches a browser does too — the base64-embedded module, the
 * patched glue, the wasm-bindgen boundary and its JS type conversions. A green
 * `cargo test` with a red run here would mean the build pipeline, not the core,
 * broke (spec SC-001 covers web as its own surface).
 *
 * Usage: node rust/scripts/verify-web.mjs
 */

import { readFileSync, readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { loadShippedCore } from './load-wasm-node.mjs';

const RUST_DIR = dirname(dirname(fileURLToPath(import.meta.url)));
const VECTORS_DIR = join(RUST_DIR, 'crates', 'vela-core', 'tests', 'vectors');

const wasm = await loadShippedCore();

const hex = (bytes) => '0x' + Buffer.from(bytes).toString('hex');
const bytes = (s) => Buffer.from(s.startsWith('0x') ? s.slice(2) : s, 'hex');

/**
 * Identicon params expectations carry section INDICES, but the binding returns the
 * artwork itself. Build the fragment -> index map out of the corpus's own
 * `section-table` group (all 84 artworks, pinned by full text against the package)
 * rather than importing identicons-esm here — that keeps this script checking the
 * SHIPPED artifact against the SAME committed truth `cargo test` uses.
 */
const fragmentIndex = new Map();
{
  const doc = JSON.parse(readFileSync(join(VECTORS_DIR, 'identicon.json'), 'utf8'));
  for (const c of doc.cases) {
    const m = /^section-table\/(\w+)_(\d+)$/.exec(c.name);
    if (m) fragmentIndex.set(`${m[1]}:${c.expect.value}`, Number(m[2]));
  }
  if (fragmentIndex.size !== 84) {
    console.error(`verify-web: expected 84 section-table cases, found ${fragmentIndex.size}`);
    process.exit(1);
  }
}
const indexOf = (section, svg) => fragmentIndex.get(`${section}:${svg}`) ?? null;

/** One arm per contracts/core-api.md function — mirrors conformance.rs. */
// The per-locale assets the web route fetches at runtime. Loading them here means
// the same 18,975 cases run through `Values::Owned` on this surface and through
// `Values::Static` in the Rust suite — the cheapest available proof that the two
// representations agree (spec 004 T048).
const LOCALES = ['en', 'zh', 'zh-TW', 'zh-HK', 'ja', 'ko', 'vi', 'id', 'tr', 'es-MX', 'pt-BR', 'fr', 'de', 'ru', 'it'];
const ASSET_DIR = join(RUST_DIR, '..', 'assets', 'i18n');
const assets = Object.fromEntries(
  LOCALES.map((l) => [l, new Uint8Array(readFileSync(join(ASSET_DIR, `${l}.json`)))]),
);

/** Build an engine with `catalogLng` resident and `changeLng` active. */
/** i18next only canonicalises tags containing `-`, so `zh-tw` becomes `zh-TW`
 *  while a bare `ZH` does not. The asset map is keyed by the canonical form. */
function canonicalTag(t) {
  if (!t || !t.includes('-')) return t;
  const parts = t.split('-');
  return parts
    .map((p, i) => {
      if (i === 0) return p.toLowerCase();
      if (p.length === 4) return p[0].toUpperCase() + p.slice(1).toLowerCase();
      if (p.length === 2) return p.toUpperCase();
      return p.toLowerCase();
    })
    .join('-');
}

function i18nEngine(catalogLng, changeLng) {
  const e = new wasm.I18n(assets.en);
  const c = canonicalTag(catalogLng);
  if (c !== 'en' && assets[c]) e.loadCatalog(c, assets[c]);
  e.changeLanguage(changeLng);
  return e;
}

/** Split a vector's `opts` into the shape the wasm DTO takes. */
function i18nOpts(o) {
  if (!o) return undefined;
  const out = {};
  for (const [k, v] of Object.entries(o)) {
    // The tagged encodings the dumper emits for values JSON cannot hold pass
    // through UNTOUCHED — the Rust DTO decodes them. Converting here would be
    // lossy in exactly the cases the tags exist for: JS `Infinity` cannot survive
    // a JSON round trip and arrives as `null`.
    out[k] = v;
  }
  return out;
}

const DISPATCH = {
  keccak256: (i) => hex(wasm.keccak256(bytes(i.data))),
  sha256: (i) => hex(wasm.sha256(bytes(i.data))),
  to_hex: (i) => wasm.toHex(bytes(i.data), i.prefixed),
  from_hex: (i) => hex(wasm.fromHex(i.s)),
  to_quantity: (i) => wasm.toQuantity(i.value),
  checksum_address: (i) => wasm.checksumAddress(i.address_hex),
  function_selector: (i) => hex(wasm.functionSelector(i.signature)),
  create2_address: (i) => wasm.create2Address(i.deployer_hex, bytes(i.salt), bytes(i.init_code_hash)),
  to_base64url: (i) => wasm.toBase64Url(bytes(i.data)),
  from_base64url: (i) => hex(wasm.fromBase64Url(i.s)),
  abi_encode_address: (i) => hex(wasm.abiEncodeAddress(i.address_hex)),
  abi_encode_uint256: (i) => hex(wasm.abiEncodeUint256(i.value_hex)),
  abi_encode_bytes32: (i) => hex(wasm.abiEncodeBytes32(bytes(i.data))),
  canonicalize_signature: (i) => wasm.canonicalizeSignature(i.sig),
  compute_selector: (i) => wasm.computeSelector(i.sig),
  match_selector: (i) => wasm.matchSelector(i.sig, bytes(i.calldata)),
  decode_calldata: (i) => wasm.decodeCalldata(i.sig, bytes(i.calldata)),
  hash_typed_data: (i) => hex(wasm.hashTypedData(i.typed_data_json)),
  encode_type: (i) => wasm.encodeType(i.typed_data_json),
  parse_public_key: (i) => wasm.parsePublicKey(i.hex),
  compute_safe_address: (i) => wasm.computeSafeAddress(bytes(i.x), bytes(i.y)),
  compute_safe_address_multi: (i) => {
    // The wasm ABI takes flat 64-byte x‖y blocks — this packing IS the
    // surface under test (a 31-byte coordinate is inexpressible here, which
    // is why the corpus carries no multi bad-coordinate case).
    const flat = new Uint8Array(i.keys.length * 64);
    i.keys.forEach((k, idx) => {
      flat.set(bytes(k.x), idx * 64);
      flat.set(bytes(k.y), idx * 64 + 32);
    });
    return wasm.computeSafeAddressMulti(flat);
  },
  compute_webauthn_signer_address: (i) => ({
    address: wasm.computeWebauthnSignerAddress(bytes(i.x), bytes(i.y)),
  }),
  compute_splitter_address: (i) => wasm.computeSplitterAddress(i.treasury_hex),
  encode_splitter_deploy_call: (i) => hex(wasm.encodeSplitterDeployCall(i.treasury_hex)),
  safe_proxy_runtime_code: () => wasm.safeProxyRuntimeCode(),
  extract_attestation_public_key: (i) => wasm.extractAttestationPublicKey(bytes(i.attestation_object)),
  der_signature_to_raw_low_s: (i) => hex(wasm.derSignatureToRawLowS(bytes(i.der))),
  validate_client_data: (i) => {
    wasm.validateClientData(i.kind.toLowerCase(), bytes(i.client_data_json), bytes(i.authenticator_data));
    return true;
  },
  webauthn_signing_hash: (i) => hex(wasm.webauthnSigningHash(bytes(i.authenticator_data), bytes(i.client_data_json))),
  make_hash: (i) => wasm.identiconMakeHash(i.seed),
  identicon_svg: (i) => wasm.identiconSvg(i.seed),
  identicon_svg_circular: (i) => wasm.identiconSvgCircular(i.seed),
  identicon_data_uri: (i) => wasm.identiconDataUri(i.seed),
  // --- i18n ---
  i18n_t: (i) => {
    const o = i18nOpts(i.opts);
    if (o === null) throw Object.assign(new Error('host-only'), { code: 'I18nUnsupportedOption' });
    return i18nEngine(i.lng ?? 'en', i.lng ?? 'en').t(String(i.key), o);
  },
  i18n_plural_suffix: (i) => wasm.i18nPluralSuffix(i.lng, i.count),
  i18n_plural_suffixes: (i) => wasm.i18nPluralSuffixes(i.lng),
  i18n_plural_suffix_legacy: (i) => wasm.i18nPluralSuffixLegacy(i.count),
  i18n_plural_suffixes_legacy: () => wasm.i18nPluralSuffixesLegacy(),
  i18n_t_keys: (i) => i18nEngine(i.lng ?? 'en', i.lng ?? 'en').tFirst((i.keys ?? []).map(String), i18nOpts(i.opts)),
  i18n_t_lng_option: (i) => {
    // The per-call `lng` path: the ACTIVE language stays `en` while resolution
    // runs against the tag in the options. Two different upstream functions.
    const target = canonicalTag(i.opts?.lng ?? 'en');
    const resident = assets[target] ? target : 'en';
    return i18nEngine(resident, 'en').t(String(i.key), i18nOpts(i.opts));
  },
  i18n_interpolate: (i) => wasm.i18nInterpolate(i.template, i18nOpts(i.opts)),
  i18n_t_legacy_plural: (i) => {
    const lng = i.lng ?? 'en';
    const e = wasm.I18n.newWithLegacyPlurals(assets.en);
    if (lng !== 'en' && assets[lng]) e.loadCatalog(lng, assets[lng]);
    e.changeLanguage(lng);
    return e.t(String(i.key), i18nOpts(i.opts));
  },
  i18n_resolve_language: (i) => {
    const s = i18nEngine('en', 'en').changeLanguage(i.requested);
    return { language: s.language, resolved_language: s.resolvedLanguage, languages: s.languages };
  },
  i18n_change_language: (i) => {
    const s = i18nEngine('en', 'en').changeLanguage(i.requested);
    return { language: s.language, resolved_language: s.resolvedLanguage, languages: s.languages };
  },
  normalize_seed: (i) => wasm.identiconNormalizeSeed(i.seed),
  identicon_params: (i) => {
    const p = wasm.identiconParams(i.seed);
    return {
      main: p.main,
      background: p.background,
      accent: p.accent,
      face: indexOf('face', p.face),
      top: indexOf('top', p.top),
      sides: indexOf('sides', p.sides),
      bottom: indexOf('bottom', p.bottom),
    };
  },
  recover_public_key_from_assertions: (i) => {
    const key = wasm.recoverPublicKeyFromAssertions(
      bytes(i.a.authenticator_data), bytes(i.a.client_data_json), bytes(i.a.signature_der),
      bytes(i.b.authenticator_data), bytes(i.b.client_data_json), bytes(i.b.signature_der),
    );
    return key ? `04${key.x.slice(2)}${key.y.slice(2)}` : null;
  },
};

/** Compare an actual value against the vector's `expect`, matching conformance.rs. */
function check(expect, actual) {
  if ('value' in expect) {
    const want = expect.value;
    const got = typeof actual === 'object' && actual !== null && !Array.isArray(actual)
      ? actual
      : actual;
    if (JSON.stringify(want) !== JSON.stringify(got)) {
      return `expected ${JSON.stringify(want)}, got ${JSON.stringify(got)}`;
    }
    return null;
  }
  // Field-wise object expectation (parse_public_key, compute_safe_address).
  // An expectation with no fields would pass over ANY result.
  const fields = Object.entries(expect).filter(([k]) => k !== 'error');
  if (fields.length === 0) return 'expectation has no fields to check';
  for (const [key, want] of fields) {
    const got = actual?.[key];
    // Deep compare, not `!==`. A reference compare can never pass for an
    // array-valued field, which went unnoticed while every suite here had only
    // scalar fields — `languages` in the i18n suites is the first array, and it
    // failed with "expected zh,en, got zh,en".
    const same =
      want === got ||
      (want !== null && got !== null && typeof want === 'object' && typeof got === 'object'
        ? JSON.stringify(want) === JSON.stringify(got)
        : false);
    if (!same) {
      return `field \`${key}\`: expected ${JSON.stringify(want)}, got ${JSON.stringify(got)}`;
    }
  }
  return null;
}

// The corpus is seven suites, discovered by scanning the directory. Every runner
// asserts that exact set is present: without it, a vector file lost to a bad merge
// or a partial checkout would make all four surfaces report "green" over a corpus
// that had silently shrunk — the precise false confidence this feature exists to
// prevent.
const REQUIRED_SUITES = [
  'abi',
  'eip712',
  // `i18n-*` sorts before `identicon`: '1' is 0x31, 'd' is 0x64.
  'i18n-behaviour',
  'i18n-exhaustive',
  'i18n-plural',
  'i18n-plural-legacy',
  'identicon',
  'identicon-bulk',
  'primitives',
  'safe',
  // `safe` before `safe-multi`: a prefix sorts before its extension.
  'safe-multi',
  'webauthn',
];

// Functions that exist in vela-core but are deliberately NOT on any binding
// surface (contracts/identicon-api.md §"Binding surfaces"): a test-only parity
// device, plus helpers with no caller on any Vela platform. Skipping is reported,
// never silent — an unreported skip is how a corpus quietly stops covering things.
const CORE_ONLY_FNS = new Set([
  'identicon_params_js_compat',
  'section_svg',
  'create_identicon',
  'nimiq_is_valid_address',
  'constants',
]);

let total = 0;
let skipped = 0;
const failures = [];
const seenSuites = [];

for (const file of readdirSync(VECTORS_DIR).sort()) {
  if (!file.endsWith('.json')) continue;
  const suite = JSON.parse(readFileSync(join(VECTORS_DIR, file), 'utf8'));
  seenSuites.push(suite.suite);

  // The exhaustive i18n suite is COLUMNAR: {locales, keys, values}. Without this
  // branch it would register its name, contribute zero cases through
  // `suite.cases ?? []`, and report green over nothing.
  if (suite.values && Array.isArray(suite.keys)) {
    if (suite.locales.length !== 15) {
      failures.push(`${suite.suite}: locale set shrank to ${suite.locales.length}`);
    }
    for (const lng of suite.locales) {
      const column = suite.values[lng];
      if (!column || column.length !== suite.keys.length) {
        failures.push(`${suite.suite}: column ${lng} is not key-aligned`);
        continue;
      }
      const e = i18nEngine(lng, lng);
      for (let k = 0; k < suite.keys.length; k++) {
        total++;
        const got = e.t(suite.keys[k], undefined);
        if (got !== column[k] && failures.length < 20) {
          failures.push(`${suite.suite}::${lng}::${suite.keys[k]} — expected ${JSON.stringify(column[k])}, got ${JSON.stringify(got)}`);
        }
      }
    }
    continue;
  }

  // The bulk identicon suite uses a compact `pairs` schema and its own runner.
  if (Array.isArray(suite.pairs)) {
    for (const [seed, expected] of suite.pairs) {
      total++;
      const got = wasm.identiconMakeHash(seed);
      if (got !== expected) {
        if (failures.length < 10) {
          failures.push(`${suite.suite}::makeHash(${JSON.stringify(seed)}) — expected ${expected}, got ${got}`);
        }
      }
    }
    continue;
  }

  for (const c of suite.cases ?? []) {
    if (CORE_ONLY_FNS.has(c.fn)) {
      skipped++;
      continue;
    }
    total++;
    const run = DISPATCH[c.fn];
    if (!run) {
      failures.push(`${suite.suite}::${c.name} — no dispatch arm for \`${c.fn}\``);
      continue;
    }
    let actual;
    let thrown = null;
    try {
      actual = run(c.input);
    } catch (e) {
      thrown = e;
    }
    const expectedError = c.expect.error;
    if (expectedError) {
      if (!thrown) {
        failures.push(`${suite.suite}::${c.name} — expected error ${expectedError}, got ${JSON.stringify(actual)}`);
      } else if (thrown.code !== expectedError) {
        failures.push(`${suite.suite}::${c.name} — expected error ${expectedError}, got ${thrown.code ?? thrown}`);
      }
      continue;
    }
    if (thrown) {
      failures.push(`${suite.suite}::${c.name} — expected success, threw ${thrown.code ?? thrown}`);
      continue;
    }
    const problem = check(c.expect, actual);
    if (problem) failures.push(`${suite.suite}::${c.name} — ${problem}`);
  }
}

if (seenSuites.sort().join(',') !== REQUIRED_SUITES.join(',')) {
  console.error(
    `verify-web: corpus is not the expected suite set — got [${seenSuites}], want [${REQUIRED_SUITES}]`,
  );
  process.exit(1);
}

// ---------------------------------------------------------------------------
// Boundary regressions the corpus STRUCTURALLY cannot express
//
// The corpus encodes values JSON cannot carry with `{"__t":…}` tags, and the
// Rust-side decoder turns those tags back into real values. That is what makes
// it replayable — and it is exactly why a corpus case cannot reach the code path
// a live JS caller takes. Each check below is a defect that was FOUND on the
// live boundary and could not have been caught by adding a vector.
// ---------------------------------------------------------------------------

const boundary = [];

// spec 005 FR-023 — a rejected option must not leave the engine unusable.
//
// `i18n_t` takes `&self`; `changeLanguage`/`loadCatalog` take `&mut self`.
// wasm-bindgen takes the borrow BEFORE argument conversion, so if decoding the
// options throws out of Rust the guard never drops and every `&mut self` method
// is dead for the lifetime of the object. The user-visible form is the worst
// available: the UI pins to the boot language while `i18n.language` moves.
{
  const e = new wasm.I18n(assets.en);
  e.loadCatalog('ja', assets.ja);
  e.changeLanguage('ja');
  try {
    // `ordinal` is a typed `bool`; `undefined` decodes as a unit value.
    e.t('common.cancel', { ordinal: undefined });
  } catch {
    /* rejecting the option is fine — poisoning the engine is not */
  }
  try {
    e.changeLanguage('ja');
  } catch (err) {
    boundary.push(
      `FR-023: a rejected option poisoned the engine — changeLanguage now throws "${err?.message ?? err}"`,
    );
  }
  try {
    e.loadCatalog('ja', assets.ja);
  } catch (err) {
    boundary.push(
      `FR-023: a rejected option poisoned the engine — loadCatalog now throws "${err?.message ?? err}"`,
    );
  }
}

// spec 005 FR-024 — non-finite interpolation variables must render as i18next
// renders them. 004 fixed this for `count` (untagged `CountValue`, f64 first);
// the flattened `vars` map still went through `serde_json::Value`, which has no
// syntax for NaN/Infinity. Reachable today: src/services/activity.ts:116 passes
// `{ n: Math.round(diff / 60) }` into `"{{n}}m"`.
{
  const e = new wasm.I18n(assets.en);
  for (const [value, want] of [
    [Number.NaN, 'NaN'],
    [Number.POSITIVE_INFINITY, 'Infinity'],
    [Number.NEGATIVE_INFINITY, '-Infinity'],
  ]) {
    let got;
    try {
      got = wasm.i18nInterpolate('[{{v}}]', { v: value });
    } catch (err) {
      got = `THREW ${err?.message ?? err}`;
    }
    if (got !== `[${want}]`) {
      boundary.push(`FR-024: interpolating {{v}} with ${want} gave ${JSON.stringify(got)}, want "[${want}]"`);
    }
  }
}

// multi-passkey flat-bytes ABI — length validation lives in the wasm shell,
// so no corpus vector can reach it: the vector layer passes structured keys
// and this script packs them. Empty and non-multiple-of-64 inputs must
// reject with the InvalidPublicKey error shape, not truncate or accept.
{
  for (const len of [0, 65, 63, 128 + 1]) {
    try {
      wasm.computeSafeAddressMulti(new Uint8Array(len));
      boundary.push(`multi flat-bytes: ${len}-byte input was accepted`);
    } catch (err) {
      if (err?.code !== 'InvalidPublicKey') {
        boundary.push(
          `multi flat-bytes: ${len}-byte input rejected with ${JSON.stringify(err?.code ?? String(err))}, want InvalidPublicKey`,
        );
      }
    }
  }
}

if (boundary.length) {
  failures.push(...boundary);
}

if (failures.length) {
  console.error(`verify-web: ${failures.length} of ${total} cases FAILED:\n${failures.join('\n')}`);
  process.exit(1);
}
console.log(
  `verify-web: ${total} conformance cases green through the shipped web artifact` +
    ` (${skipped} skipped — core-only functions with no binding surface: ${[...CORE_ONLY_FNS].join(', ')})`,
);
console.log(`verify-web: ${2 + 3} boundary regressions green (FR-023 borrow safety, FR-024 non-finite vars)`);
