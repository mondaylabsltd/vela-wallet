#!/usr/bin/env node
/**
 * Generate the core's token registry: one chain-scoped table of the tokens
 * the wallet names by itself (spec 097 part D).
 *
 *   node scripts/gen-token-registry.mjs [--check]
 *
 * Sources, all read at generation time and never at runtime:
 *
 * 1. The chain-data service every shell's asset list uses
 *    (`https://ethereum-data.getvela.app/chains/eip155-<id>.json`, read by
 *    `app-web/vela-wallet/src/lib/services/chain-tokens.ts` and its shell
 *    twins), for each built-in network in `network_admin.rs BUILTIN_CHAINS`:
 *    its `stables[]` (the service's symbol) and its `wrappedNativeToken`
 *    (the contract's own `symbol()`).
 * 2. The well-known tokens the wallet carried before 097 D (`CURATED` below),
 *    each pinned to the one chain it lives on.
 *
 * Every row's decimals are the contract's own `decimals()`, read over the
 * chain's RPCs. A chain-data row whose decimals cannot be read is left out
 * (Plume's `0x4200…0006` answers nothing); a curated row whose decimals the
 * chain contradicts stops the run.
 *
 * Output: `rust/crates/vela-core/src/app/token_registry/table.rs`, committed.
 * The committed table is what the core builds and tests against; refreshing
 * it is a reviewable commit, not a runtime call. `--check` regenerates in
 * memory and fails when the committed table differs (it needs the network).
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const NETWORKS = join(root, 'rust/crates/vela-core/src/app/network_admin.rs');
const TARGET = join(root, 'rust/crates/vela-core/src/app/token_registry/table.rs');
const CHAIN_DATA = 'https://ethereum-data.getvela.app/chains';

/**
 * The well-known tokens of the old `KNOWN_TOKENS` tables (clear_signing.rs,
 * token_trust.rs, web `services/tokens.ts`), pinned to their real chain.
 * `[chainId, address, symbol, decimals]`; the decimals are checked on-chain.
 */
const CURATED = [
	// ---- Ethereum ----
	[1, '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48', 'USDC', 6],
	[1, '0xdac17f958d2ee523a2206206994597c13d831ec7', 'USDT', 6],
	[1, '0x6b175474e89094c44da98b954eedeac495271d0f', 'DAI', 18],
	[1, '0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2', 'WETH', 18],
	[1, '0x2260fac5e5542a773aa44fbcfedf7c193bc2c599', 'WBTC', 8],
	[1, '0x514910771af9ca656af840dff83e8264ecf986ca', 'LINK', 18],
	[1, '0x1f9840a85d5af5bf1d1762f925bdaddc4201f984', 'UNI', 18],
	[1, '0xae7ab96520de3a18e5e111b5eaab095312d7fe84', 'stETH', 18],
	[1, '0xbe9895146f7af43049ca1c1ae358b0541ea49704', 'cbETH', 18],
	[1, '0xae78736cd615f374d3085123a210448e74fc6393', 'rETH', 18],
	[1, '0x7f39c581f595b53c5cb19bd0b3f8da6c935e2ca0', 'wstETH', 18],
	[1, '0x5a98fcbea516cf06857215779fd812ca3bef1b32', 'LDO', 18],
	[1, '0xd533a949740bb3306d119cc777fa900ba034cd52', 'CRV', 18],
	[1, '0x7fc66500c84a76ad7e9c93437bfc5ac33e2ddae9', 'AAVE', 18],
	[1, '0xc00e94cb662c3520282e6f5717214004a7f26888', 'COMP', 18],
	[1, '0x9f8f72aa9304c8b593d555f12ef6589cc3a579a2', 'MKR', 18],
	// ---- Polygon ----
	[137, '0x3c499c542cef5e3811e1192ce70d8cc03d5c3359', 'USDC', 6],
	[137, '0x2791bca1f2de4661ed88a30c99a7a9449aa84174', 'USDC.e', 6],
	// ---- Arbitrum ----
	[42161, '0xaf88d065e77c8cc2239327c5edb3a432268e5831', 'USDC', 6],
	[42161, '0xfd086bc7cd5c481dcc9c85ebe478a1c0b69fcbb9', 'USDT', 6]
];

const DECIMALS = '0x313ce567';
const SYMBOL = '0x95d89b41';

