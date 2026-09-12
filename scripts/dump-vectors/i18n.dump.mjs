#!/usr/bin/env node
/**
 * Dumps the i18n conformance corpus from the INSTALLED `i18next` (26.3.1),
 * driving it with resources built exactly as `src/i18n/resources.ts` builds them
 * and an init config copied from `src/i18n/index.ts`.
 *
 * Spec 004-rust-i18n, contracts/conformance-vectors.md. Four suites, 18,975 cases:
 *   i18n-exhaustive     17,115  columnar — every key x every locale
 *   i18n-behaviour         210  the option/edge matrix + language normalisation
 *   i18n-plural            825  MODE A (full Intl.PluralRules)
 *   i18n-plural-legacy     825  MODE B (Intl.PluralRules deleted)
 *
 * Convention is inherited from scripts/dump-vectors/identicon.dump.mjs:
 *   - JSON.stringify(doc, null, 1) + '\n'
 *   - NO timestamp, NO git sha, deterministic ordering  =>  any diff is a
 *     behaviour change.
 *   - every expectation is COMPUTED from the oracle, never hand-typed.
 *
 * Standalone .mjs (not a *.dump.test.ts) for a reason specific to this suite:
 * MODE B requires constructing a second i18next instance with `Intl.PluralRules`
 * DELETED. That is a process-global mutation, and jest runs suites concurrently in
 * a shared worker, so doing it there would non-deterministically corrupt any other
 * suite touching Intl.
 *
 * MODE A is the conformance target. MODE B is what every native build silently
 * produces today, because Hermes ships Intl without PluralRules — it is pinned so
 * the delta stays enumerable (75 of 825 plural cases differ), not because it is
 * behaviour worth keeping.
 */

