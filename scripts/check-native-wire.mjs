#!/usr/bin/env node
/**
 * The native wire ruler (native parity program, F0.3): what a native shell's
 * wire declarations say against the generated mirror of the core
 * (`app-web/vela-wallet/src/lib/core/generated`, ts-rs, committed) — per
 * machine, the operations it cannot answer, the results it cannot say, the
 * result fields it leaves out, the events the web dispatches that it never
 * does, and the view fields it does not decode.
 *
 *   node scripts/check-native-wire.mjs                 # Android, markdown on stdout
 *   node scripts/check-native-wire.mjs --platform android --out docs/android/wire-drift.md
 *   node scripts/check-native-wire.mjs --strict        # exit 1 on any missing op/result/required field
 *
 * Android is the platform it reads today: its wire is kotlinx `@Serializable`
 * sealed hierarchies in `*Wire.kt` (and `CoreViews.kt`), whose `@SerialName`s
 * are the Rust variants' names. The onboarding executor, the one machine still
 * answered from `org.json`, is read from its `when` arms. The Kotlin name a
 * mirror goes by is the one `CoreWireDriftTest` pairs it with
 * (`assertVariantsExhaustive<Kotlin>("Mirror")`), else the same name.
 *
 * The CI gate is `CoreWireDriftTest`; this is the table a person reads (and
 * the one the parity audit counts), so its exit code stays 0 unless asked.
 */
import { readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { basename, join } from 'node:path';

const ROOT = new URL('..', import.meta.url).pathname;
const GEN = join(ROOT, 'app-web/vela-wallet/src/lib/core/generated');
const ONBOARDING_GEN = join(ROOT, 'app-web/vela-wallet/src/lib/onboarding/generated');
const WEB = join(ROOT, 'app-web/vela-wallet/src');
const ANDROID_SRC = join(ROOT, 'app-android/vela-wallet/app/src/main/java/app/getvela/wallet');
const ANDROID_DRIFT_TEST = join(
	ROOT,
	'app-android/vela-wallet/app/src/test/java/app/getvela/wallet/CoreWireDriftTest.kt'
);

const args = process.argv.slice(2);
const flag = (name) => args.includes(name);
const option = (name) => {
	const at = args.indexOf(name);
	return at >= 0 ? args[at + 1] : undefined;
};
const platform = option('--platform') ?? 'android';
if (platform !== 'android') {
	console.error(`check-native-wire: platform '${platform}' is not read yet (android only)`);
	process.exit(2);
}

function walk(dir, out = []) {
	for (const name of readdirSync(dir)) {
		const p = join(dir, name);
		if (statSync(p).isDirectory()) {
			if (!/node_modules|__tests__|[/\\]generated$/.test(p)) walk(p, out);
		} else out.push(p);
	}
	return out;
}

// ---------------------------------------------------------------------------
// The mirror (ts-rs): `export type X = { "type": "a", f: T, } | { … };`
// ---------------------------------------------------------------------------

function splitTop(text, sep) {
	const parts = [];
	let depth = 0;
	let cur = '';
	for (const ch of text) {
		if ('{<(['.includes(ch)) depth++;
		if ('}>)]'.includes(ch)) depth--;
		if (ch === sep && depth === 0) {
			parts.push(cur);
			cur = '';
		} else cur += ch;
	}
	parts.push(cur);
	return parts.filter((p) => p.trim() !== '');
}

/** `[{ type, fields: Map<name, {type, optional}> }]`, one per union arm; a struct is one arm with no type. */
function tsMembers(dir, name) {
	let text;
	try {
		text = readFileSync(join(dir, `${name}.ts`), 'utf8');
	} catch {
		return null;
	}
	const body = text
		.replace(/\/\*[\s\S]*?\*\//g, ' ')
		.replace(/^\s*\/\/.*$/gm, '')
		.split(`export type ${name} =`)[1];
	if (body === undefined) return null;
	const decl = body.slice(0, body.lastIndexOf(';'));
	return splitTop(decl, '|')
		.map((arm) => arm.trim())
		.filter((arm) => arm.startsWith('{'))
		.map((arm) => {
			const inner = arm.trim().replace(/^\{/, '').replace(/\}$/, '');
			const fields = new Map();
			let type = null;
			for (const field of splitTop(inner, ',')) {
				const colon = field.indexOf(':');
				if (colon < 0) continue;
				let key = field.slice(0, colon).trim().replace(/^"|"$/g, '');
				const value = field.slice(colon + 1).trim();
				const optional = key.endsWith('?');
				key = key.replace(/\?$/, '');
				if (key === 'type') {
					type = value.replace(/^"|"$/g, '');
					continue;
				}
				fields.set(key, { type: value, optional: optional || /\|\s*null\b/.test(value) });
			}
			return { type, fields };
		});
}

/** A fieldless Rust enum: `export type X = "a" | "b";` */
function tsStringUnion(dir, name) {
	try {
		const text = readFileSync(join(dir, `${name}.ts`), 'utf8').replace(/\/\*[\s\S]*?\*\//g, ' ');
		const decl = text.split(`export type ${name} =`)[1];
		if (!decl || decl.trim().startsWith('{')) return null;
		return [...decl.slice(0, decl.indexOf(';')).matchAll(/"([a-z0-9_]+)"/g)].map((m) => m[1]);
	} catch {
		return null;
	}
}

// ---------------------------------------------------------------------------
// Kotlin wire: `sealed class X { @Serializable @SerialName("a") data class A(val f: T) : X() … }`
// ---------------------------------------------------------------------------

const androidFiles = walk(ANDROID_SRC).filter((f) => f.endsWith('.kt'));
const wireFiles = androidFiles.filter((f) => /Wire\.kt$|CoreViews\.kt$/.test(f));
const stripComments = (text) =>
	text.replace(/\/\*[\s\S]*?\*\//g, (c) => c.replace(/[^\n]/g, ' ')).replace(/\/\/[^\n]*/g, '');
const wireText = wireFiles.map((f) => stripComments(readFileSync(f, 'utf8'))).join('\n');
const codeFiles = androidFiles.filter((f) => !wireFiles.includes(f));
const codeText = codeFiles.map((f) => stripComments(readFileSync(f, 'utf8'))).join('\n');

function blockFrom(text, openAt) {
	let depth = 0;
	for (let i = openAt; i < text.length; i++) {
		if (text[i] === '{') depth++;
		else if (text[i] === '}') {
			depth--;
			if (depth === 0) return text.slice(openAt + 1, i);
		}
	}
	return text.slice(openAt + 1);
}

function parenFrom(text, openAt) {
	let depth = 0;
	for (let i = openAt; i < text.length; i++) {
		if (text[i] === '(') depth++;
		else if (text[i] === ')') {
			depth--;
			if (depth === 0) return text.slice(openAt + 1, i);
		}
	}
	return '';
}

function paramNames(params) {
	return splitTop(params, ',')
		.map((p) => {
			const serial = p.match(/@SerialName\("([^"]+)"\)/);
			const name = p.replace(/@\w+(\([^)]*\))?/g, '').match(/\bval\s+(\w+)\s*:/);
			return serial ? serial[1] : name ? name[1] : null;
		})
		.filter(Boolean);
}

/** `Map<variant, Set<field>>` for a Kotlin sealed class, or null. */
function kotlinSealed(name) {
	const re = new RegExp(`sealed\\s+(?:class|interface)\\s+${name}\\b[^{]*\\{`);
	const m = re.exec(wireText);
	if (!m) return null;
	const block = blockFrom(wireText, m.index + m[0].length - 1);
	const variants = new Map();
	const decl = /@SerialName\("([a-z0-9_]+)"\)\s*(?:@\w+(?:\([^)]*\))?\s*)*(data\s+class|data\s+object|object|class)\s+(\w+)/g;
	let d;
	while ((d = decl.exec(block)) !== null) {
		const after = d.index + d[0].length;
		let fields = [];
		if (/class$/.test(d[2]) && block[after] === '(') fields = paramNames(parenFrom(block, after));
		else if (/class$/.test(d[2])) {
			const open = block.indexOf('(', after);
			const colon = block.indexOf(':', after);
			if (open >= 0 && (colon < 0 || open < colon)) fields = paramNames(parenFrom(block, open));
		}
		variants.set(d[1], { cls: d[3], fields: new Set(fields) });
	}
	return variants;
}

/** The fields of a Kotlin wire data class, or null. */
function kotlinStruct(name) {
	// kotlinx wire types only: a hand-built view (`CoreViews.kt`) has its own names.
	const re = new RegExp(`@Serializable\\s*(?:@\\w+(?:\\([^)]*\\))?\\s*)*data\\s+class\\s+${name}\\s*\\(`);
	const m = re.exec(wireText);
	if (!m) return null;
	return new Set(paramNames(parenFrom(wireText, m.index + m[0].length - 1)));
}

// The Kotlin name each mirror goes by, as the drift gate pairs them.
const pairs = new Map();
try {
	const test = readFileSync(ANDROID_DRIFT_TEST, 'utf8');
	for (const m of test.matchAll(/<([\w.]+)>\("(\w+)"\)/g)) {
		const kotlin = m[1].split('.').pop();
		if (!pairs.has(m[2])) pairs.set(m[2], new Set());
		pairs.get(m[2]).add(kotlin);
	}
} catch {
	// No gate file: same names only.
}
const kotlinNamesFor = (ts) => [...(pairs.get(ts) ?? []), ts];
const firstSealed = (ts) => {
	for (const k of kotlinNamesFor(ts)) {
		const v = kotlinSealed(k);
		if (v) return { name: k, variants: v };
	}
	return null;
};

// ---------------------------------------------------------------------------
// The web's dispatches (the event ruler's rule, spec 047 US5)
// ---------------------------------------------------------------------------

const webText = walk(WEB)
	.filter((f) => /\.(ts|svelte)$/.test(f) && !/\.test\./.test(f))
	.map((f) => readFileSync(f, 'utf8'))
	.join('\n');
const webDispatches = (variant) => new RegExp(`type:\\s*['"]${variant}['"]`).test(webText);
function androidDispatches(variant, cls) {
	if (cls && new RegExp(`\\b${cls}\\b`).test(codeText)) return true;
	return new RegExp(`event\\("${variant}"\\)|"${variant}"\\s*\\)|put\\("type",\\s*"${variant}"\\)`).test(codeText);
}

// ---------------------------------------------------------------------------
// The onboarding executor (org.json): its `when` arms and the results it says
// ---------------------------------------------------------------------------

function onboardingJson() {
	const file = join(ANDROID_SRC, 'feature/onboarding/core/OnboardingExecutor.kt');
	const text = stripComments(readFileSync(file, 'utf8'));
	const runAt = text.indexOf('private suspend fun run(');
	const run = blockFrom(text, text.indexOf('{', text.indexOf('when (val type', runAt)));
	const ops = new Set([...run.matchAll(/^\s*"([a-z0-9_]+)"\s*(?:,\s*"[a-z0-9_]+"\s*)*->/gm)].flatMap((m) =>
		[...m[0].matchAll(/"([a-z0-9_]+)"/g)].map((x) => x[1])
	));
	const results = new Map();
	for (const m of text.matchAll(/result\("([a-z0-9_]+)"\)\s*\{/g)) {
		const body = blockFrom(text, m.index + m[0].length - 1);
		const fields = results.get(m[1]) ?? new Set();
		for (const f of body.matchAll(/put\(\s*"([a-z0-9_]+)"/g)) fields.add(f[1]);
		results.set(m[1], fields);
	}
	return { ops, results };
}

// ---------------------------------------------------------------------------
// The report
// ---------------------------------------------------------------------------

const machines = readdirSync(GEN)
	.filter((f) => /Operation\.ts$/.test(f) && f !== 'FeedDappOperation.ts')
	.map((f) => basename(f, 'Operation.ts'))
	.sort();

const rows = [];
const totals = { ops: 0, staleOps: 0, results: 0, staleResults: 0, requiredFields: 0, optionalFields: 0, events: 0, staleEvents: 0 };
const notDriven = [];

function compareFamily(tsName, dir, kotlin) {
	const mirror = tsMembers(dir, tsName) ?? [];
	const mirrorNames = mirror.map((m) => m.type).filter(Boolean);
	const kotlinNames = kotlin ? [...kotlin.keys()] : [];
	const missing = mirrorNames.filter((v) => !kotlinNames.includes(v));
	const stale = kotlinNames.filter((v) => !mirrorNames.includes(v));
	const fields = [];
	for (const arm of mirror) {
		const k = kotlin?.get(arm.type);
		if (!k) continue;
		for (const [field, info] of arm.fields) {
			const has = k.fields instanceof Set ? k.fields.has(field) : false;
			if (!has) fields.push({ variant: arm.type, field, optional: info.optional });
		}
		for (const field of k.fields ?? []) {
			if (!arm.fields.has(field)) fields.push({ variant: arm.type, field, stale: true });
		}
	}
	return { missing, stale, fields };
}

for (const machine of machines) {
	const opName = `${machine}Operation`;
	const resName = `${machine}ShellResult`;
	const evName = `${machine}Event`;
	const ops = firstSealed(opName);
	if (!ops) {
		notDriven.push(machine);
		continue;
	}
	const res = firstSealed(resName);
	const opCmp = compareFamily(opName, GEN, ops.variants);
	const resCmp = compareFamily(resName, GEN, res?.variants ?? null);
	// Event names: the mirror is `XEvent` (or the drift-paired Kotlin name).
	const evTs = [evName, `${machine}SigningEvent`, `${machine}ImportEvent`].find((n) => tsMembers(GEN, n) !== null);
	const evMirror = evTs ? tsMembers(GEN, evTs) : [];
	const ev = (evTs && firstSealed(evTs)) ?? firstSealed(evName);
	const evMirrorNames = evMirror.map((m) => m.type).filter(Boolean);
	const evUndispatched = evMirrorNames.filter(
		(v) => webDispatches(v) && !androidDispatches(v, ev?.variants.get(v)?.cls)
	);
	const evStale = ev ? [...ev.variants.keys()].filter((v) => !evMirrorNames.includes(v)) : [];
	const required = resCmp.fields.filter((f) => !f.stale && !f.optional);
	const optional = resCmp.fields.filter((f) => !f.stale && f.optional);
	totals.ops += opCmp.missing.length;
	totals.staleOps += opCmp.stale.length;
	totals.results += resCmp.missing.length;
	totals.staleResults += resCmp.stale.length;
	totals.requiredFields += required.length;
	totals.optionalFields += optional.length;
	totals.events += evUndispatched.length;
	totals.staleEvents += evStale.length;
	rows.push({
		machine,
		kotlin: ops.name,
		ops: opCmp,
		res: resCmp,
		required,
		optional,
		opStaleFields: opCmp.fields.filter((f) => f.stale),
		evUndispatched,
		evStale
	});
}

// The multisig shell (native multisig M1–M6): ONE operation family for the
// five machines (`MultisigOperation` / `MultisigShellResult`, the multisig
// mirror), and each machine's events — Android writes them as `ev("name", …)`
// (`feature/multisig`), so an event is "dispatched" when that call names it.
const MULTISIG_GEN = join(ROOT, 'app-web/vela-wallet/src/lib/multisig/generated');
const multisigOps = firstSealed('MultisigOperation');
if (multisigOps) {
	const multisigRes = firstSealed('MultisigShellResult');
	const opCmp = compareFamily('MultisigOperation', MULTISIG_GEN, multisigOps.variants);
	const resCmp = compareFamily('MultisigShellResult', MULTISIG_GEN, multisigRes?.variants ?? null);
	const required = resCmp.fields.filter((f) => !f.stale && !f.optional);
	const optional = resCmp.fields.filter((f) => !f.stale && f.optional);
	const multisigCode = androidFiles
		.filter((f) => f.includes('/feature/multisig/'))
		.map((f) => stripComments(readFileSync(f, 'utf8')))
		.join('\n');
	const sent = new Set([...multisigCode.matchAll(/\bev\(\s*"([a-z_]+)"/g)].map((m) => m[1]));
	// The outside PAGE runs in a browser with the owner's wallet: no Android shell drives it.
	const families = ['MultisigAccountsEvent', 'MultisigProposalsEvent', 'MultisigMembersEvent', 'OutsideSessionEvent'];
	const evUndispatched = families.flatMap((family) =>
		(tsMembers(MULTISIG_GEN, family) ?? [])
			.map((m) => m.type)
			.filter((v) => v && webDispatches(v) && !sent.has(v))
			.map((v) => `${family.replace(/Event$/, '')}.${v}`)
	);
	totals.ops += opCmp.missing.length;
	totals.staleOps += opCmp.stale.length;
	totals.results += resCmp.missing.length;
	totals.staleResults += resCmp.stale.length;
	totals.requiredFields += required.length;
	totals.optionalFields += optional.length;
	totals.events += evUndispatched.length;
	rows.push({
		machine: 'Multisig (5 machines)',
		kotlin: multisigOps.name,
		ops: opCmp,
		res: resCmp,
		required,
		optional,
		opStaleFields: opCmp.fields.filter((f) => f.stale),
		evUndispatched,
		evStale: []
	});
}

// Onboarding (org.json): `ShellOperation` / `ShellResult` in the onboarding mirror.
const onboarding = (() => {
	const { ops, results } = onboardingJson();
	const mirrorOps = (tsMembers(ONBOARDING_GEN, 'ShellOperation') ?? []).map((m) => m.type);
	const mirrorResults = tsMembers(ONBOARDING_GEN, 'ShellResult') ?? [];
	const missingOps = mirrorOps.filter((v) => !ops.has(v));
	const staleOps = [...ops].filter((v) => !mirrorOps.includes(v));
	const unknownResults = [...results.keys()].filter((r) => !mirrorResults.some((m) => m.type === r));
	const fields = [];
	for (const [variant, put] of results) {
		const arm = mirrorResults.find((m) => m.type === variant);
		if (!arm) continue;
		for (const [field, info] of arm.fields) if (!put.has(field) && !info.optional) fields.push(`${variant}.${field}`);
	}
	totals.ops += missingOps.length;
	totals.staleOps += staleOps.length;
	totals.staleResults += unknownResults.length;
	totals.requiredFields += fields.length;
	return { missingOps, staleOps, unknownResults, fields };
})();

// Closed string enums (a fieldless Rust enum is a TS string union): a value the
// Kotlin enum lacks is a whole view or answer that throws on decode.
function kotlinEnum(name) {
	const m = new RegExp(`enum\\s+class\\s+${name}\\b[^{]*\\{`).exec(wireText);
	if (!m) return null;
	const values = [...blockFrom(wireText, m.index + m[0].length - 1).matchAll(/@SerialName\("([a-z0-9_]+)"\)/g)].map((x) => x[1]);
	// No @SerialName: a hand-decoded enum (its own `wire` names), not a kotlinx one.
	return values.length ? values : null;
}
const enums = [];
for (const file of readdirSync(GEN).filter((f) => f.endsWith('.ts'))) {
	const ts = basename(file, '.ts');
	const values = tsStringUnion(GEN, ts);
	if (!values || values.length === 0) continue;
	const kotlin = kotlinNamesFor(ts).map(kotlinEnum).find(Boolean);
	if (!kotlin) continue;
	const missing = values.filter((v) => !kotlin.includes(v));
	const stale = kotlin.filter((v) => !values.includes(v));
	if (missing.length || stale.length) enums.push({ ts, missing, stale });
}
totals.enumValues = enums.reduce((n, e) => n + e.missing.length, 0);
totals.staleEnumValues = enums.reduce((n, e) => n + e.stale.length, 0);

// Views: the fields of each mirrored view the Kotlin wire does not decode.
const views = readdirSync(GEN)
	.filter((f) => /View\.ts$/.test(f))
	.map((f) => basename(f, '.ts'))
	.sort()
	.map((name) => {
		const mirror = tsMembers(GEN, name);
		if (!mirror || mirror.length !== 1 || mirror[0].type !== null) return null;
		const kotlin = kotlinNamesFor(name).map(kotlinStruct).find(Boolean);
		if (!kotlin) return { name, missing: null, total: mirror[0].fields.size };
		const missing = [...mirror[0].fields.keys()].filter((f) => !kotlin.has(f));
		return { name, missing, total: mirror[0].fields.size };
	})
	.filter(Boolean);
const viewsDecoded = views.filter((v) => v.missing !== null);
const viewFieldsMissing = viewsDecoded.reduce((n, v) => n + v.missing.length, 0);

const list = (xs) => (xs.length ? xs.join(', ') : '—');
const out = [];
out.push(`# Android wire drift vs the core mirror`);
out.push('');
out.push(
	'Generated by `node scripts/check-native-wire.mjs --platform android` — the Kotlin wire ' +
		'(`app-android/…/*Wire.kt`, `CoreViews.kt`, the onboarding executor\'s arms) against ' +
		'`app-web/vela-wallet/src/lib/core/generated` (ts-rs). The CI gate is `CoreWireDriftTest`; ' +
		'this is its table.'
);
out.push('');
out.push('## Totals');
out.push('');
out.push(`- operations Android cannot answer: **${totals.ops}**`);
out.push(`- operations Android declares that the core no longer has (stale): **${totals.staleOps}**`);
out.push(`- results Android cannot say: **${totals.results}**`);
out.push(`- results Android declares that the core no longer has (stale): **${totals.staleResults}**`);
out.push(`- required result fields Android leaves out: **${totals.requiredFields}**`);
out.push(`- optional result fields Android leaves out: **${totals.optionalFields}**`);
out.push(`- events the web dispatches that Android never does: **${totals.events}**`);
out.push(`- events Android declares that the core no longer has (stale): **${totals.staleEvents}**`);
out.push(`- closed enum values Android cannot decode: **${totals.enumValues}**`);
out.push(`- closed enum values Android declares that the core no longer has (stale): **${totals.staleEnumValues}**`);
out.push(`- view fields not decoded: **${viewFieldsMissing}** across ${viewsDecoded.length} decoded views`);
out.push('');
out.push('## Per machine');
out.push('');
out.push('| Machine | Ops missing | Ops stale | Results missing | Results stale | Result fields missing (required) | Result fields missing (optional) | Web-dispatched events Android never sends | Stale events |');
out.push('| --- | --- | --- | --- | --- | --- | --- | --- | --- |');
for (const r of rows) {
	out.push(
		`| ${r.machine} | ${list(r.ops.missing)} | ${list(r.ops.stale)} | ${list(r.res.missing)} | ${list(r.res.stale)} | ` +
			`${list(r.required.map((f) => `${f.variant}.${f.field}`))} | ${list(r.optional.map((f) => `${f.variant}.${f.field}`))} | ` +
			`${list(r.evUndispatched)} | ${list(r.evStale)} |`
	);
}
out.push(
	`| Onboarding (org.json) | ${list(onboarding.missingOps)} | ${list(onboarding.staleOps)} | — | ${list(onboarding.unknownResults)} | ${list(onboarding.fields)} | — | — | — |`
);
out.push('');
const opFieldRows = rows.filter((r) => r.opStaleFields.length);
if (opFieldRows.length) {
	out.push('Operation fields Android requires that the mirror does not carry (a decode that would throw):');
	out.push('');
	for (const r of opFieldRows) out.push(`- ${r.machine}: ${r.opStaleFields.map((f) => `${f.variant}.${f.field}`).join(', ')}`);
	out.push('');
}
out.push(`Machines Android does not drive (no Kotlin operation family): ${list(notDriven)}.`);
out.push('');
out.push('## Closed enums (string unions) that differ');
out.push('');
if (enums.length === 0) out.push('None: every Kotlin enum paired with a mirror carries exactly its values.');
else {
	out.push('| Mirror | Values Android cannot decode | Stale values |');
	out.push('| --- | --- | --- |');
	for (const e of enums) out.push(`| ${e.ts} | ${list(e.missing)} | ${list(e.stale)} |`);
}
out.push('');
out.push('## Views: mirror fields the Kotlin wire does not decode');
out.push('');
out.push('A view may be a subset (the drift gate allows it); a subset is a choice to record.');
out.push('');
out.push('| View | Fields | Not decoded |');
out.push('| --- | --- | --- |');
for (const v of views) {
	out.push(`| ${v.name} | ${v.total} | ${v.missing === null ? '(no Kotlin type)' : list(v.missing)} |`);
}
out.push('');

const text = out.join('\n');
const file = option('--out');
if (file) writeFileSync(join(ROOT, file), text);
else console.log(text);

const hard = totals.ops + totals.results + totals.requiredFields + totals.enumValues;
console.error(
	`check-native-wire (${platform}): ${totals.ops} ops, ${totals.results} results, ${totals.requiredFields} required result fields missing; ` +
		`${totals.enumValues} enum values; ${totals.events} web events undispatched; ` +
		`${totals.staleOps + totals.staleResults + totals.staleEvents + totals.staleEnumValues} stale variants; ` +
		`${viewFieldsMissing} view fields undecoded`
);
if (flag('--strict') && hard > 0) process.exit(1);
// Exported for the ruler's own self-check.
export { tsMembers, tsStringUnion };
