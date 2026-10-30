#!/usr/bin/env node
/**
 * The iOS wire ruler: operations / results / result fields (native parity
 * F0.3), the sibling of `check-ios-event-parity.mjs`.
 *
 * The event ruler asks "does iOS SAY what the web says". This one asks the
 * other half of the effect loop: for every core machine iOS runs, does its
 * executor ANSWER every operation the core can ask, can it produce every
 * result the core understands, and does each result it produces carry every
 * field the core reads? Measured against the ts-rs mirror of the core's own
 * types (`app-web/vela-wallet/src/lib/core/generated`), which is regenerated
 * from the Rust and so is the wire as it is today.
 *
 * Five columns per machine:
 *
 *   - **unanswered ops** — an operation variant no `case "…"` in the
 *     machine's executor handles. The executor's `default:` answers it, and
 *     that answer is a guess.
 *   - **results never produced** — a result variant no `"type": "…"` in the
 *     executor writes. Informational: some results exist for one shell only.
 *   - **required fields missing** — a field of a result iOS DOES produce, not
 *     optional in the type (`name:` without `?` and without `| null`), that no
 *     dictionary literal producing that result carries. The core cannot
 *     decode that answer unless the Rust says `#[serde(default)]` — a type
 *     the TS mirror cannot show, which is why this is a ruler and not a proof.
 *   - **optional fields missing** — the same for `name?:` / `T | null`
 *     fields: decodable, but the core hears "not said".
 *   - **payload fields missing** — one level down: a result field whose type
 *     is a plain generated struct (`SendRecipientRisk`, …) whose own keys
 *     never appear as a key anywhere in the executor files. `(opt)` when the
 *     payload's field is optional, or the parent is optional and only ever
 *     answered `NSNull()` (those do not fail `--strict`).
 *
 * **Swift builds results by string.** Every executor here answers with a
 * `[String: Any]` literal whose `"type"` is the wire name, so the test is
 * textual: comments stripped, then the literal the `"type": "x"` sits in is
 * read for its keys (plus `dict["key"] = …` assignments within the next
 * lines, the other way this codebase fills an answer).
 *
 *   node app-ios/scripts/check-ios-wire-parity.mjs            # the table, exit 0
 *   node app-ios/scripts/check-ios-wire-parity.mjs --strict   # exit 1 on any
 *        unanswered op or missing required field (what CI gates on)
 *   node app-ios/scripts/check-ios-wire-parity.mjs --json     # machine-readable
 *   node app-ios/scripts/check-ios-wire-parity.mjs --report   # the markdown
 *        committed under app-ios/scripts/reports/
 *
 * The unit tests are `check-ios-wire-parity.test.mjs` (`node --test`).
 */
