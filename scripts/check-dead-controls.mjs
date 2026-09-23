#!/usr/bin/env node
/**
 * Does any shipping control have a handler nobody ever passes?
 *
 * A SwiftUI view declares its actions as properties with a no-op default:
 *
 *   struct SettingsNetworkRow: View {
 *       var onTap: (String) -> Void = { _ in }   // ← the default
 *       …
 *       .contentShape(Rectangle())
 *       .onTapGesture { onTap(row.id) }          // ← the hit area
 *   }
 *
 * The default exists so a gallery can draw the row as a picture. It also
 * means a call site that forgets `onTap:` compiles, renders, has a hit area,
 * and does nothing at all — no warning anywhere. A device run found exactly
 * that on the Add Network search results, and a sweep of all four shells for
 * the same shape found twenty more.
 *
 * This checks the half a script can check with certainty: **an action that is
 * passed at NO live call site.** If a view ships (it is built somewhere
 * outside a gallery, a fixture file or a `#Preview`) and one of its actions is
 * never handed a handler by any of those live sites, then that control is
 * drawn and inert everywhere a person can reach it.
 *
 * ## What it does NOT catch, including the bug that prompted it
 *
 * Be clear about the reach, because a green gate that is believed to mean
 * more than it does is worse than no gate.
 *
 * - **It would not have caught the Add Network search row.** `onTap` IS
 *   passed — by the Networks list two hundred lines above. One site forgot
 *   it; the handler is not unused. Deciding whether a PARTICULAR site should
 *   pass one needs a person: `ContactRow.onSend` is rightly omitted on a
 *   screen with no swipe actions, and a script that flagged that would cry
 *   wolf until nobody read it. The sweep's own numbers: 116 (view, call-site)
 *   pairs omit something, 6 were bugs.
 * - **It does not follow optionals through a host.** `SendFormBody.onDenom`
 *   is passed by `FlowHost`, which passes its own `onDenom?` — and THAT was
 *   the nil one. The chain is one hop too long for a regex.
 *
 * What it does catch, verified by re-creating both: `ConnectionPanelView`'s
 * "Switch account" (#9) and the add-token sheet's network row (#10).
 *
 * Exit 1 on any finding. Usage:
 *   node scripts/check-dead-controls.mjs [--json]
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';

const ROOT = new URL('..', import.meta.url).pathname;
const IOS = join(ROOT, 'app-ios/VelaWallet/VelaWallet');
const asJson = process.argv.includes('--json');

/** A file that exists to DRAW, not to ship: its call sites prove nothing. */
const isDrawing = (path) => /Fixtures|Gallery|Preview|Mock/i.test(path);

/**
 * Known, with a reason. Every entry is a claim somebody checked by reading,
 * and it says what would have to change for the entry to go.
 *
 * This list is meant to stay short. An entry that cannot say why is a bug
 * being filed under "known".
 */
const ALLOWED = new Map([
	[
		'ContactRow.onSend',
		'The control is a SWIPE action, and it is only drawn when the row carries a `swipe` model — which `ContactsLive` never sets. Nothing inert reaches the screen. Remove this entry when contacts grow swipe actions.'
	]
]);

function walk(dir, out = []) {
	for (const name of readdirSync(dir)) {
		const p = join(dir, name);
		if (statSync(p).isDirectory()) walk(p, out);
		else if (p.endsWith('.swift')) out.push(p);
	}
	return out;
}

/** Blank comments, keeping offsets, so a commented example is not a call site. */
const stripComments = (s) =>
	s
		.replace(/\/\*[\s\S]*?\*\//g, (m) => m.replace(/[^\n]/g, ' '))
		.replace(/^\s*\/\/.*$/gm, '')
		.replace(/\/\/\/.*$/gm, '');

const files = walk(IOS);
const source = new Map(files.map((f) => [f, stripComments(readFileSync(f, 'utf8'))]));

// --- What each View declares -------------------------------------------------

/** view name → { file, actions: [name], body } */
const views = new Map();
const structRe = /(?:private\s+)?struct\s+(\w+)\s*:\s*View\s*\{/g;
const actionRe = /\n\s*var\s+(on[A-Z]\w*)\s*:\s*\([^)]*\)\s*->\s*Void\s*=\s*\{[^}]*\}/g;