import { mkdirSync, writeFileSync, readFileSync, statSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';

// Full ICU is load-bearing: `Intl.getCanonicalLocales` drives language-code
// normalisation and `Intl.PluralRules` drives every plural suffix. A small-icu
// Node would silently emit a DIFFERENT, wrong corpus.
if (!process.versions.icu || Intl.PluralRules.supportedLocalesOf(['ru']).length !== 1) {
  throw new Error('i18n.dump: this Node lacks full ICU — the corpus would be silently wrong');
}
// Date interpolation stringifies through Date.prototype.toString(), which reads
// the ambient timezone. Pinned here (Node re-reads process.env.TZ) rather than in
// the npm script, so the corpus is machine-independent on Windows too. Asserted,
// not assumed: a Node that ignored the assignment would emit a corpus whose Date
// vectors encode the dumping machine's timezone.
process.env.TZ = 'UTC';
if (new Date(0).toString() !== 'Thu Jan 01 1970 00:00:00 GMT+0000 (Coordinated Universal Time)') {
  throw new Error('i18n.dump: TZ pin did not take effect — Date vectors would be machine-dependent');
}

// Resolved from this file's own location, exactly like identicon.dump.mjs:35, so
// the script runs from any cwd and on any checkout.
const REPO_ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
// Defaults to the committed corpus directory (writer.ts:26). The override exists
// for the ALT-encoding measurement and for scratch runs; it must never be used to
// write into tests/vectors under a different name — see ALT_SUITES below.
const OUT_DIR = process.env.I18N_VECTORS_OUT ?? join(REPO_ROOT, 'rust/crates/vela-core/tests/vectors');
// Resolved from this file's own location: the oracle is installed by the
// tooling package in scripts/ (spec 039), not at the repository root.
const require = createRequire(import.meta.url);
const i18next = require('i18next');

/// Encodings kept only to keep the 2.21x / 5.38x size measurement in research.md
/// D4 honest. They are NOT part of the corpus: an unregistered
/// `i18n-exhaustive-ALT-*` file in tests/vectors fails the REQUIRED_SUITES
/// assertion in all four runners, so they are written only under an explicit
/// I18N_VECTORS_OUT override.
const ALT_SUITES = new Set(['i18n-exhaustive-ALT-triples', 'i18n-exhaustive-ALT-cases']);
const WRITE_ALT = process.env.I18N_VECTORS_OUT !== undefined;

// ---------------------------------------------------------------------------
// Resources — mirrors src/i18n/resources.ts (same locale order, same namespace
// spread order, so a later namespace overwriting an earlier key behaves the same)
// ---------------------------------------------------------------------------

const LOCALES = ['en', 'zh', 'zh-TW', 'zh-HK', 'ja', 'ko', 'vi', 'id', 'tr', 'es-MX', 'pt-BR', 'fr', 'de', 'ru', 'it'];
const NAMESPACE_FILES = [
  'home', 'send', 'receive', 'assets', 'addToken', 'tokenDetail', 'history',
  'onboarding', 'connect', 'about', 'clearSigning', 'componentsTx',
  'componentsUi', 'settingsModals', 'contacts', 'explore',
];
const FALLBACK_LANGUAGE = 'en';
// The corpus lives in the crate (spec 004 FR-010); this dumper is a consumer.
const LOCALES_DIR = join(REPO_ROOT, 'rust/crates/vela-core/i18n/locales');

const readJson = (p) => JSON.parse(readFileSync(p, 'utf8'));

function buildLocale(lng) {
  let out = { ...readJson(join(LOCALES_DIR, `${lng}.json`)) };
  for (const ns of NAMESPACE_FILES) out = { ...out, ...readJson(join(LOCALES_DIR, lng, `${ns}.json`)) };
  return out;
}

const resources = {};
for (const lng of LOCALES) resources[lng] = { translation: buildLocale(lng) };

/** Init options copied verbatim from src/i18n/index.ts. */
const INIT = {
  resources,
  lng: FALLBACK_LANGUAGE,
  fallbackLng: FALLBACK_LANGUAGE,
  supportedLngs: LOCALES,
  load: 'currentOnly',
  interpolation: { escapeValue: false },
  returnNull: false,
};

const i18n = i18next.createInstance();
i18n.init({ ...INIT });

// ---------------------------------------------------------------------------
// Key inventory — union of leaf paths across ALL locales, sorted.
// The union (not `en`'s own set) is what makes fallbackLng observable: 12 of the
// 1141 paths exist only in `ru`, and 10 exist only in `en`/`zh`, so a per-locale
// key set would never exercise the fallback branch at all.
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

const KEY_PATHS = [...new Set(LOCALES.flatMap((l) => leafPaths(resources[l].translation, '', [])))].sort();

const PLURAL_SUFFIXES = ['_zero', '_one', '_two', '_few', '_many', '_other'];
const PLURAL_BASES = [
  ...new Set(
    KEY_PATHS.flatMap((p) => PLURAL_SUFFIXES.filter((s) => p.endsWith(s)).map((s) => p.slice(0, -s.length))),
  ),
].sort();

// ---------------------------------------------------------------------------
// Writer — identical convention to scripts/dump-vectors/writer.ts
// ---------------------------------------------------------------------------

function writeSuite(suite, body) {
  // Refuse to drop a measurement-only encoding into the committed corpus. All four
  // runners assert an exact REQUIRED_SUITES set, so a stray ALT file turns every
  // surface red with a diagnosis ("unexpected suite") that has nothing to do with
  // the port. Cheaper to make it impossible than to explain it later.
  if (ALT_SUITES.has(suite) && !WRITE_ALT) return null;
  mkdirSync(OUT_DIR, { recursive: true });
  const doc = { suite, source: 'scripts/dump-vectors/i18n.dump.mjs (i18next@26.3.1 + src/i18n resources)', ...body };
  const file = join(OUT_DIR, `${suite}.json`);
  writeFileSync(file, JSON.stringify(doc, null, 1) + '\n');
  return file;
}

/** Precedent: writer.ts expectOracleThrow — a non-throwing oracle means the vector is wrong. */
function expectOracleThrow(label, errorCode, call) {
  try {
    call();
  } catch {
    return { error: errorCode };
  }
  throw new Error(`i18n.dump: oracle did NOT throw for ${label} — vector definition is wrong`);
}

// ---------------------------------------------------------------------------
// Tagged encoding for option values JSON cannot express.
// Decoded by the Rust side; documented in contracts/conformance-vectors.md.
// ---------------------------------------------------------------------------

const T = {
  undef: { __t: 'undefined' },
  nan: { __t: 'nan' },
  inf: (sign) => ({ __t: 'infinity', sign }),
  bigint: (v) => ({ __t: 'bigint', v: String(v) }),
  date: (iso) => ({ __t: 'date', iso }),
  fn: (src) => ({ __t: 'fn', src }),
};

function decodeTag(v) {
  if (v === null || typeof v !== 'object' || Array.isArray(v)) return v;
  switch (v.__t) {
    case undefined: {
      const out = {};
      for (const k of Object.keys(v)) out[k] = decodeTag(v[k]);
      return out;
    }
    case 'undefined': return undefined;
    case 'nan': return NaN;
    case 'infinity': return v.sign < 0 ? -Infinity : Infinity;
    case 'bigint': return BigInt(v.v);
    case 'date': return new Date(v.iso);
    // eslint-disable-next-line no-eval
    case 'fn': return eval(`(${v.src})`);
    default: throw new Error(`unknown tag ${v.__t}`);
  }
}

// ---------------------------------------------------------------------------
// SUITE (a): exhaustive — every key path x every locale, no options.
// Three candidate encodings are built so the size trade-off is MEASURED.
// ---------------------------------------------------------------------------

function buildExhaustive() {
  const values = {};
  const nonString = [];
  for (const lng of LOCALES) {
    i18n.changeLanguage(lng);
    const col = new Array(KEY_PATHS.length);
    for (let i = 0; i < KEY_PATHS.length; i++) {
      let v;
      try {
        v = i18n.t(KEY_PATHS[i]);
      } catch (e) {
        nonString.push(`${lng}::${KEY_PATHS[i]} THREW ${e}`);
        v = null;
      }
      if (typeof v !== 'string') nonString.push(`${lng}::${KEY_PATHS[i]} -> ${typeof v}`);
      col[i] = v;
    }
    values[lng] = col;
  }
  i18n.changeLanguage(FALLBACK_LANGUAGE);
  return { values, nonString };
}

// ---------------------------------------------------------------------------
// SUITE (b): synthetic behaviour matrix
// ---------------------------------------------------------------------------

const COUNTS = [-1, 0, 1, 1.5, 2, 5, 21, 101, 1000000, T.nan, '3'];
const COUNT_LABEL = (c) => (c && c.__t === 'nan' ? 'NaN' : JSON.stringify(c));

/** Run t() through the oracle and turn the outcome into an `expect` object. */
function tExpect(lng, key, opts) {
  const decoded = opts === undefined ? undefined : decodeTag(opts);
  i18n.changeLanguage(lng);
  let v;
  try {
    v = decoded === undefined ? i18n.t(key) : i18n.t(key, decoded);
  } catch (e) {
    return { threw: `${e.constructor.name}: ${e.message}` };
  }
  if (typeof v !== 'string') return { non_string: typeof v, value: v };
  return { value: v };
}

function buildBehaviour() {
  const cases = [];
  const anomalies = [];
  /**
   * `extra` may carry:
   *   error_code   — this case is EXPECTED to make the oracle throw; the vector
   *                  becomes {error: code} and the raw TypeError text is recorded
   *                  in `divergence.ts_behavior` (writer.ts expectOracleThrow rule:
   *                  a non-throwing oracle means the vector is wrong, so fail loud).
   *   divergence   — hand-written, precedent: divergences.ts.
   */
  const add = (name, fn, input, extra) => {
    const key = input.keys ?? input.key;
    const got = tExpect(input.lng, key, input.opts);
    const { error_code, rust_error, divergence, ...rest } = extra ?? {};

    if (error_code) {
      if (!got.threw) throw new Error(`i18n.dump: oracle did NOT throw for ${name} — vector definition is wrong`);
      anomalies.push(`THROW ${name}: ${got.threw}`);
      cases.push({
        name, fn, input,
        expect: { error: error_code },
        divergence: divergence ?? { ts_behavior: `throws a raw ${got.threw}`, reason: 'a typed Rust API returns CoreError; an uncaught TypeError inside render blanks the screen' },
        ...rest,
      });
      return;
    }
    if (got.threw) throw new Error(`i18n.dump: unexpected oracle THROW for ${name}: ${got.threw}`);
    if (got.non_string) {
      anomalies.push(`NON-STRING ${name}: ${got.non_string} ${JSON.stringify(got.value)}`);
      if (!divergence) throw new Error(`i18n.dump: ${name} returned a non-string and carries no divergence entry`);
      cases.push({ name, fn, input, expect: { value: String(got.value) }, divergence, ...rest });
      return;
    }
    // `expect` is what RUST produces; `divergence.ts_behavior` is what TS does.
    // That is the writer.ts convention (scripts/dump-vectors/divergences.ts), and
    // it matters here: three interpolation values stringify through host semantics
    // Rust cannot reach — a JS `Date`, a callable, and an object carrying its own
    // `toString`. Writing the JS text into `expect` would commit the port to an
    // output it can never produce. They are rejected instead (spec FR-008 / T035).
    if (rust_error) {
      cases.push({
        name,
        fn,
        input,
        expect: { error: rust_error },
        divergence: divergence ?? {
          ts_behavior: `returns ${JSON.stringify(got.value)}`,
          reason: 'host-only stringification with no Rust analogue',
        },
        ...rest,
      });
      return;
    }
    cases.push({ name, fn, input, expect: { value: got.value }, ...(divergence ? { divergence } : {}), ...rest });
  };

  // --- counts x a real plural base, per the app's own keys
  const base = 'send.recipientCount';
  for (const c of COUNTS) {
    add(`count/${COUNT_LABEL(c)}`, 'i18n_t', { lng: 'en', key: base, opts: { count: c } });
    add(`count/ru/${COUNT_LABEL(c)}`, 'i18n_t', { lng: 'ru', key: base, opts: { count: c } });
  }
  // --- ordinal (a second CLDR rule set, so a cardinal-only port fails loudly)
  for (const c of [1, 2, 3, 4, 11, 21]) {
    add(`ordinal/en/${c}`, 'i18n_t', { lng: 'en', key: base, opts: { count: c, ordinal: true } });
    add(`ordinal/ru/${c}`, 'i18n_t', { lng: 'ru', key: base, opts: { count: c, ordinal: true } });
  }
  // --- degenerate counts
  add('count/null', 'i18n_t', { lng: 'en', key: base, opts: { count: null } });
  add('count/own-undefined', 'i18n_t', { lng: 'en', key: base, opts: { count: T.undef } });
  add('count/negative-zero', 'i18n_t', { lng: 'en', key: base, opts: { count: -0 } });
  add('count/infinity', 'i18n_t', { lng: 'en', key: base, opts: { count: T.inf(1) } });
  add('count/neg-infinity', 'i18n_t', { lng: 'en', key: base, opts: { count: T.inf(-1) } });
  add('count/object', 'i18n_t', { lng: 'en', key: base, opts: { count: { a: 1 } } });
  add('count/bigint', 'i18n_t', { lng: 'en', key: base, opts: { count: T.bigint('5') } }, {
    error_code: 'I18nInvalidCount',
    divergence: {
      ts_behavior: 'Intl.PluralRules.select(5n) throws TypeError "Cannot convert a BigInt value to a number" — a bigint token amount passed as `count` crashes the render',
      reason: 'FR-004: an unconvertible count must be a typed error, never an uncaught TypeError in a component',
    },
  });
  // --- per-call lng
  add('lng/bogus-code', 'i18n_t', { lng: 'en', key: 'common.cancel', opts: { lng: '!!!' } });
  add('lng/null', 'i18n_t', { lng: 'en', key: 'common.cancel', opts: { lng: null } });

  // --- context
  add('context/present-but-unused', 'i18n_t', { lng: 'en', key: 'common.cancel', opts: { context: 'male' } });
  add('context/empty-string', 'i18n_t', { lng: 'en', key: 'common.cancel', opts: { context: '' } });
  add('context/null', 'i18n_t', { lng: 'en', key: 'common.cancel', opts: { context: null } });
  add('context/number', 'i18n_t', { lng: 'en', key: 'common.cancel', opts: { context: 7 } });
  add('context/with-count', 'i18n_t', { lng: 'en', key: base, opts: { context: 'x', count: 2 } });
  add('context/synthetic-hit', 'i18n_t', {
    lng: 'en', key: 'zz.ctx',
    opts: { context: 'male', defaultValue: 'BASE', defaultValue_male: 'MALE' },
  });

  // --- defaultValue shapes
  add('default/plain', 'i18n_t', { lng: 'en', key: 'zz.missing', opts: { defaultValue: 'D' } });
  add('default/null', 'i18n_t', { lng: 'en', key: 'zz.missing', opts: { defaultValue: null } });
  add('default/empty-string', 'i18n_t', { lng: 'en', key: 'zz.missing', opts: { defaultValue: '' } });
  add('default/undefined-own-property', 'i18n_t', { lng: 'en', key: 'zz.missing', opts: { defaultValue: T.undef } });
  add('default/number', 'i18n_t', { lng: 'en', key: 'zz.missing', opts: { defaultValue: 42 } }, {
    divergence: {
      ts_behavior: 'returns the raw NUMBER 42 — t() is not string-typed at runtime',
      reason: 'the Rust signature returns String; the caller receives "42"',
    },
  });
  add('default/bool', 'i18n_t', { lng: 'en', key: 'zz.missing', opts: { defaultValue: true } }, {
    divergence: {
      ts_behavior: 'returns the raw BOOLEAN true',
      reason: 'same as default/number — Rust returns String',
    },
  });
  add('default/object', 'i18n_t', { lng: 'en', key: 'zz.missing', opts: { defaultValue: { a: 1 } } });
  add('default/array', 'i18n_t', { lng: 'en', key: 'zz.missing', opts: { defaultValue: ['a', 'b'] } });
  add('default/other-only', 'i18n_t', { lng: 'en', key: 'zz.missing', opts: { count: 2, defaultValue_other: '{{count}} O' } });
  add('default/other-and-base', 'i18n_t', {
    lng: 'en', key: 'zz.missing',
    opts: { count: 1, defaultValue: 'ONE', defaultValue_other: 'OTHER' },
  });
  add('default/other-ru-many', 'i18n_t', {
    lng: 'ru', key: 'zz.missing',
    opts: { count: 5, defaultValue: 'B', defaultValue_one: 'ONE', defaultValue_few: 'FEW', defaultValue_many: 'MANY', defaultValue_other: 'OTHER' },
  });
  add('default/zero-suffix', 'i18n_t', {
    lng: 'en', key: 'zz.missing',
    opts: { count: 0, defaultValue: 'B', defaultValue_zero: 'ZERO', defaultValue_other: 'OTHER' },
  });
  add('default/interpolated', 'i18n_t', { lng: 'en', key: 'zz.missing', opts: { defaultValue: 'D {{a}}', a: 'A' } });
  add('default/absent-on-missing-key', 'i18n_t', { lng: 'en', key: 'zz.missing' });

  // --- interpolation values
  const TPL = 'V=[{{v}}]';
  const IV = [
    ['absent', {}],
    ['own-undefined', { v: T.undef }],
    ['null', { v: null }],
    ['zero', { v: 0 }],
    ['false', { v: false }],
    ['true', { v: true }],
    ['nan', { v: T.nan }],
    ['infinity', { v: T.inf(1) }],
    ['neg-infinity', { v: T.inf(-1) }],
    ['number', { v: 1234.5 }],
    ['big-number', { v: 1e21 }],
    ['neg-zero', { v: -0 }],
    ['bigint', { v: T.bigint('123456789012345678901234567890') }],
    ['empty-string', { v: '' }],
    ['string-with-braces', { v: '{{v}}' }],
    ['object', { v: { a: 1 } }],
    ['array', { v: [1, 2, 3] }],
    ['nested-array', { v: [[1], [2]] }],
    ['date', { v: T.date('1970-01-01T00:00:00.000Z') }],
    ['function', { v: T.fn("() => 'from-fn'") }],
    ['symbol-free-object-with-toString', { v: { toString: T.fn("() => 'CUSTOM'") } }],
  ];
  /** Values whose JS stringification has no Rust analogue — pinned, never silent. */
  const HOST_ONLY = {
    date: {
      ts_behavior: 'Date.prototype.toString() — "Thu Jan 01 1970 00:00:00 GMT+0000 (Coordinated Universal Time)"; reads the ambient TZ, hence the TZ=UTC guard at the top of this dumper',
      reason: 'the Rust interpolation-value enum has no Date variant; callers must pre-format dates through services/locale-format.ts, which is what the app already does',
    },
    function: {
      ts_behavior: 'Function.prototype.toString() splices the callback SOURCE TEXT into the UI string',
      reason: 'unrepresentable in Rust and a latent source-leak in TS; the port rejects callable values',
    },
    'symbol-free-object-with-toString': {
      ts_behavior: 'honours a user-supplied toString() on the replacement object',
      reason: 'Rust has no dynamic toString dispatch on a JSON value',
    },
  };
  for (const [label, vals] of IV) {
    add(`interp/${label}`, 'i18n_t', { lng: 'en', key: 'zz.interp', opts: { defaultValue: TPL, ...vals } },
      HOST_ONLY[label]
        ? { divergence: HOST_ONLY[label], rust_error: 'I18nUnsupportedOption' }
        : undefined);
  }
  // dotted / nested lookup path
  add('interp/dotted-path', 'i18n_t', { lng: 'en', key: 'zz.i', opts: { defaultValue: 'V={{a.b.c}}', a: { b: { c: 'DEEP' } } } });
  add('interp/dotted-path-missing', 'i18n_t', { lng: 'en', key: 'zz.i', opts: { defaultValue: 'V={{a.b.z}}', a: { b: { c: 'DEEP' } } } });
  add('interp/replace-object', 'i18n_t', { lng: 'en', key: 'zz.i', opts: { defaultValue: TPL, replace: { v: 'R' } } });
  add('interp/replace-shadows-top-level', 'i18n_t', { lng: 'en', key: 'zz.i', opts: { defaultValue: TPL, v: 'TOP', replace: { v: 'R' } } });
  // unescape form
  add('interp/unescape', 'i18n_t', { lng: 'en', key: 'zz.i', opts: { defaultValue: 'U=[{{- v}}]', v: '<b>&amp;</b>' } });
  add('interp/unescape-no-space', 'i18n_t', { lng: 'en', key: 'zz.i', opts: { defaultValue: 'U=[{{-v}}]', v: '<b>&</b>' } });
  add('interp/escaped-default-off', 'i18n_t', { lng: 'en', key: 'zz.i', opts: { defaultValue: TPL, v: '<b>&</b>' } });
  // unbalanced braces
  for (const [label, tpl] of [
    ['open-only', 'a {{v'],
    ['close-only', 'a v}}'],
    ['triple-open', 'a {{{v}} b'],
    ['triple-close', 'a {{v}}} b'],
    ['nested-braces', 'a {{ {{v}} }} b'],
    ['space-padded', 'a {{ v }} b'],
    ['empty-var', 'a {{}} b'],
    ['dash-only', 'a {{-}} b'],
  ]) {
    add(`braces/${label}`, 'i18n_t', { lng: 'en', key: 'zz.i', opts: { defaultValue: tpl, v: 'V' } });
  }

  // --- $t() nesting
  add('nest/simple', 'i18n_t', { lng: 'en', key: 'zz.n', opts: { defaultValue: 'A $t(common.cancel) B' } });
  add('nest/missing-target', 'i18n_t', { lng: 'en', key: 'zz.n', opts: { defaultValue: 'A $t(zz.nope) B' } });
  add('nest/self-reference', 'i18n_t', { lng: 'en', key: 'zz.self', opts: { defaultValue: 'S $t(zz.self) E' } });
  add('nest/with-options', 'i18n_t', { lng: 'en', key: 'zz.n', opts: { defaultValue: 'A $t(send.recipientCount, {"count": 5}) B' } });
  add('nest/with-interp-passthrough', 'i18n_t', { lng: 'en', key: 'zz.n', opts: { defaultValue: 'A $t(zz.inner) B', inner: 'x' } });
  add('nest/double', 'i18n_t', { lng: 'en', key: 'zz.n', opts: { defaultValue: '$t(common.cancel) $t(common.error)' } });
  add('nest/malformed', 'i18n_t', { lng: 'en', key: 'zz.n', opts: { defaultValue: 'A $t(common.cancel B' } });
  add('nest/cross-locale', 'i18n_t', { lng: 'zh', key: 'zz.n', opts: { defaultValue: 'A $t(common.cancel) B' } });

  // --- array keys
  cases.push({
    name: 'keys/first-hit',
    fn: 'i18n_t_keys',
    input: { lng: 'en', keys: ['zz.nope', 'common.cancel'] },
    expect: { value: (i18n.changeLanguage('en'), i18n.t(['zz.nope', 'common.cancel'])) },
  });
  cases.push({
    name: 'keys/all-missing',
    fn: 'i18n_t_keys',
    input: { lng: 'en', keys: ['zz.a', 'zz.b'] },
    expect: { value: i18n.t(['zz.a', 'zz.b']) },
  });
  cases.push({
    name: 'keys/single',
    fn: 'i18n_t_keys',
    input: { lng: 'en', keys: ['common.cancel'] },
    expect: { value: i18n.t(['common.cancel']) },
  });
  cases.push({
    name: 'keys/empty-list',
    fn: 'i18n_t_keys',
    input: { lng: 'en', keys: [] },
    expect: expectOracleThrow('t([])', 'I18nEmptyKeyList', () => i18n.t([])),
    divergence: {
      ts_behavior: "i18next dereferences keys[keys.length-1] without a length check and throws a raw TypeError (\"Cannot read properties of undefined (reading 'includes')\")",
      reason: 'a typed Rust API returns a CoreError; an uncaught TypeError inside a render would blank the screen',
    },
  });

  // --- branch nodes / non-string leaves
  add('branch/object-node', 'i18n_t', { lng: 'en', key: 'common' });
  add('branch/root-namespace', 'i18n_t', { lng: 'en', key: 'send' });
  add('branch/object-joinArrays', 'i18n_t', { lng: 'en', key: 'common', opts: { joinArrays: '|' } });
  // returnObjects / returnDetails hand back a JS object. The Rust surface is
  // `-> Result<String, CoreError>`, so these option shapes do not exist there;
  // pinned so a future "just add returnObjects" PR has to confront the choice.
  for (const [name, k, opts] of [
    ['branch/object-returnObjects', 'common', { returnObjects: true }],
    ['leaf/returnDetails', 'common.cancel', { returnDetails: true }],
    ['default/array-joinArrays', 'zz.missing', { defaultValue: ['a', 'b'], joinArrays: '-' }],
  ]) {
    const got = tExpect('en', k, opts);
    if (!got.non_string) throw new Error(`i18n.dump: ${name} was expected to return a non-string`);
    anomalies.push(`NON-STRING ${name}: ${got.non_string} ${JSON.stringify(got.value)}`);
    cases.push({
      name,
      fn: 'i18n_t',
      input: { lng: 'en', key: k, opts },
      expect: { error: 'I18nUnsupportedOption' },
      divergence: {
        ts_behavior: `returns a JS ${got.non_string}: ${JSON.stringify(got.value).slice(0, 120)}`,
        reason: 'the Rust t() returns String; object-returning options are deliberately not ported',
      },
    });
  }

  // --- non-string / degenerate keys
  add('key/number', 'i18n_t', { lng: 'en', key: 123 });
  add('key/null-literal-in-list', 'i18n_t_keys', { lng: 'en', keys: [null] });

  // --- namespaces
  add('ns/explicit-prefix', 'i18n_t', { lng: 'en', key: 'translation:common.cancel' });
  add('ns/bogus-prefix', 'i18n_t', { lng: 'en', key: 'bogus:common.cancel' });
  add('ns/empty-prefix', 'i18n_t', { lng: 'en', key: ':common.cancel' });
  add('ns/double-colon', 'i18n_t', { lng: 'en', key: 'translation::common.cancel' });
  add('ns/option-ns', 'i18n_t', { lng: 'en', key: 'common.cancel', opts: { ns: 'translation' } });
  add('ns/option-bogus-ns', 'i18n_t', { lng: 'en', key: 'common.cancel', opts: { ns: 'bogus' } });

  // --- natural-language-looking keys (the "key is a sentence" fallback path)
  for (const s of [
    'Hello, world. How are you?',
    'Send',
    'a.b.c.d.e',
    'Price: $1.00',
    'trailing.dot.',
    '.leading.dot',
    '',
    '   ',
    'emoji 🚀 key',
    'key\nwith\nnewlines',
  ]) {
    add(`natural/${JSON.stringify(s)}`, 'i18n_t', { lng: 'en', key: s });
  }

  // --- keySeparator / nsSeparator overrides
  add('sep/keySeparator-false', 'i18n_t', { lng: 'en', key: 'common.cancel', opts: { keySeparator: false } });
  add('sep/nsSeparator-false', 'i18n_t', { lng: 'en', key: 'bogus:common.cancel', opts: { nsSeparator: false } });

  // --- fallback: keys that exist ONLY in en, requested from another locale
  const enOnly = KEY_PATHS.filter(
    (k) => leafSet('en').has(k) && !leafSet('ja').has(k),
  ).slice(0, 6);
  for (const k of enOnly) add(`fallback/ja-misses/${k}`, 'i18n_t', { lng: 'ja', key: k });
  const ruOnly = KEY_PATHS.filter((k) => leafSet('ru').has(k) && !leafSet('en').has(k)).slice(0, 6);
  for (const k of ruOnly) add(`fallback/en-misses/${k}`, 'i18n_t', { lng: 'en', key: k });

  // --- interpolator in isolation: a red case here means "interpolation is
  // wrong", not "store lookup is wrong". Same templates, driven through the
  // Interpolator service directly rather than through t().
  for (const [label, tpl, vals] of [
    ['plain', 'V=[{{v}}]', { v: 'X' }],
    ['missing', 'V=[{{v}}]', {}],
    ['unescape', 'V=[{{- v}}]', { v: '<b>&</b>' }],
    ['two-vars', '{{a}}/{{b}}', { a: '1', b: '2' }],
    ['repeat-var', '{{a}}{{a}}', { a: 'z' }],
    ['dotted', '{{a.b}}', { a: { b: 'D' } }],
    ['self-referential-value', '{{a}}', { a: '{{a}}' }],
    ['unbalanced-open', 'a {{v', { v: 'V' }],
    ['unbalanced-close', 'a v}} b', { v: 'V' }],
    ['empty-template', '', { v: 'V' }],
  ]) {
    i18n.changeLanguage('en');
    cases.push({
      name: `interpolate/${label}`,
      fn: 'i18n_interpolate',
      input: { lng: 'en', template: tpl, opts: vals },
      expect: { value: i18n.services.interpolator.interpolate(tpl, decodeTag(vals), 'en', {}) },
    });
  }

  return { cases, anomalies };
}

const _leafCache = new Map();
function leafSet(lng) {
  if (!_leafCache.has(lng)) _leafCache.set(lng, new Set(leafPaths(resources[lng].translation, '', [])));
  return _leafCache.get(lng);
}

// ---------------------------------------------------------------------------
// SUITE: language-code normalisation
// ---------------------------------------------------------------------------

const LANG_CODES = [
  'en', 'EN', 'de', 'DE', 'De', 'zh', 'ZH', 'Zh', 'zh-TW', 'zh-tw', 'ZH-TW',
  'ZH-tw', 'zH-Tw', 'zh_TW', 'zh-Hant', 'es-AR', 'zh-Hant-TW',
];

function buildLanguage() {
  const cases = [];
  for (const code of LANG_CODES) {
    // (1) init-time resolution — what src/i18n/index.ts does at startup
    const inst = i18next.createInstance();
    inst.init({ ...INIT, lng: code });
    cases.push({
      name: `resolve-init/${code}`,
      fn: 'i18n_resolve_language',
      input: { requested: code },
      expect: { language: inst.language, resolved_language: inst.resolvedLanguage, languages: inst.languages },
    });
    // (2) per-call {lng} resolution — a DIFFERENT code path in i18next
    i18n.changeLanguage(FALLBACK_LANGUAGE);
    cases.push({
      name: `resolve-per-call/${code}`,
      fn: 'i18n_t_lng_option',
      input: { key: 'language.title', opts: { lng: code } },
      expect: { value: i18n.t('language.title', { lng: code }) },
    });
    // (3) changeLanguage resolution
    const inst2 = i18next.createInstance();
    inst2.init({ ...INIT });
    inst2.changeLanguage(code);
    cases.push({
      name: `resolve-change/${code}`,
      fn: 'i18n_change_language',
      input: { requested: code },
      expect: { language: inst2.language, resolved_language: inst2.resolvedLanguage, languages: inst2.languages },
    });
  }
  return cases;
}

// ---------------------------------------------------------------------------
// SUITES (c)/(d): plural, mode A (full Intl.PluralRules) and mode B (deleted)
// ---------------------------------------------------------------------------

const PLURAL_COUNTS = [0, 1, 2, 3, 5, 11, 21, 101, 1000000];

function buildPlural(inst, fnName) {
  const cases = [];
  const anomalies = [];
  for (const lng of LOCALES) {
    inst.changeLanguage(lng);
    // The CLDR rule itself, isolated from the store: a red case here says
    // "plural rules are wrong", not "lookup is wrong".
    cases.push({
      name: `suffixes/${lng}`,
      fn: fnName === 'i18n_t' ? 'i18n_plural_suffixes' : 'i18n_plural_suffixes_legacy',
      input: { lng },
      expect: { value: inst.services.pluralResolver.getSuffixes(lng) },
    });
    for (const count of PLURAL_COUNTS) {
      cases.push({
        name: `suffix/${lng}/${count}`,
        fn: fnName === 'i18n_t' ? 'i18n_plural_suffix' : 'i18n_plural_suffix_legacy',
        input: { lng, count },
        expect: { value: inst.services.pluralResolver.getSuffix(lng, count) },
      });
      for (const key of PLURAL_BASES) {
        let v;
        try {
          v = inst.t(key, { count });
        } catch (e) {
          anomalies.push(`${lng}/${key}/${count} THREW ${e}`);
          continue;
        }
        if (typeof v !== 'string') anomalies.push(`${lng}/${key}/${count} -> ${typeof v}`);
        cases.push({
          name: `${lng}/${key}/${count}`,
          fn: fnName,
          input: { lng, key, opts: { count } },
          expect: { value: v },
        });
      }
    }
  }
  return { cases, anomalies };
}

// ---------------------------------------------------------------------------
// Emit
// ---------------------------------------------------------------------------

const sizes = {};
const record = (file) => {
  if (file === null) return null; // suppressed ALT encoding
  sizes[file.split('/').pop()] = statSync(file).size;
  return file;
};

const { values, nonString } = buildExhaustive();

// --- encoding A: columnar (chosen)
const fileColumnar = record(writeSuite('i18n-exhaustive', {
  locales: LOCALES,
  keys: KEY_PATHS,
  values,
}));

// --- encodings B and C exist only to keep research.md D4's 2.21x / 5.38x size
// comparison reproducible. Skipped entirely on a normal run.
// --- encoding B: flat triples (`pairs`-style, like identicon-bulk)
if (WRITE_ALT) {
  const triples = [];
  for (const lng of LOCALES) for (let i = 0; i < KEY_PATHS.length; i++) triples.push([lng, KEY_PATHS[i], values[lng][i]]);
  record(writeSuite('i18n-exhaustive-ALT-triples', { pairs: triples }));
}
// --- encoding C: full VectorCase
if (WRITE_ALT) {
  const cases = [];
  for (const lng of LOCALES) for (let i = 0; i < KEY_PATHS.length; i++) {
    cases.push({ name: `${lng}/${KEY_PATHS[i]}`, fn: 'i18n_t', input: { lng, key: KEY_PATHS[i] }, expect: { value: values[lng][i] } });
  }
  record(writeSuite('i18n-exhaustive-ALT-cases', { cases }));
}

const behaviour = buildBehaviour();
const langCases = buildLanguage();
record(writeSuite('i18n-behaviour', { cases: [...behaviour.cases, ...langCases] }));

const modeA = buildPlural(i18n, 'i18n_t');
record(writeSuite('i18n-plural', { cases: modeA.cases }));

// --- MODE B: Intl.PluralRules deleted. A brand-new instance is required because
// PluralResolver memoises rules per instance in `pluralRulesCache`.
const savedPluralRules = Intl.PluralRules;
delete Intl.PluralRules;
const legacy = i18next.createInstance();
legacy.init({ ...INIT });
const modeB = buildPlural(legacy, 'i18n_t_legacy_plural');
Intl.PluralRules = savedPluralRules;
record(writeSuite('i18n-plural-legacy', { cases: modeB.cases }));

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

const kb = (n) => `${(n / 1024).toFixed(1)} KiB`;
console.log(`key paths (union over 15 locales): ${KEY_PATHS.length}`);
console.log(`per-locale leaf counts: ${LOCALES.map((l) => `${l}=${leafSet(l).size}`).join(' ')}`);
console.log(`sum of per-locale leaves: ${LOCALES.reduce((a, l) => a + leafSet(l).size, 0)}`);
console.log(`plural bases: ${PLURAL_BASES.length} -> ${PLURAL_BASES.join(', ')}`);
console.log('');
console.log(`(a) exhaustive           : ${KEY_PATHS.length} keys x ${LOCALES.length} locales = ${KEY_PATHS.length * LOCALES.length} cases`);
console.log(`(b) behaviour + language : ${behaviour.cases.length} + ${langCases.length} = ${behaviour.cases.length + langCases.length} cases`);
console.log(`(c) plural mode A        : ${modeA.cases.length} cases`);
console.log(`(d) plural mode B        : ${modeB.cases.length} cases`);
console.log(`TOTAL                    : ${KEY_PATHS.length * LOCALES.length + behaviour.cases.length + langCases.length + modeA.cases.length + modeB.cases.length}`);
console.log('');
for (const [f, s] of Object.entries(sizes)) console.log(`  ${f.padEnd(34)} ${kb(s).padStart(12)}  (${s} bytes)`);
console.log('');
console.log(`non-string / throwing results in the exhaustive suite: ${nonString.length}`);
for (const a of nonString.slice(0, 20)) console.log(`  ${a}`);
console.log(`anomalies in behaviour suite: ${behaviour.anomalies.length}`);
for (const a of behaviour.anomalies) console.log(`  ${a}`);
console.log(`anomalies in plural A: ${modeA.anomalies.length}  B: ${modeB.anomalies.length}`);
for (const a of [...modeA.anomalies, ...modeB.anomalies].slice(0, 10)) console.log(`  ${a}`);
console.log(`\nchosen exhaustive encoding: ${fileColumnar}`);
