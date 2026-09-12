#!/usr/bin/env node
/**
 * The "dropped judgement" ruler (spec 047 US5, reference_dropped_judgement_sweep):
 * per core View, the fields the core computes that Android's live builders,
 * controllers and screens never read. A field counts as read when
 * `.field_name` appears in main sources outside the wire files. Fields the
 * Kotlin wire does not even carry are listed separately (a view may be a
 * subset — the drift gate allows it — but a subset is a choice to record).
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, basename } from 'node:path';

const ROOT = new URL('..', import.meta.url).pathname;
const GEN = join(ROOT, 'app-web/vela-wallet/src/lib/core/generated');
const ANDROID = join(ROOT, 'app-android/vela-wallet/app/src/main/java/app/getvela/wallet');
function walk(dir, out = []) {
	for (const name of readdirSync(dir)) {
		const p = join(dir, name);
		if (statSync(p).isDirectory()) walk(p, out); else out.push(p);
	}
	return out;
}
const files = walk(ANDROID).filter((f) => f.endsWith('.kt'));
const wire = files.filter((f) => /Wire\.kt$|CoreViews\.kt$/.test(f)).map((f) => readFileSync(f, 'utf8')).join('\n');
const code = files.filter((f) => !/Wire\.kt$|CoreViews\.kt$/.test(f)).map((f) => readFileSync(f, 'utf8')).join('\n');
const views = readdirSync(GEN).filter((f) => /View\.ts$/.test(f) && !/Row|Entry|Item|Step|Leg|Spec|Report|Identity|Section|Field|Result/.test(f));
console.log('| View | Fields | Not on the Kotlin wire | Carried but never read |');
console.log('| --- | --- | --- | --- |');
let dropped = 0;
for (const file of views) {
	const name = basename(file, '.ts');
	const ts = readFileSync(join(GEN, file), 'utf8');
	const body = ts.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\/\/.*$/gm, '');
	const fields = [...body.matchAll(/^\s*([a-z][a-z0-9_]*)\s*:/gm)].map((m) => m[1]);
	if (fields.length === 0) continue;
	const missing = fields.filter((f) => !new RegExp(`val ${f}\\b`).test(wire));
	const unread = fields.filter((f) => !missing.includes(f) && !new RegExp(`\\.${f}\\b`).test(code));
	dropped += unread.length;
	console.log(`| ${name} | ${fields.length} | ${missing.join(', ') || '—'} | ${unread.join(', ') || '—'} |`);
}
console.log(`\ncarried but never read: ${dropped} fields across ${views.length} views`);