/** `[chainId, rpcUrl]` for every built-in network, read from the core. */
function builtinChains() {
	const source = readFileSync(NETWORKS, 'utf8');
	const block = source.slice(source.indexOf('pub const BUILTIN_CHAINS'));
	const body = block.slice(0, block.indexOf('\n];'));
	const chains = [...body.matchAll(/chain_id: (\d+),\s*native_symbol: "[^"]*",\s*rpc_url: "([^"]+)"/g)].map(
		(m) => [Number(m[1]), m[2]]
	);
	if (chains.length === 0) throw new Error('no BUILTIN_CHAINS found in network_admin.rs');
	return chains;
}

async function getJson(url, init = {}) {
	const res = await fetch(url, {
		...init,
		headers: { 'user-agent': 'vela-gen-token-registry', ...(init.headers ?? {}) },
		signal: AbortSignal.timeout(15_000)
	});
	if (!res.ok) throw new Error(`${url}: HTTP ${res.status}`);
	return res.json();
}

/** One `eth_call`, tried on each RPC in turn; `null` when none answers. */
async function call(rpcs, to, data) {
	for (const rpc of rpcs) {
		try {
			const body = { jsonrpc: '2.0', id: 1, method: 'eth_call', params: [{ to, data }, 'latest'] };
			const reply = await getJson(rpc, {
				method: 'POST',
				headers: { 'content-type': 'application/json' },
				body: JSON.stringify(body)
			});
			if (typeof reply.result === 'string' && reply.result.length > 2) return reply.result;
		} catch {
			// The next RPC.
		}
	}
	return null;
}

/** A `uint8` answer, or `null` for anything that is not one. */
function decimalsOf(hex) {
	if (!hex || !/^0x[0-9a-f]{64}$/i.test(hex)) return null;
	const n = BigInt(hex);
	return n <= 255n ? Number(n) : null;
}

/** An ABI `string` answer, or a `bytes32` one (MKR), as text. */
function symbolOf(hex) {
	if (!hex) return null;
	const bytes = Buffer.from(hex.slice(2), 'hex');
	if (bytes.length >= 96) {
		const len = Number(BigInt('0x' + bytes.subarray(32, 64).toString('hex')));
		return bytes.subarray(64, 64 + len).toString('utf8');
	}
	return bytes.toString('utf8').replace(/\0+$/, '');
}

async function readToken(rpcs, address) {
	const [decimals, symbol] = await Promise.all([call(rpcs, address, DECIMALS), call(rpcs, address, SYMBOL)]);
	return { decimals: decimalsOf(decimals), symbol: symbolOf(symbol) };
}

async function scanChain([chainId, builtinRpc]) {
	const doc = await getJson(`${CHAIN_DATA}/eip155-${chainId}.json`);
	const rpcs = [
		builtinRpc,
		...(doc.rpc ?? []).filter((u) => typeof u === 'string' && u.startsWith('https://') && !u.includes('${'))
	]
		.filter((u, i, all) => all.indexOf(u) === i)
		.slice(0, 6);
	const wanted = (doc.stables ?? [])
		.filter((s) => typeof s.contract === 'string' && typeof s.symbol === 'string')
		.map((s) => ({ address: s.contract.toLowerCase(), symbol: s.symbol, source: 'chain-data' }));
	const wrapped = doc.wrappedNativeToken;
	if (typeof wrapped === 'string' && /^0x[0-9a-f]{40}$/i.test(wrapped)) {
		wanted.push({ address: wrapped.toLowerCase(), symbol: null, source: 'wrapped native' });
	}
	for (const [chain, address, symbol] of CURATED) {
		if (chain === chainId) wanted.push({ address, symbol, source: 'curated' });
	}
	const rows = [];
	for (const want of wanted) {
		const read = await readToken(rpcs, want.address);
		rows.push({ chainId, ...want, read });
	}
	return rows;
}

const chains = builtinChains();
const scanned = (await Promise.all(chains.map(scanChain))).flat();

/** `${chainId}:${address}` → row; chain-data first, so it names a token both list. */
const table = new Map();
const problems = [];
for (const row of scanned) {
	const key = `${row.chainId}:${row.address}`;
	const curated = CURATED.find(([c, a]) => `${c}:${a}` === key);
	if (row.read.decimals === null) {
		if (curated) problems.push(`${key}: curated ${curated[2]} answered no decimals()`);
		else console.warn(`skip ${key} (${row.source}): no decimals() answer`);
		continue;
	}
	if (curated && curated[3] !== row.read.decimals) {
		problems.push(`${key}: curated ${curated[2]} says ${curated[3]} decimals, the chain says ${row.read.decimals}`);
		continue;
	}
	const symbol = row.symbol ?? row.read.symbol;
	if (!symbol) {
		console.warn(`skip ${key} (${row.source}): no symbol`);
		continue;
	}
	const existing = table.get(key);
	if (existing) {
		if (existing.symbol !== symbol || existing.decimals !== row.read.decimals) {
			console.warn(`${key}: ${row.source} says ${symbol}/${row.read.decimals}; keeping ${existing.symbol}/${existing.decimals}`);
		}
		continue;
	}
	table.set(key, { chainId: row.chainId, address: row.address, symbol, decimals: row.read.decimals });
}
if (problems.length > 0) {
	console.error(problems.join('\n'));
	process.exit(1);
}

const rows = [...table.values()].sort((a, b) => a.chainId - b.chainId || (a.address < b.address ? -1 : 1));
const rust = (value) => JSON.stringify(value); // Rust and JSON agree on "…"

const networks = new Set(rows.map((r) => r.chainId)).size;
const out = `//! @generated by \`scripts/gen-token-registry.mjs\` — do not edit.
//!
//! ${rows.length} tokens on ${networks} networks: the stablecoins and wrapped native coin
//! the chain-data service lists for each built-in network, and the well-known
//! tokens pinned to their chains. Decimals are each contract's own
//! \`decimals()\`. Regenerate with \`node scripts/gen-token-registry.mjs\`.

/// \`(chain id, lowercased address, symbol, decimals)\`, sorted by chain id,
/// then address — strictly, so a lookup can binary-search and no token is
/// listed twice. One token per line (\`rustfmt::skip\`), so a refresh diffs
/// by token.
#[rustfmt::skip]
pub(super) static TABLE: [(u32, &str, &str, u32); ${rows.length}] = [
${rows.map((r) => `    (${r.chainId}, ${rust(r.address)}, ${rust(r.symbol)}, ${r.decimals}),`).join('\n')}
];
`;

const text = out;

if (process.argv.includes('--check')) {
	let current = '';
	try {
		current = readFileSync(TARGET, 'utf8');
	} catch {
		// Missing counts as stale.
	}
	if (current !== text) {
		console.error('token registry differs from the chain-data service — run `node scripts/gen-token-registry.mjs`');
		process.exit(1);
	}
	console.log(`token registry in sync (${rows.length} tokens)`);
} else {
	writeFileSync(TARGET, text);
	console.log(`wrote ${TARGET} (${rows.length} tokens)`);
}
