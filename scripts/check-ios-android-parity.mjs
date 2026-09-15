#!/usr/bin/env node
/**
 * The "Android as the checklist" ruler (spec 058).
 *
 * 040–049 wired Android against web; 050–057 wired iOS against web. Nobody has
 * ever read the two phones against EACH OTHER, and the founder's question is
 * exactly that one: does the iPhone do what the Android does, in send, receive,
 * the browser and settings?
 *
 * Per core machine, two columns: the Event variants ANDROID dispatches and the
 * ones iOS dispatches. `Android-only` is the gap this spec exists to close;
 * `iOS-only` is the reverse debt, which is real too — a control the iPhone has
 * and the phone in the founder's other hand does not.
 *
 * Detection is each client's own idiom, borrowed from the two existing rulers:
 *   Android  the `@SerialName("x")` sealed-class name, used outside Wire files
 *   iOS      the quoted wire name in a Swift source outside Wire files,
 *            comments stripped
 *
 * Exit code stays 0. The table is the deliverable; what each row MEANS is a
 * judgement and results.md names it.
 *
 * Usage: node scripts/check-ios-android-parity.mjs [--machine SendEvent]
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, basename } from 'node:path';

const ROOT = new URL('..', import.meta.url).pathname;
const GEN = join(ROOT, 'app-web/vela-wallet/src/lib/core/generated');
const IOS = join(ROOT, 'app-ios/VelaWallet/VelaWallet');
const ANDROID = join(ROOT, 'app-android/vela-wallet/app/src/main/java/app/getvela/wallet');

const only = process.argv.includes('--machine')
	? process.argv[process.argv.indexOf('--machine') + 1]
	: null;

function walk(dir, out = []) {
	for (const name of readdirSync(dir)) {
		const p = join(dir, name);
		if (statSync(p).isDirectory()) {
			if (!/node_modules|generated|__tests__|\/build\//.test(p)) walk(p, out);
		} else out.push(p);
	}
	return out;
}
const stripComments = (s) =>
	s.replace(/\/\*[\s\S]*?\*\//g, '').replace(/^\s*\/\/.*$/gm, '');

const swift = walk(IOS).filter((f) => f.endsWith('.swift'));
const isWireSwift = (f) => /Wire\.swift$|CoreViews\.swift$/.test(f);
const iosText = swift
	.filter((f) => !isWireSwift(f))
	.map((f) => stripComments(readFileSync(f, 'utf8')))
	.join('\n');

const kotlin = walk(ANDROID).filter((f) => f.endsWith('.kt'));
const isWireKt = (f) => /Wire\.kt$|CoreViews\.kt$/.test(f);
const wireText = kotlin
	.filter(isWireKt)
	.map((f) => readFileSync(f, 'utf8'))
	.join('\n');
const androidText = kotlin
	.filter((f) => !isWireKt(f))
	.map((f) => stripComments(readFileSync(f, 'utf8')))
	.join('\n');

const rows = [];
for (const file of readdirSync(GEN).filter((f) => /Event\.ts$/.test(f))) {
	const machine = basename(file, '.ts');
	if (only && machine !== only) continue;
	const ts = readFileSync(join(GEN, file), 'utf8');
	const variants = [...ts.matchAll(/"type":\s*"([a-z0-9_]+)"/g)].map((m) => m[1]);
	if (variants.length === 0) continue;
	const iosUses = variants.filter((v) => new RegExp(`"${v}"`).test(iosText));
	const androidUses = variants.filter((v) => {
		const cls = [
			...wireText.matchAll(
				new RegExp(`@SerialName\\("${v}"\\)\\s*\\n\\s*(?:data class|data object|object)\\s+(\\w+)`, 'g')
			)
		].map((m) => m[1]);
		const byClass = cls.some((c) => new RegExp(`\\b${c}\\b`).test(androidText));
		const byString = new RegExp(`event\\("${v}"\\)|"${v}"\\s*\\)`).test(androidText);
		return byClass || byString;
	});
	const androidOnly = androidUses.filter((v) => !iosUses.includes(v));
	const iosOnly = iosUses.filter((v) => !androidUses.includes(v));
	rows.push({ machine, total: variants.length, android: androidUses.length, ios: iosUses.length, androidOnly, iosOnly });
}
rows.sort(
	(a, b) => b.androidOnly.length - a.androidOnly.length || a.machine.localeCompare(b.machine)
);
console.log('| Machine | Variants | Android dispatches | iOS dispatches | Android-only | iOS-only |');
console.log('| --- | --- | --- | --- | --- | --- |');
for (const r of rows) {
	console.log(
		`| ${r.machine} | ${r.total} | ${r.android} | ${r.ios} | ${r.androidOnly.join(', ') || '—'} | ${r.iosOnly.join(', ') || '—'} |`
	);
}
console.log(
	`\nAndroid-only: ${rows.reduce((n, r) => n + r.androidOnly.length, 0)} · iOS-only: ${rows.reduce((n, r) => n + r.iosOnly.length, 0)} · across ${rows.length} machines`
);
