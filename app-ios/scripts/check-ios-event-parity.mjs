#!/usr/bin/env node
/**
 * The "web as the checklist" ruler for iOS (spec 056 US5).
 *
 * Per core machine: the Event variants the WEB dispatches, minus the ones iOS
 * dispatches. The table is the deliverable and the exit code stays 0 — what
 * the residue MEANS is a judgement, and results.md names each one.
 *
 * **Swift dispatches by string.** Every store here builds a dictionary whose
 * `"type"` is the variant's wire name (`["type": "open_scanner"]`), so a
 * variant counts as dispatched when that quoted name appears in a Swift source
 * outside the wire files. That is a looser test than Android's sealed-class
 * check and it is the honest one for this client: there is no class to look
 * for, and a name in a comment is a false positive the reader can see.
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, basename } from 'node:path';

const ROOT = new URL('../..', import.meta.url).pathname;
const GEN = join(ROOT, 'app-web/vela-wallet/src/lib/core/generated');
const WEB = join(ROOT, 'app-web/vela-wallet/src');
const IOS = join(ROOT, 'app-ios/VelaWallet/VelaWallet');

function walk(dir, out = []) {
	for (const name of readdirSync(dir)) {
		const p = join(dir, name);
		if (statSync(p).isDirectory()) {
			if (!/node_modules|generated|__tests__/.test(p)) walk(p, out);
		} else out.push(p);
	}
	return out;
}

const webText = walk(WEB)
	.filter((f) => /\.(ts|svelte)$/.test(f) && !/\.test\./.test(f))
	.map((f) => readFileSync(f, 'utf8'))
	.join('\n');

const swift = walk(IOS).filter((f) => f.endsWith('.swift'));
// The wire files MIRROR the core's vocabulary; finding a name there proves
// nothing about whether anything sends it.
const isWire = (f) => /Wire\.swift$|CoreViews\.swift$/.test(f);
const iosText = swift
	.filter((f) => !isWire(f))
	.map((f) =>
		readFileSync(f, 'utf8')
			// Comments are where this codebase's own prose names events, at
			// length. Stripping them is what keeps a paragraph ABOUT
			// `scan_resolved` from counting as a dispatch OF it.
			.replace(/\/\*[\s\S]*?\*\//g, '')
			.replace(/^\s*\/\/.*$/gm, '')
	)
	.join('\n');

const rows = [];
for (const file of readdirSync(GEN).filter((f) => /Event\.ts$/.test(f))) {
	const machine = basename(file, '.ts');
	const ts = readFileSync(join(GEN, file), 'utf8');
	const variants = [...ts.matchAll(/"type":\s*"([a-z0-9_]+)"/g)].map((m) => m[1]);
	if (variants.length === 0) continue;
	const webUses = variants.filter((v) => new RegExp(`type:\\s*['"]${v}['"]`).test(webText));
	const iosUses = variants.filter((v) =>
		new RegExp(`"type"\\s*:\\s*"${v}"|"${v}"`).test(iosText)
	);
	const strong = webUses.filter((v) => !iosUses.includes(v));
	const iosOnly = iosUses.filter((v) => !webUses.includes(v));
	rows.push({ machine, total: variants.length, web: webUses.length, ios: iosUses.length, strong, iosOnly });
}
rows.sort((a, b) => b.strong.length - a.strong.length || a.machine.localeCompare(b.machine));
console.log('| Machine | Variants | Web dispatches | iOS dispatches | Web-only (strong) | iOS-only |');
console.log('| --- | --- | --- | --- | --- | --- |');
for (const r of rows) {
	console.log(`| ${r.machine} | ${r.total} | ${r.web} | ${r.ios} | ${r.strong.join(', ') || '—'} | ${r.iosOnly.join(', ') || '—'} |`);
}
console.log(`\nstrong diffs: ${rows.reduce((n, r) => n + r.strong.length, 0)} across ${rows.length} machines`);
