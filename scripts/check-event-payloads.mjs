#!/usr/bin/env node
/**
 * Do the shells send the core the FIELDS the event actually has?
 *
 * The two existing rulers (`check-android-event-parity.mjs`,
 * `check-ios-android-parity.mjs`) compare `"type"` strings: they answer "does
 * this client dispatch this event at all". Spec 081 FR-001 was a gap neither
 * could see. iOS dispatched `{"type":"endpoint_edited","id":…,"value":…}` where
 * the core declares `field`, so serde rejected the whole event and it never
 * reached `update`. Every type-string ruler was green, the Service Endpoints
 * page silently saved nothing, and it stayed that way for a release.
 *
 * So this one reads the payload. For each dispatch site written as a literal —
 * a Swift dictionary, a Kotlin `buildJsonObject`/`JSONObject`, a TypeScript
 * object — it takes the keys sitting beside `"type"` and compares them with the
 * variant's fields in `app-web/vela-wallet/src/lib/core/generated/*Event.ts`,
 * which ts-rs writes from the Rust enum. A key the core does not declare, or a
 * declared field nobody sends, is printed with its file and line.
 *
 * What it deliberately does NOT do: follow variables. A dispatch whose payload
 * is built up in pieces is skipped, and skipped sites are counted in the
 * summary so the number is honest about its own reach. Being unable to check
 * every site is not a reason to check none.
 *
 * Exit code 1 on any mismatch. Usage:
 *   node scripts/check-event-payloads.mjs [--machine NetEvent] [--json]
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, basename, relative } from 'node:path';

const ROOT = new URL('..', import.meta.url).pathname;
const GEN = join(ROOT, 'app-web/vela-wallet/src/lib/core/generated');
const SHELLS = [
	{ name: 'ios', dir: join(ROOT, 'app-ios/VelaWallet/VelaWallet'), ext: /\.swift$/ },
	{
		name: 'android',
		dir: join(ROOT, 'app-android/vela-wallet/app/src/main/java/app/getvela/wallet'),
		ext: /\.kt$/
	},
	{ name: 'web', dir: join(ROOT, 'app-web/vela-wallet/src'), ext: /\.(ts|svelte)$/ }
];

const arg = (flag) =>
	process.argv.includes(flag) ? process.argv[process.argv.indexOf(flag) + 1] : null;
const onlyMachine = arg('--machine');
const asJson = process.argv.includes('--json');

function walk(dir, out = []) {
	for (const name of readdirSync(dir)) {
		const p = join(dir, name);
		if (statSync(p).isDirectory()) {
			if (!/node_modules|generated|__tests__|\/build\/|\/dist\//.test(p)) walk(p, out);
		} else out.push(p);
	}
	return out;
}

const stripComments = (s) =>
	s.replace(/\/\*[\s\S]*?\*\//g, (m) => m.replace(/[^\n]/g, ' ')).replace(/^\s*\/\/.*$/gm, '');

/**
 * variant name → declared field names, from the ts-rs union. Parsed by
 * balancing braces rather than by one regex: a variant can carry a nested
 * object type, and a lazy `[^}]*` would stop at its first inner brace.
 */
