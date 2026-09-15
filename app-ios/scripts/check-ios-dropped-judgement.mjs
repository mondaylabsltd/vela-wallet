#!/usr/bin/env node
/**
 * The "dropped judgement" ruler for iOS (spec 056 US5).
 *
 * Per core View: the fields the core COMPUTES that no live builder, store or
 * screen ever reads. The difference between this and the drift gate is the
 * whole point — drift catches a field that changed SHAPE, and this catches a
 * field the core worked out and the screen never said.
 *
 * A field counts as read when `.fieldName` appears in a Swift source outside
 * the wire files. Swift's wires are camelCase (`CoreJSON` decodes with
 * `.convertFromSnakeCase`), so the core's `snake_case` name is converted
 * before it is looked for.
 *
 * Fields the Swift wire does not carry at all are listed separately: a view
 * may legitimately be a subset — the drift gate allows it — but a subset is a
 * choice, and results.md gives each one a reason.
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, basename } from 'node:path';

const ROOT = new URL('../..', import.meta.url).pathname;
const GEN = join(ROOT, 'app-web/vela-wallet/src/lib/core/generated');
const IOS = join(ROOT, 'app-ios/VelaWallet/VelaWallet');

function walk(dir, out = []) {
	for (const name of readdirSync(dir)) {
		const p = join(dir, name);
		if (statSync(p).isDirectory()) walk(p, out);
		else out.push(p);
	}
	return out;
}

const camel = (snake) => snake.replace(/_([a-z0-9])/g, (_, c) => c.toUpperCase());

const swift = walk(IOS).filter((f) => f.endsWith('.swift'));
const isWire = (f) => /Wire\.swift$|CoreViews\.swift$/.test(f);
const wire = swift.filter(isWire).map((f) => readFileSync(f, 'utf8')).join('\n');
const code = swift
	.filter((f) => !isWire(f))
	.map((f) =>
		readFileSync(f, 'utf8')
			.replace(/\/\*[\s\S]*?\*\//g, '')
			.replace(/^\s*\/\/.*$/gm, '')
	)
	.join('\n');

// Row / entry / item types are the CONTENTS of a view, not a view — their
// fields are read through the collection that holds them.
const views = readdirSync(GEN).filter(
	(f) =>
		/View\.ts$/.test(f) &&
		!/Row|Entry|Item|Step|Leg|Spec|Report|Identity|Section|Field|Result/.test(f)
);

console.log('| View | Fields | Not on the Swift wire | Carried but never read |');
console.log('| --- | --- | --- | --- |');
let dropped = 0;
let missingTotal = 0;
for (const file of views) {
	const name = basename(file, '.ts');
	const ts = readFileSync(join(GEN, file), 'utf8');
	const body = ts.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\/\/.*$/gm, '');
	const fields = [...body.matchAll(/^\s*([a-z][a-z0-9_]*)\s*:/gm)].map((m) => m[1]);
	if (fields.length === 0) continue;
	const missing = fields.filter((f) => !new RegExp(`\\b(let|var) ${camel(f)}\\b`).test(wire));
	const unread = fields.filter(
		(f) => !missing.includes(f) && !new RegExp(`\\.${camel(f)}\\b`).test(code)
	);
	dropped += unread.length;
	missingTotal += missing.length;
	console.log(
		`| ${name} | ${fields.length} | ${missing.map(camel).join(', ') || '—'} | ${unread.map(camel).join(', ') || '—'} |`
	);
}
console.log(
	`\nnot on the wire: ${missingTotal} · carried but never read: ${dropped} — across ${views.length} views`
);
