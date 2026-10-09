// Fifteen languages, compiled into the page (spec 102 P3-04).
//
//   node samples/locales-test.mjs      (bun runs it unchanged)
//
// What it holds the catalogues to:
//   · the fifteen the apps ship (vela-core `i18n::resolve::SUPPORTED`), each
//     registered under the file's own name and named by its own endonym;
//   · every catalogue has exactly English's keys — a key one language lacks
//     shows English there, which on a signing prompt is a hole, not a style;
//   · every value says something, and names the same {placeholders} as
//     English's: a translation that drops {amount} drops the amount;
//   · the page loads every one of them (src/sign.html), so the published
//     bytes carry them all;
//   · the page picks the language the wallet would: vela-core's own
//     `match_system_tag` vectors, read from the core's test, must give the
//     same answer here.
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { root, makeChecks } from './test-kit.mjs';

const check = makeChecks();
const CORE = join(root, '../../rust/crates/vela-core/src/i18n/resolve.rs');
const core = readFileSync(CORE, 'utf8');

// The core's list and endonyms, read from the source the apps compile.
const supported = JSON.parse(
  /pub const SUPPORTED: \[&str; 15\] = (\[[^\]]+\]);/.exec(core)[1].replace(/,\s*\]/, ']'),
);
const endonyms = Object.fromEntries(
  [...core.slice(core.indexOf('pub fn endonym')).split('other => other')[0]
    .matchAll(/"([^"]+)" => "([^"]+)"/g)].map((m) => [m[1], m[2]]),
);

// The page's i18n and every catalogue, as the page loads them.
globalThis.window = globalThis;
globalThis.document = { documentElement: { setAttribute() {} } };
const html = readFileSync(join(root, 'src/sign.html'), 'utf8');
const scripts = [...html.matchAll(/<script src="([^"]+)"><\/script>/g)].map((m) => m[1]);
const localeFiles = scripts.filter((s) => s.startsWith('lib/locales/'));
(0, eval)(readFileSync(join(root, 'src/lib/i18n.js'), 'utf8'));
const registered = [];
const realRegister = globalThis.VelaCS.i18n.register;
globalThis.VelaCS.i18n.register = (code, entries) => {
  registered.push({ code, entries });
  realRegister(code, entries);
};
const absent = [];
for (const file of localeFiles) {
  let text = null;
  try { text = readFileSync(join(root, 'src', file), 'utf8'); } catch { absent.push(file); }
  if (text !== null) (0, eval)(text);
}
const ns = globalThis.VelaCS;
check('every catalogue the page names is there', !absent.length, absent.join(', '));

const onDisk = readdirSync(join(root, 'src/lib/locales')).filter((f) => f.endsWith('.js')).sort();
check('the page loads every catalogue on disk', JSON.stringify(localeFiles.map((f) => f.slice(12)).sort()) === JSON.stringify(onDisk),
  `${localeFiles.length} loaded, ${onDisk.length} on disk`);
check('the fifteen the apps ship, no more, no fewer',
  JSON.stringify(registered.map((r) => r.code).sort()) === JSON.stringify([...supported].sort()),
  registered.map((r) => r.code).join(' '));
check('each file registers under its own name',
  localeFiles.every((file, i) => registered[i] && file === `lib/locales/${registered[i].code}.js`));

const en = registered.find((r) => r.code === 'en').entries;
const enKeys = Object.keys(en);
const placeholders = (text) => (String(text).match(/\{\w+\}/g) || []).sort().join(' ');

for (const { code, entries } of registered) {
  const keys = Object.keys(entries);
  const missing = enKeys.filter((k) => !(k in entries));
  const extra = keys.filter((k) => !(k in en));
  check(`${code}: exactly English's ${enKeys.length} keys`, !missing.length && !extra.length,
    [missing.length && `missing ${missing.slice(0, 6).join(', ')}`, extra.length && `extra ${extra.slice(0, 6).join(', ')}`]
      .filter(Boolean).join('; '));
  const empty = keys.filter((k) => typeof entries[k] !== 'string' || !entries[k].trim());
  check(`${code}: every value says something`, !empty.length, empty.slice(0, 6).join(', '));
  const wrong = enKeys.filter((k) => k in entries && placeholders(entries[k]) !== placeholders(en[k]));
  check(`${code}: the same {placeholders} as English`, !wrong.length,
    wrong.slice(0, 4).map((k) => `${k}: ${placeholders(entries[k])} ≠ ${placeholders(en[k])}`).join('; '));
  check(`${code}: names itself ${endonyms[code]}`, entries['locale.name'] === endonyms[code], entries['locale.name']);
  if (code !== 'en') {
    // A value identical to English is allowed (a symbol, "Permit2 …"), but
    // a catalogue that is mostly English was never translated.
    const same = enKeys.filter((k) => entries[k] === en[k] && /[a-z]{4,}/.test(en[k]));
    check(`${code}: translated, not copied (${same.length} of ${enKeys.length} lines read as English)`,
      same.length < enKeys.length * 0.08, same.slice(0, 8).join(', '));
  }
}

// The language the page picks: the core's own vectors.
const block = core.slice(core.indexOf('fn a_platform_tag_finds_its_shipped_locale'));
const body = block.slice(0, block.indexOf('fn the_first_served_preference_wins'));
const vectors = [...body.split('] {')[0].matchAll(/\("([^"]*)", "([^"]+)"\)/g)].map((m) => [m[1], m[2]]);
const none = JSON.parse(/for tag in (\[[^\]]+\]) \{\s*assert_eq!\(match_system_tag\(tag\), None/.exec(body)[1]);
const bad = vectors.filter(([tag, want]) => ns.i18n.match(tag) !== want);
check(`a language tag finds the locale the core finds (${vectors.length} vectors)`, vectors.length > 20 && !bad.length,
  bad.map(([tag, want]) => `${tag} → ${ns.i18n.match(tag)}, core ${want}`).join('; '));
const badNone = none.filter((tag) => ns.i18n.match(tag) !== null);
check(`…and finds none where the core finds none (${none.length})`, !badNone.length, badNone.join(', '));
check('every shipped tag is itself', supported.every((tag) => ns.i18n.match(tag) === tag));
check('an explicit ?lang= wins, any case', ns.i18n.detect('ZH-tw') === 'zh-TW' && ns.i18n.detect('pt') === 'pt-BR');
Object.defineProperty(globalThis, 'navigator', { value: { languages: ['ar-SA', 'fr-CA', 'de'] }, configurable: true });
check('the first served preference wins', ns.i18n.detect(null) === 'fr');
Object.defineProperty(globalThis, 'navigator', { value: { languages: ['th', 'hi'] }, configurable: true });
check('nothing served is English', ns.i18n.detect('xx') === 'en');

process.exit(check.summary() ? 0 : 1);
