#!/usr/bin/env node
/**
 * The "web as the checklist" ruler (spec 047 US5, reference_web_as_checklist):
 * per core machine, the Event variants the WEB dispatches minus the ones
 * ANDROID dispatches. A wire name counts as dispatched on Android when it
 * appears in main sources outside the wire files — as the `@SerialName`
 * class it maps to (`SendEvent.Open(`) or as the raw string the onboarding
 * machines use (`event("boot")`). Strong diffs are printed; the exit code
 * stays 0 — the table is the deliverable, results.md names the residue.
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, basename } from 'node:path';

const ROOT = new URL('..', import.meta.url).pathname;
const GEN = join(ROOT, 'app-web/vela-wallet/src/lib/core/generated');
const WEB = join(ROOT, 'app-web/vela-wallet/src');
const ANDROID = join(ROOT, 'app-android/vela-wallet/app/src/main/java/app/getvela/wallet');

function walk(dir, out = []) {
	for (const name of readdirSync(dir)) {
		const p = join(dir, name);
		if (statSync(p).isDirectory()) { if (!/node_modules|generated|__tests__/.test(p)) walk(p, out); }
		else out.push(p);
	}
	return out;
}
const webFiles = walk(WEB).filter((f) => /\.(ts|svelte)$/.test(f) && !/\.test\./.test(f));
const webText = webFiles.map((f) => readFileSync(f, 'utf8')).join('\n');
const androidFiles = walk(ANDROID).filter((f) => f.endsWith('.kt'));
const wireFiles = androidFiles.filter((f) => /Wire\.kt$|CoreViews\.kt$/.test(f));
const androidText = androidFiles.filter((f) => !wireFiles.includes(f)).map((f) => readFileSync(f, 'utf8')).join('\n');
const wireText = wireFiles.map((f) => readFileSync(f, 'utf8')).join('\n');

const rows = [];
for (const file of readdirSync(GEN).filter((f) => /Event\.ts$/.test(f))) {
	const machine = basename(file, '.ts');
	const ts = readFileSync(join(GEN, file), 'utf8');
	const variants = [...ts.matchAll(/"type":\s*"([a-z0-9_]+)"/g)].map((m) => m[1]);
	if (variants.length === 0) continue;
	const webUses = variants.filter((v) => new RegExp(`type:\\s*['"]${v}['"]`).test(webText));
	const androidUses = variants.filter((v) => {
		const cls = [...wireText.matchAll(new RegExp(`@SerialName\\("${v}"\\)\\s*\\n\\s*(?:data class|data object|object)\\s+(\\w+)`, 'g'))].map((m) => m[1]);
		const byClass = cls.some((c) => new RegExp(`\\b${c}\\b`).test(androidText));
		const byString = new RegExp(`event\\("${v}"\\)|"${v}"\\s*\\)`).test(androidText);
		return byClass || byString;
	});
	const strong = webUses.filter((v) => !androidUses.includes(v));
	const androidOnly = androidUses.filter((v) => !webUses.includes(v));
	rows.push({ machine, total: variants.length, web: webUses.length, android: androidUses.length, strong, androidOnly });
}
rows.sort((a, b) => b.strong.length - a.strong.length || a.machine.localeCompare(b.machine));
console.log('| Machine | Variants | Web dispatches | Android dispatches | Web-only (strong) | Android-only |');
console.log('| --- | --- | --- | --- | --- | --- |');
for (const r of rows) console.log(`| ${r.machine} | ${r.total} | ${r.web} | ${r.android} | ${r.strong.join(', ') || '—'} | ${r.androidOnly.join(', ') || '—'} |`);
console.log(`\nstrong diffs: ${rows.reduce((n, r) => n + r.strong.length, 0)} across ${rows.length} machines`);