function variantsOf(rawSource) {
	// ts-rs carries each field's Rust doc comment into the union, and a comment
	// sitting between two fields hides the second one from a "comma then name"
	// scan. Blank them, keeping the offsets.
	const source = rawSource.replace(/\/\*[\s\S]*?\*\//g, (m) => m.replace(/[^\n]/g, ' '));
	const out = new Map();
	for (let i = source.indexOf('{ "type"'); i !== -1; i = source.indexOf('{ "type"', i + 1)) {
		let depth = 0;
		let end = i;
		for (; end < source.length; end++) {
			if (source[end] === '{') depth++;
			else if (source[end] === '}' && --depth === 0) break;
		}
		const body = source.slice(i, end + 1);
		const name = body.match(/"type":\s*"([a-z0-9_]+)"/)?.[1];
		if (!name) continue;
		// Top-level keys only: anything inside a nested brace belongs to a field.
		const fields = [];
		let nest = 0;
		for (const m of body.matchAll(/[{}]|(?:^|[,{]\s*)([A-Za-z_][A-Za-z0-9_]*)\s*:/g)) {
			if (m[0] === '{') nest++;
			else if (m[0] === '}') nest--;
			else if (nest === 1 && m[1] !== 'type') fields.push(m[1]);
		}
		out.set(name, fields);
	}
	return out;
}

// variant name → one entry per machine that declares it. A wire name reused by
// several machines (`account_switched`, `chain_changed`, `open`…) cannot be
// attributed from the payload alone, so a dispatch matching ANY of its
// declarations is correct — reporting the others would be inventing a bug.
const declared = new Map();
// EVERY tagged union ts-rs generated, not just the Event ones. A shell answers
// an operation with a tagged result of the same shape, and nested payloads
// (`SignSubmitOutcome`) are tagged too; several share a wire name with an event
// (`submitted`, `receipt_pending`). A site checked against the wrong shape is a
// mismatch this script invented, which is worse than one it missed.
for (const file of readdirSync(GEN).filter((f) => f.endsWith('.ts'))) {
	const machine = basename(file, '.ts');
	if (onlyMachine && machine !== onlyMachine) continue;
	for (const [name, fields] of variantsOf(readFileSync(join(GEN, file), 'utf8'))) {
		declared.set(name, [...(declared.get(name) ?? []), { machine, fields }]);
	}
}

/** The literal enclosing a `"type": "x"` occurrence, or null if it is not one. */
function enclosingLiteral(text, at) {
	let start = -1;
	let depth = 0;
	for (let i = at; i >= 0; i--) {
		const c = text[i];
		if (c === '}' || c === ']' || c === ')') depth++;
		else if (c === '{' || c === '[' || c === '(') {
			if (depth === 0) {
				start = i;
				break;
			}
			depth--;
		} else if (c === ';' || c === '\n') {
			// Keep scanning: these literals are routinely written over lines.
		}
		if (at - i > 4000) break;
	}
	if (start === -1) return null;
	const open = text[start];
	const close = { '{': '}', '[': ']', '(': ')' }[open];
	let end = start;
	depth = 0;
	for (; end < text.length; end++) {
		if (text[end] === open) depth++;
		else if (text[end] === close && --depth === 0) break;
	}
	if (end >= text.length) return null;
	return text.slice(start, end + 1);
}

const findings = [];
let checked = 0;
let skipped = 0;

for (const shell of SHELLS) {
	for (const file of walk(shell.dir).filter((f) => shell.ext.test(f))) {
		if (/Wire\.(swift|kt)$|CoreViews\.(swift|kt)$|\.test\.|Tests?\.swift$/.test(file)) continue;
		const raw = readFileSync(file, 'utf8');
		if (!raw.includes('"type"')) continue;
		const text = stripComments(raw);
		for (const m of text.matchAll(/"type"\s*[:=]\s*"([a-z0-9_]+)"/g)) {
			const options = declared.get(m[1]);
			if (!options) continue;
			const literal = enclosingLiteral(text, m.index);
			if (literal === null) {
				skipped++;
				continue;
			}
			// Only the keys of THIS literal: a nested payload object belongs to a
			// field, and its keys are that field's business.
			const keys = [];
			let nest = 0;
			// A key follows an opening bracket or a comma. Without that anchor a
			// Swift ternary — `method == "eth_sign" ? "eth_sign" : "personal_sign"`
			// — reads as a key called `eth_sign`, and the script reports a bug
			// that is a colon.
			for (const k of literal.matchAll(
				/[{}[\]]|(?<=[,{[]\s*)"([A-Za-z_][A-Za-z0-9_]*)"\s*[:=]/g
			)) {
				if (k[0] === '{' || k[0] === '[') nest++;
				else if (k[0] === '}' || k[0] === ']') nest--;
				else if (nest === 1 && k[1] !== 'type') keys.push(k[1]);
			}
			if (!keys.length && options.some((o) => !o.fields.length)) {
				checked++;
				continue;
			}
			// A literal that names no key at all is almost always a payload built
			// elsewhere and merged in; counting it as "sends nothing" would bury
			// the real findings in noise.
			if (!keys.length) {
				skipped++;
				continue;
			}
			checked++;
			// Score every declaration; the closest one is the one this site meant.
			const scored = options
				.map((o) => ({
					machine: o.machine,
					fields: o.fields,
					unknown: keys.filter((k) => !o.fields.includes(k)),
					missing: o.fields.filter((f) => !keys.includes(f))
				}))
				.sort((a, b) => a.unknown.length + a.missing.length - (b.unknown.length + b.missing.length));
			const best = scored[0];
			if (best.unknown.length || best.missing.length) {
				findings.push({
					shell: shell.name,
					file: relative(ROOT, file),
					line: text.slice(0, m.index).split('\n').length,
					event: m[1],
					machine: best.machine,
					sends: keys,
					declares: best.fields,
					unknown: best.unknown,
					missing: best.missing
				});
			}
		}
	}
}

if (asJson) {
	console.log(JSON.stringify({ checked, skipped, findings }, null, '\t'));
} else {
	console.log('| Shell | Where | Event | Machine | Sends | Core declares | Verdict |');
	console.log('| --- | --- | --- | --- | --- | --- | --- |');
	for (const f of findings) {
		const verdict = [
			f.unknown.length ? `unknown: ${f.unknown.join(', ')}` : '',
			f.missing.length ? `missing: ${f.missing.join(', ')}` : ''
		]
			.filter(Boolean)
			.join('; ');
		console.log(
			`| ${f.shell} | ${f.file}:${f.line} | \`${f.event}\` | ${f.machine} | ${f.sends.join(', ') || '—'} | ${
				f.declares.join(', ') || '—'
			} | ${verdict} |`
		);
	}
	console.log(
		`\n${findings.length} mismatch${findings.length === 1 ? '' : 'es'} · ${checked} literal dispatch sites checked · ${skipped} skipped (payload not a literal)`
	);
	if (findings.length) {
		const missing = findings.filter((f) => f.missing.length).length;
		const unknownOnly = findings.length - missing;
		console.log(
			`\n${missing} site${missing === 1 ? '' : 's'} omit a declared field: serde rejects the whole value, so the core never sees the event — unless that field carries a serde default, which the generated types do not say. ${unknownOnly} send${unknownOnly === 1 ? 's' : ''} only keys the core does not declare: those are dropped in silence, which means the shell believes it is passing something that never arrives.`
		);
	}
}

process.exit(findings.length ? 1 : 0);