import { readdirSync, readFileSync, statSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = fileURLToPath(new URL('../..', import.meta.url));
export const GEN = join(ROOT, 'app-web/vela-wallet/src/lib/core/generated');
// `VELA_IOS_SRC`: measure another copy of the sources (a past commit's, for a
// before/after report) against today's generated types.
export const IOS = process.env.VELA_IOS_SRC || join(ROOT, 'app-ios/VelaWallet/VelaWallet');

/**
 * Generated machine prefix → the Swift files that answer its operations.
 * Paths are relative to `app-ios/VelaWallet/VelaWallet`. A machine with `null`
 * is one iOS deliberately does not run (the reason beside it); a machine
 * missing from this table is reported as UNMAPPED, so a new core machine
 * cannot slip past the ruler unnoticed. `{ files, passthrough }` names
 * `result.field` payloads the executor hands over whole from storage (their
 * keys are the stored bytes', not literals in the Swift).
 */
export const MACHINES = {
	// The token rows are written by the chain reads.
	Balance: ['Features/Wallet/BalanceExecutor.swift', 'Core/TokenReads.swift'],
	Batch: ['Features/Send/BatchExecutor.swift'],
	// The history list is handed over as stored (the web's shape, snake_case).
	Bhist: { files: ['Features/Explore/Core/ExploreExecutor.swift'], passthrough: ['loaded.entries'] },
	Clear: ['Features/Signing/Core/ClearExecutor.swift'],
	Contact: ['Features/Contacts/ContactsExecutor.swift'],
	Currency: ['Features/Settings/DisplayCurrencyExecutor.swift'],
	Dbr: ['Features/Explore/Core/DbrExecutor.swift'],
	// `loaded.doc` is the stored document handed over as it was written.
	Explore: { files: ['Features/Explore/Core/ExploreExecutor.swift'], passthrough: ['loaded.doc'] },
	// The bundler quote's fields are written by the relay client.
	Fee: ['Features/Send/FeeExecutor.swift', 'Core/RelayClient.swift'],
	FeeTierPref: ['Features/Settings/FeeTierExecutor.swift'],
	// The records' wire form is `TxRecords.toWire`.
	Feed: ['Features/Wallet/ActivityExecutor.swift', 'Core/TxRecords.swift'],
	Guard: ['Features/Signing/Core/GuardExecutor.swift'],
	// The custom tokens' wire form is `CustomTokens.toWire`.
	Mtok: ['Features/Wallet/ManageTokensExecutor.swift', 'Core/CustomTokens.swift'],
	Net: ['Features/Settings/NetworkAdminExecutor.swift'],
	PaymentRequest: ['Features/Wallet/PaymentRequestExecutor.swift'],
	ReceiveWatch: ['Features/Wallet/ReceiveWatchStore.swift'],
	Rpc: ['Core/RpcPool.swift'],
	Send: ['Features/Send/SendExecutor.swift'],
	// The accounts and the outbox are handed over as `AccountStore` holds them.
	Session: {
		files: ['Features/Onboarding/Core/SessionExecutor.swift'],
		passthrough: ['accounts_loaded.accounts', 'pending_uploads_loaded.records']
	},
	Sign: ['Features/Signing/Core/SignExecutor.swift'],
	SigningPages: ['Features/Settings/SigningPagesExecutor.swift'],
	// A receipt's logs go back as the node wrote them.
	Track: {
		files: ['Features/Send/TrackerExecutor.swift'],
		passthrough: ['receipt_with_logs.logs']
	},
	Trust: ['Features/Wallet/TokenTrustExecutor.swift', 'Core/CustomTokens.swift'],
	// Not iOS machines: the extension's permission store, its dApp sessions
	// (WalletPair / remote inject) and its attestation cache. iOS's dApps run
	// in the in-app browser (`Dbr`).
	Dperm: null,
	Dsess: null,
	ExtCache: null,
	// A payload type whose name ends in `Operation`, not a machine.
	FeedDapp: null,
	// No operations at all (pure machines).
	FeeSpeed: null,
	Intent: null
};

// ---------------------------------------------------------------------------
// TypeScript (ts-rs output) parsing
// ---------------------------------------------------------------------------

export const stripTsComments = (ts) =>
	ts.replace(/\/\*[\s\S]*?\*\//g, '').replace(/^\s*\/\/.*$/gm, '');

/** The text after `export type Name =`, up to its terminating `;` at depth 0. */
export function typeBody(ts) {
	const src = stripTsComments(ts);
	const at = src.search(/export type \w+(<[^>]*>)?\s*=/);
	if (at < 0) return null;
	const start = src.indexOf('=', at) + 1;
	let depth = 0;
	for (let i = start; i < src.length; i++) {
		const c = src[i];
		if ('{[(<'.includes(c)) depth++;
		else if ('}])>'.includes(c)) depth--;
		else if (c === ';' && depth === 0) return src.slice(start, i).trim();
	}
	return src.slice(start).trim();
}

/** Split `s` on `sep` at bracket depth 0. */
function splitTop(s, sep) {
	const out = [];
	let depth = 0;
	let cur = '';
	for (const c of s) {
		if ('{[(<'.includes(c)) depth++;
		else if ('}])>'.includes(c)) depth--;
		if (c === sep && depth === 0) {
			out.push(cur);
			cur = '';
		} else cur += c;
	}
	if (cur.trim()) out.push(cur);
	return out.map((p) => p.trim()).filter(Boolean);
}

/** `{ a: T, b?: U, }` → [{ name, type, optional }] */
export function objectFields(obj) {
	const inner = obj.trim().replace(/^\{/, '').replace(/\}$/, '');
	return splitTop(inner, ',')
		.map((part) => {
			const m = part.match(/^"?([A-Za-z0-9_]+)"?(\??)\s*:\s*([\s\S]+)$/);
			if (!m) return null;
			const type = m[3].trim();
			return {
				name: m[1],
				type,
				optional: m[2] === '?' || /(^|\|)\s*null\s*($|\|)/.test(type)
			};
		})
		.filter(Boolean);
}

/**
 * A tagged union `{ "type": "a", … } | { "type": "b", … }` → its variants:
 * [{ tag, fields }] (the `type` field itself left out). A variant written
 * `{ "type": "a" } & Payload` takes Payload's fields when they are given.
 */
export function unionVariants(body, resolve = () => null) {
	return splitTop(body, '|')
		.map((alt) => {
			const parts = splitTop(alt, '&');
			const obj = parts.find((p) => p.startsWith('{'));
			if (!obj) return null;
			const fields = objectFields(obj);
			const tag = fields.find((f) => f.name === 'type');
			if (!tag) return null;
			let rest = fields.filter((f) => f.name !== 'type');
			for (const p of parts) {
				if (p.startsWith('{')) continue;
				const extra = resolve(p.trim());
				if (extra) rest = rest.concat(extra);
			}
			return { tag: tag.type.replace(/"/g, ''), fields: rest };
		})
		.filter(Boolean);
}

/** A generated type's fields when it is a plain struct (`{ … }`), else null. */
export function structFields(ts) {
	const body = ts && typeBody(ts);
	if (!body || !body.startsWith('{') || splitTop(body, '|').length > 1) return null;
	if (splitTop(body, '&').length > 1) return null;
	return objectFields(body);
}

// ---------------------------------------------------------------------------
// Swift scanning
// ---------------------------------------------------------------------------

export const stripSwiftComments = (swift) =>
	swift.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\/\/.*$/gm, '');

/** Every string a `case "a", "b":` arm of a `switch` names. */
export function handledCases(swift) {
	const out = new Set();
	// `case Self.name:` / `case name:` over `static let name = "x"`.
	const constants = new Map();
	for (const m of swift.matchAll(/(?:static\s+)?let\s+(\w+)\s*=\s*"([a-z0-9_]+)"/g)) constants.set(m[1], m[2]);
	for (const m of swift.matchAll(/case\s+(?:Self\.)?(\w+)\s*:/g)) {
		if (constants.has(m[1])) out.add(constants.get(m[1]));
	}
	for (const m of swift.matchAll(/case\s+((?:"[a-z0-9_]+"\s*,\s*)*"[a-z0-9_]+")\s*(?:,\s*)?:/g)) {
		for (const s of m[1].matchAll(/"([a-z0-9_]+)"/g)) out.add(s[1]);
	}
	return out;
}

/** Index of the `[` that opens the literal containing `pos`, or -1. */
function enclosingBracket(src, pos) {
	let depth = 0;
	for (let i = pos - 1; i >= 0; i--) {
		const c = src[i];
		if (c === ']') depth++;
		else if (c === '[') {
			if (depth === 0) return i;
			depth--;
		}
	}
	return -1;
}

/** Keys written at depth 1 of the literal opened at `open`. */
function literalKeys(src, open) {
	const keys = new Set();
	let depth = 0;
	let inString = false;
	for (let i = open; i < src.length; i++) {
		const c = src[i];
		if (inString) {
			if (c === '\\') i++;
			else if (c === '"') inString = false;
			continue;
		}
		if (c === '"') {
			if (depth === 1) {
				const m = src.slice(i).match(/^"([A-Za-z0-9_]+)"\s*:/);
				if (m) {
					keys.add(m[1]);
					i += m[0].length - 1;
					continue;
				}
			}
			inString = true;
		} else if (c === '[' || c === '(' || c === '{') depth++;
		else if (c === ']' || c === ')' || c === '}') {
			depth--;
			if (depth === 0) break;
		}
	}
	return keys;
}

/**
 * For each result tag the Swift produces, the union of the keys its literals
 * carry. A result counts as produced when `"type": "tag"` appears; its keys
 * are those of the enclosing literal plus any `x["key"] =` within the 30 lines
 * that follow (an answer filled in after it is declared).
 */
export function producedResults(swift) {
	const out = new Map();
	for (const m of swift.matchAll(/"type"\s*:\s*"([a-z0-9_]+)"/g)) {
		const tag = m[1];
		const keys = out.get(tag) ?? new Set();
		const open = enclosingBracket(swift, m.index);
		if (open >= 0) for (const k of literalKeys(swift, open)) keys.add(k);
		const after = swift.slice(m.index).split('\n').slice(0, 30).join('\n');
		for (const a of after.matchAll(/\w+\["([A-Za-z0-9_]+)"\]\s*=/g)) keys.add(a[1]);
		keys.delete('type');
		out.set(tag, keys);
	}
	return out;
}

/**
 * Whether every value the Swift writes for key `name` is `NSNull()` — true
 * too when it never writes the key (the parent's absence is its own column).
 */
export function onlyNull(swift, name) {
	const values = [...swift.matchAll(new RegExp(`"${name}"\\s*:\\s*([^,\\]\\n]+)`, 'g'))].map((m) => m[1].trim());
	return values.every((v) => v.startsWith('NSNull()'));
}

/** Every key the Swift writes anywhere (`"k":` or `["k"] =`). */
export function anyKeys(swift) {
	const out = new Set();
	for (const m of swift.matchAll(/"([A-Za-z0-9_]+)"\s*:/g)) out.add(m[1]);
	for (const m of swift.matchAll(/\["([A-Za-z0-9_]+)"\]\s*=/g)) out.add(m[1]);
	return out;
}

// ---------------------------------------------------------------------------
// The measurement
// ---------------------------------------------------------------------------

/**
 * @param {object} io  { readGen(name) → ts | null, listGen() → names,
 *                       readSwift(rel) → swift | null }
 */
export function measure(io, machines = MACHINES) {
	const names = io.listGen();
	const prefixes = new Set();
	for (const n of names) {
		const m = n.match(/^([A-Za-z]+?)(Operation|ShellResult)\.ts$/);
		if (m) prefixes.add(m[1]);
	}
	const resolveFields = (typeName) => {
		const ts = io.readGen(`${typeName}.ts`);
		return ts ? structFields(ts) : null;
	};
	const rows = [];
	for (const machine of [...prefixes].sort()) {
		const opsTs = io.readGen(`${machine}Operation.ts`);
		const resTs = io.readGen(`${machine}ShellResult.ts`);
		const ops = opsTs ? unionVariants(typeBody(opsTs) ?? '', resolveFields).map((v) => v.tag) : [];
		const results = resTs ? unionVariants(typeBody(resTs) ?? '', resolveFields) : [];
		if (!(machine in machines)) {
			rows.push({ machine, status: 'unmapped', ops: ops.length, results: results.length });
			continue;
		}
		const entry = machines[machine];
		if (entry === null) {
			rows.push({ machine, status: 'not-ios', ops: ops.length, results: results.length });
			continue;
		}
		const files = Array.isArray(entry) ? entry : entry.files;
		const passthrough = new Set(Array.isArray(entry) ? [] : entry.passthrough ?? []);
		const swift = files
			.map((f) => io.readSwift(f))
			.filter((s) => s !== null)
			.map(stripSwiftComments)
			.join('\n');
		const missingFiles = files.filter((f) => io.readSwift(f) === null);
		const cases = handledCases(swift);
		const produced = producedResults(swift);
		const keysAnywhere = anyKeys(swift);
		const unanswered = ops.filter((o) => !cases.has(o));
		const neverProduced = results.filter((r) => !produced.has(r.tag)).map((r) => r.tag);
		const required = [];
		const optional = [];
		const payload = [];
		for (const r of results) {
			const keys = produced.get(r.tag);
			if (!keys) continue;
			for (const f of r.fields) {
				if (!keys.has(f.name)) (f.optional ? optional : required).push(`${r.tag}.${f.name}`);
				if (passthrough.has(`${r.tag}.${f.name}`)) continue;
				// `T`, `T | null` and `Array<T>` all carry T's keys.
				const bare = f.type
					.replace(/\s*\|\s*null$/, '')
					.trim()
					.replace(/^Array<\s*([A-Za-z0-9]+)\s*>$/, '$1');
				if (!/^[A-Z][A-Za-z0-9]*$/.test(bare)) continue;
				const nested = resolveFields(bare);
				if (!nested) continue;
				for (const nf of nested) {
					if (keysAnywhere.has(nf.name)) continue;
					// Decodable either way when the payload field is optional, or
					// when the Swift only ever answers the optional parent with
					// `NSNull()` (it never fills the payload at all).
					const soft = nf.optional || (f.optional && onlyNull(swift, f.name));
					payload.push(`${r.tag}.${f.name}.${nf.name}${soft ? ' (opt)' : ''}`);
				}
			}
		}
		rows.push({
			machine,
			status: missingFiles.length ? 'missing-file' : 'ios',
			files,
			missingFiles,
			ops: ops.length,
			results: results.length,
			unanswered,
			neverProduced,
			required,
			optional,
			payload
		});
	}
	return rows;
}

export function render(rows) {
	const lines = [];
	lines.push(
		'| Machine | Ops | Unanswered ops | Results | Never produced | Required fields missing | Optional fields missing | Payload fields missing |'
	);
	lines.push('| --- | --- | --- | --- | --- | --- | --- | --- |');
	const list = (xs) => (xs && xs.length ? xs.join(', ') : '—');
	for (const r of rows) {
		if (r.status === 'not-ios' || r.status === 'unmapped') {
			const why = r.status === 'not-ios' ? '(not an iOS machine)' : '**UNMAPPED — add it to MACHINES**';
			lines.push(`| ${r.machine} | ${r.ops} | ${why} | ${r.results} | | | | |`);
			continue;
		}
		const name = r.status === 'missing-file' ? `${r.machine} (**missing ${r.missingFiles.join(', ')}**)` : r.machine;
		lines.push(
			`| ${name} | ${r.ops} | ${list(r.unanswered)} | ${r.results} | ${list(r.neverProduced)} | ${list(r.required)} | ${list(r.optional)} | ${list(r.payload)} |`
		);
	}
	const sum = (k) => rows.reduce((n, r) => n + (r[k]?.length ?? 0), 0);
	const unmapped = rows.filter((r) => r.status === 'unmapped' || r.status === 'missing-file').length;
	lines.push('');
	lines.push(
		`unanswered ops: ${sum('unanswered')} · results never produced: ${sum('neverProduced')} · required fields missing: ${sum('required')} · optional fields missing: ${sum('optional')} · payload fields missing: ${sum('payload')} · unmapped machines: ${unmapped}`
	);
	return lines.join('\n');
}

/** What `--strict` fails on: the answers the core cannot get or cannot read. */
export function strictFailures(rows) {
	const out = [];
	for (const r of rows) {
		if (r.status === 'unmapped') out.push(`${r.machine}: unmapped machine`);
		if (r.status === 'missing-file') out.push(`${r.machine}: missing ${r.missingFiles.join(', ')}`);
		for (const o of r.unanswered ?? []) out.push(`${r.machine}: unanswered op ${o}`);
		for (const f of r.required ?? []) out.push(`${r.machine}: required field missing ${f}`);
		for (const f of r.payload ?? []) {
			if (!f.endsWith(' (opt)')) out.push(`${r.machine}: payload field missing ${f}`);
		}
	}
	return out;
}

function diskIO() {
	return {
		listGen: () => readdirSync(GEN),
		readGen: (name) => {
			const p = join(GEN, name);
			return existsSync(p) ? readFileSync(p, 'utf8') : null;
		},
		readSwift: (rel) => {
			const p = join(IOS, rel);
			return existsSync(p) && statSync(p).isFile() ? readFileSync(p, 'utf8') : null;
		}
	};
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
	const args = new Set(process.argv.slice(2));
	const rows = measure(diskIO());
	if (args.has('--json')) console.log(JSON.stringify(rows, null, 2));
	else if (args.has('--report')) {
		// The committed form (reports/): the table, then what --strict says.
		const fails = strictFailures(rows);
		console.log('# iOS wire parity\n');
		console.log('`node app-ios/scripts/check-ios-wire-parity.mjs --report`, measured against');
		console.log('`app-web/vela-wallet/src/lib/core/generated` (the ts-rs mirror of the core).\n');
		console.log(render(rows));
		console.log(`\n## --strict: ${fails.length ? `${fails.length} failure(s)` : 'ok'}\n`);
		for (const f of fails) console.log(`- ${f}`);
	} else console.log(render(rows));
	if (args.has('--strict')) {
		const fails = strictFailures(rows);
		if (fails.length) {
			console.error(`\ncheck-ios-wire-parity --strict: ${fails.length} failure(s)`);
			for (const f of fails) console.error(`  ${f}`);
			process.exit(1);
		}
		console.error('\ncheck-ios-wire-parity --strict: ok');
	}
}