for (const [file, text] of source) {
	for (const m of text.matchAll(structRe)) {
		// Balance braces from the struct's opening one.
		let depth = 0;
		let end = m.index + m[0].length - 1;
		for (; end < text.length; end++) {
			if (text[end] === '{') depth++;
			else if (text[end] === '}' && --depth === 0) break;
		}
		const body = text.slice(m.index, end + 1);
		const actions = [...body.matchAll(actionRe)].map((a) => a[1]);
		if (actions.length) views.set(m[1], { file, actions, body });
	}
}

// --- Where each View is built ------------------------------------------------

/** The argument list of one `Name(` call, plus whether a trailing closure follows. */
function callsOf(text, name) {
	const out = [];
	for (const m of text.matchAll(new RegExp(`\\b${name}\\s*\\(`, 'g'))) {
		let depth = 0;
		let end = m.index + m[0].length - 1;
		for (; end < text.length; end++) {
			if (text[end] === '(') depth++;
			else if (text[end] === ')' && --depth === 0) break;
		}
		if (end >= text.length) continue;
		const after = text.slice(end + 1, end + 40);
		out.push({
			at: m.index,
			args: text.slice(m.index, end + 1),
			trailing: /^\s*\{/.test(after)
		});
	}
	return out;
}

const findings = [];
const allowed = [];
let checkedViews = 0;
let drawingOnly = 0;

for (const [name, view] of views) {
	// Every construction of this view, anywhere.
	const live = [];
	let anywhere = 0;
	for (const [file, text] of source) {
		for (const call of callsOf(text, name)) {
			// Its own declaration is not a call site.
			if (file === view.file && view.body.includes(call.args.slice(0, 40))) continue;
			anywhere++;
			if (!isDrawing(file) && !inPreview(text, call.at)) live.push({ file, call });
		}
	}
	if (!live.length) {
		if (anywhere) drawingOnly++;
		continue;
	}
	checkedViews++;
	for (const action of view.actions) {
		// A control nobody draws cannot be dead ON SCREEN; require a hit area
		// or a Button built from this action.
		if (!drawsSomething(view.body, action)) continue;
		const passed = live.some(
			({ call }) => call.args.includes(`${action}:`) || (call.trailing && isLastAction(view, action))
		);
		if (!passed) {
			if (ALLOWED.has(`${name}.${action}`)) {
				allowed.push(`${name}.${action}`);
				continue;
			}
			findings.push({
				view: name,
				action,
				declaredIn: relative(ROOT, view.file),
				liveSites: live.map(({ file }) => relative(ROOT, file))
			});
		}
	}
}

/** Is `action` referenced by something that takes a tap? */
function drawsSomething(body, action) {
	return (
		new RegExp(`Button\\s*\\(\\s*action:\\s*${action}\\b`).test(body) ||
		new RegExp(`onTapGesture\\s*\\{[^}]*\\b${action}\\b`).test(body) ||
		new RegExp(`Button\\s*\\{[^}]*\\b${action}\\b`).test(body) ||
		new RegExp(`onSelect:\\s*${action}\\b`).test(body) ||
		new RegExp(`action:\\s*${action}\\b`).test(body)
	);
}

/** Swift's trailing-closure sugar fills the LAST closure parameter. */
function isLastAction(view, action) {
	return view.actions[view.actions.length - 1] === action;
}

/** Is this offset inside a `#Preview { … }` block? */
function inPreview(text, at) {
	const before = text.lastIndexOf('#Preview', at);
	if (before === -1) return false;
	let depth = 0;
	for (let i = before; i < text.length && i <= at + 1; i++) {
		if (text[i] === '{') depth++;
		else if (text[i] === '}') depth--;
		if (depth === 0 && i > before && text[i] === '}') return false;
	}
	return depth > 0;
}

if (asJson) {
	console.log(JSON.stringify({ checkedViews, drawingOnly, allowed, findings }, null, '\t'));
} else {
	console.log('| View | Action | Declared in | Built live in |');
	console.log('| --- | --- | --- | --- |');
	for (const f of findings) {
		console.log(
			`| ${f.view} | \`${f.action}\` | ${f.declaredIn} | ${[...new Set(f.liveSites)].join(', ')} |`
		);
	}
	console.log(
		`\n${findings.length} control${findings.length === 1 ? '' : 's'} drawn with a handler no live call site passes · ${checkedViews} shipping views checked · ${drawingOnly} built only by galleries and previews · ${allowed.length} known (${allowed.join(', ') || 'none'})`
	);
	if (findings.length) {
		console.log(
			'\nEach of these is drawn, has a hit area, and calls its no-op default wherever a person can reach it. Either pass the handler, or stop drawing the control as something that acts.'
		);
	}
}

process.exit(findings.length ? 1 : 0);
