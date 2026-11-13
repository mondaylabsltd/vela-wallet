// The page's catalogue of networks and tokens, generated from vela-core's own
// tables (spec 102 P3).
//
//   bun samples/gen-catalog.mjs           → writes src/lib/catalog.js
//   bun samples/gen-catalog.mjs --check   → fails if src/lib/catalog.js is not
//                                           what the core's tables give today
//
// Why the page carries this at all. The page names a chain and reads a token
// amount from facts it holds itself, never from the request (PROTOCOL.md §9):
// a requester that could name the token could make 250 USDC read as 2.5. Until
// 102 those facts were a hand-written table of mainnet tokens, so a Base or
// Arbitrum USDC transfer — most of what Vela signs — was an unknown token,
// read with a guessed 18 decimals and shown as "0 ?". The apps already know
// every token they offer and every network they ship; this copies the same
// rows, so the page and the wallet say the same thing about the same bytes.
//
// Sources (read, never edited):
//   rust/crates/vela-core/src/app/network_admin.rs        BUILTIN_CHAINS
//   rust/crates/vela-core/src/app/token_registry/table.rs TABLE
//   rust/crates/vela-core/src/app/manage_tokens.rs        native_alias_token
//
// Zero dependencies, like the page: `node:fs`, `node:path`, `node:url`.

import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, '..');
const CORE = resolve(ROOT, '../../rust/crates/vela-core/src');
const OUT = join(ROOT, 'src/lib/catalog.js');

/** `BUILTIN_CHAINS`: `[chainId, displayName, nativeSymbol]`, in the core's order. */
export function coreChains() {
	const source = readFileSync(join(CORE, 'app/network_admin.rs'), 'utf8');
	const block = source.slice(source.indexOf('pub const BUILTIN_CHAINS'));
	const body = block.slice(0, block.indexOf('\n];'));
	const chains = [];
	for (const entry of body.split('NetBuiltinChain {').slice(1)) {
		const name = /display_name:\s*"([^"]+)"/.exec(entry);
		const id = /chain_id:\s*(\d+)/.exec(entry);
		const native = /native_symbol:\s*"([^"]+)"/.exec(entry);
		if (!name || !id || !native) throw new Error('a BUILTIN_CHAINS entry changed shape:\n' + entry);
		chains.push([Number(id[1]), name[1], native[1]]);
	}
	const declared = /BUILTIN_CHAINS:\s*\[NetBuiltinChain;\s*(\d+)\]/.exec(source);
	if (!declared || Number(declared[1]) !== chains.length) {
		throw new Error(`read ${chains.length} built-in chains, the core declares ${declared && declared[1]}`);
	}
	return chains;
}

/** `TABLE`: `[chainId, address, symbol, decimals]`, sorted as the core sorts it. */
export function coreTokens() {
	const source = readFileSync(join(CORE, 'app/token_registry/table.rs'), 'utf8');
	const rows = [...source.matchAll(/^\s*\((\d+),\s*"(0x[0-9a-f]{40})",\s*"([^"]+)",\s*(\d+)\),\s*$/gm)]
		.map((m) => [Number(m[1]), m[2], m[3], Number(m[4])]);
	const declared = /TABLE:\s*\[\(u32, &str, &str, u32\);\s*(\d+)\]/.exec(source);
	if (!declared || Number(declared[1]) !== rows.length) {
		throw new Error(`read ${rows.length} tokens, the core declares ${declared && declared[1]}`);
	}
	return rows;
}

/**
 * The ERC-20 interface onto a chain's NATIVE coin, where it has one
 * (`native_alias_token`): Arc's USDC at `0x3600…0000`, Stable's USDT0. The
 * wallet reads a transfer through it as the chain's own coin ("−0.001 USDC");
 * without these rows the page drew the same bytes as "1,000 ? ?" (round 7,
 * r7-4). Both interfaces are 6-decimal views of an 18-decimal native balance
 * (the core's doc comment: specs 060 and 061), so the row is the chain's
 * native symbol at 6 decimals — the generator refuses a doc comment that
 * stops saying so.
 */
export function coreNativeAliases(chains) {
	const source = readFileSync(join(CORE, 'app/manage_tokens.rs'), 'utf8');
	const start = source.indexOf('pub fn native_alias_token');
	if (start < 0) throw new Error('native_alias_token is gone from manage_tokens.rs');
	const doc = source.slice(source.lastIndexOf('/// The ERC-20 interface onto', start), start).replace(/\s*\/\/\/\s*/g, ' ');
	const body = source.slice(start, source.indexOf('\n}', start));
	if (!/6-decimal ERC-20 view/.test(doc) || !/6-dp ERC-20/.test(body)) {
		throw new Error('native_alias_token no longer says its interfaces are 6-decimal: read it again');
	}
	const rows = [];
	for (const arm of body.matchAll(/^\s*([\d_| ]+)\s*=>\s*Some\("(0x[0-9a-fA-F]{40})"\)/gm)) {
		for (const id of arm[1].split('|').map((n) => Number(n.trim().replace(/_/g, '')))) {
			const chain = chains.find((c) => c[0] === id);
			if (chain) rows.push([id, arm[2].toLowerCase(), chain[2], 6]);
		}
	}
	if (!rows.length) throw new Error('read no native aliases from manage_tokens.rs');
	return rows.sort((a, b) => a[0] - b[0]);
}

const js = (value) => JSON.stringify(value).replace(/"/g, "'");

export function render() {
	const chains = coreChains();
	const tokens = coreTokens();
	const aliases = coreNativeAliases(chains);
	const lines = [
		'// @generated by samples/gen-catalog.mjs from vela-core — do not edit.',
		'//',
		`// ${chains.length} built-in networks (app/network_admin.rs BUILTIN_CHAINS) and`,
		`// ${tokens.length} tokens (app/token_registry/table.rs): the networks Vela ships and the`,
		'// tokens it offers on them, exactly as the apps name them, then the',
		`// ${aliases.length} ERC-20 interfaces onto a chain's own coin (app/manage_tokens.rs`,
		'// native_alias_token). lib/registry.js',
		'// reads these rows; nothing in a request can add to them or change them.',
		'// Regenerate with `bun samples/gen-catalog.mjs`.',
		'window.VelaCS = window.VelaCS || {};',
		'window.VelaCS.catalog = {',
		'  // [chain id, name, native coin]',
		'  chains: [',
		...chains.map((row) => `    ${js(row)},`),
		'  ],',
		'  // [chain id, lowercased address, symbol, decimals]',
		'  tokens: [',
		...tokens.map((row) => `    ${js(row)},`),
		"    // the chain's own coin behind an ERC-20 interface: its symbol, 6 decimals",
		...aliases.map((row) => `    ${js(row)},`),
		'  ],',
		'};',
		'',
	];
	return lines.join('\n');
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url))) {
	const text = render();
	if (process.argv.includes('--check')) {
		if (!existsSync(OUT) || readFileSync(OUT, 'utf8') !== text) {
			console.error('gen-catalog --check: src/lib/catalog.js is not what the core\'s tables give.\n' +
				'  run: bun samples/gen-catalog.mjs');
			process.exit(1);
		}
		console.log('gen-catalog --check: src/lib/catalog.js matches vela-core');
	} else {
		writeFileSync(OUT, text);
		console.log(`gen-catalog: wrote src/lib/catalog.js`);
	}
}
