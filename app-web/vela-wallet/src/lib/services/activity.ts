/**
 * Receipt discovery + persistence — WEB.
 *
 * Ported (trimmed) from src/services/activity.ts @ c13e89d4 (spec 025): only
 * the WRITE side the `activity_feed` executor calls — `syncReceivedTransfers`
 * and its helpers, `dayStartMs`, the stablecoin table. The READ side (the
 * old TypeScript `loadActivityItems` feed, relative-time and date wording)
 * is the core's on web (`activity_feed.rs`) and did not come along.
 *
 * The ONE judgement kept here is the ingest valuation in `incomingToRecord`
 * (spec 017 `no_core_owns_it`): a real price if we hold one, else ≈$1 for a
 * stablecoin, else unknown. The core re-derives from the stored string on
 * read (`tx_usd_value`) and applies the SAME table, so the two sides cannot
 * disagree — `STABLE_SYMBOLS` is mirrored verbatim in `activity_feed.rs`.
 */

import { incomingScanPlan } from '$lib/core/kernels';
import type { TrustScanFacts } from '$lib/core/generated/TrustScanFacts';
import { fromBaseUnits } from './eip681';
import { fetchIncomingTransfers } from './incoming-transfers';
import { getCustomChainIdsSync, nativeSymbol, spaceNetworksSync } from './networks';
import { watchNativeBalances } from './native-watch';
import { mergeTransactions } from './records';
import { resolveTokenMetadata } from './token-metadata';
import { tokenChainId, type APIToken } from './tokens-model';
import { tokenLogoURLs } from '$lib/flows/marks';
import type { IncomingTransfer } from './transfer-types';
import type { LocalTransaction } from './transactions-model';
import { fetchTokens } from './wallet-api';

/** Local-midnight grouping key for a unix-seconds timestamp (device timezone). */
export function dayStartMs(tsSeconds: number): number {
	const d = new Date(tsSeconds * 1000);
	return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
}

function formatUsd(n: number): string {
	if (!isFinite(n) || n <= 0) return '$0.00';
	return '$' + n.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

/**
 * Symbols treated as ≈ $1 so stablecoin transfers aren't shown as "$0.00".
 * Copied verbatim in `activity_feed.rs`'s `STABLE_SYMBOLS`.
 */
export const STABLE_SYMBOLS = new Set([
	'USDT',
	'USDT0',
	'USDC',
	'USDC.E',
	'DAI',
	'BUSD',
	'TUSD',
	'FDUSD',
	'USDE',
	'PYUSD',
	'USDP',
	'GUSD',
	'LUSD',
	'FRAX',
	'USDD'
]);

/** Upper-case and fold the Tether glyph "₮" to "T" so "USD₮0" matches "USDT0". */
function stableKey(symbol: string): string {
	return (symbol || '').toUpperCase().replace(/₮/g, 'T');
}

export function isStable(symbol: string): boolean {
	return STABLE_SYMBOLS.has(stableKey(symbol));
}

interface TokenMeta {
	symbol: string;
	decimals: number;
	priceUsd: number | null;
}

function buildTokenIndex(tokens: APIToken[]): Map<string, TokenMeta> {
	const m = new Map<string, TokenMeta>();
	for (const t of tokens) {
		const cid = tokenChainId(t);
		const key = t.tokenAddress ? `${cid}:${t.tokenAddress.toLowerCase()}` : `${cid}:native`;
		m.set(key, { symbol: t.symbol, decimals: t.decimals, priceUsd: t.priceUsd });
	}
	return m;
}

/** Map a discovered incoming transfer into a persistable 'receive' record. */
export function incomingToRecord(
	tx: IncomingTransfer,
	address: string,
	index: Map<string, TokenMeta>
): LocalTransaction {
	const key = tx.isNative
		? `${tx.chainId}:native`
		: `${tx.chainId}:${(tx.token ?? '').toLowerCase()}`;
	const meta = index.get(key);
	const symbol = meta?.symbol ?? (tx.isNative ? nativeSymbol(tx.chainId) : 'tokens');
	const decimals = meta?.decimals ?? 18;
	const amount = Number(tx.value) / 10 ** decimals;
	// Ingest valuation — the shell's, on purpose (see the module doc).
	const usd =
		meta?.priceUsd != null
			? formatUsd(amount * meta.priceUsd)
			: isStable(symbol)
				? formatUsd(amount)
				: '$0.00';
	// Logo candidates captured now, while the contract address is in hand.
	const logoUrls = tx.isNative
		? tokenLogoURLs(tx.chainId, symbol, null)
		: tx.token
			? tokenLogoURLs(tx.chainId, symbol, tx.token)
			: [];
	return {
		id: tx.id,
		userOpHash: '',
		txHash: tx.txHash,
		from: tx.from,
		to: address,
		// Exact, never `String(Number)`: a receive below 1e-6 became
		// "1.373924e-12" and read 10¹² too large (097 final pass).
		value: fromBaseUnits(tx.value, decimals),
		symbol,
		decimals,
		...(logoUrls.length ? { logoUrls } : {}),
		// Round 5: the contract it came from — a symbol alone is whatever a
		// contract answers.
		...(!tx.isNative && tx.token ? { token: tx.token } : {}),
		chainId: tx.chainId,
		timestamp: tx.timestamp,
		// PR 3: the feed's time IS the block's own (`token_trust` withholds a
		// transfer whose block it has not read), so the record says so — the
		// mark that tells it from an older one stamped with the clock.
		timeVerified: true,
		status: 'confirmed',
		type: 'receive',
		usd
	};
}

/**
 * Fill the lookup with metadata for incoming tokens the user doesn't already
 * hold, so a 6-decimal stablecoin is never persisted as "+0 tokens".
 */
async function enrichTokenIndex(
	incoming: IncomingTransfer[],
	index: Map<string, TokenMeta>
): Promise<void> {
	const byChain = new Map<number, Set<string>>();
	for (const tx of incoming) {
		if (tx.isNative || !tx.token) continue;
		const addr = tx.token.toLowerCase();
		if (index.has(`${tx.chainId}:${addr}`)) continue;
		let set = byChain.get(tx.chainId);
		if (!set) byChain.set(tx.chainId, (set = new Set()));
		set.add(addr);
	}
	if (byChain.size === 0) return;

	await Promise.all(
		[...byChain].map(async ([chainId, addrs]) => {
			const metas = await resolveTokenMetadata(chainId, [...addrs]).catch((e) => {
				console.warn(
					`[Activity] token metadata unresolved for ${addrs.size} token(s) on chain ${chainId} ` +
						`(${e instanceof Error ? e.message : String(e)}) — kept out of feed, will retry next sync`
				);
				return null;
			});
			if (!metas) return;
			for (const [addr, meta] of metas) {
				index.set(`${chainId}:${addr}`, {
					symbol: meta.symbol,
					decimals: meta.decimals,
					priceUsd: null
				});
			}
		})
	);
}

/** The multisigs this browser added (`multisig/core/store.svelte.ts`'s key). */
const MULTISIG_ACCOUNTS_KEY = 'vela.multisigAccounts';

/**
 * r9-17: the networks `address` lives on when it is a multisig this browser
 * added — its record's networks, where its proposals are looked for. Read
 * from the stored list (the store's own JSON, as `shown-networks.ts` reads
 * it), so this module needs no store. A Vela account lives on every network
 * and has none.
 */
function multisigNetworks(address: string): number[] {
	try {
		const raw = globalThis.localStorage?.getItem(MULTISIG_ACCOUNTS_KEY);
		if (!raw) return [];
		const rows = JSON.parse(raw) as unknown;
		if (!Array.isArray(rows)) return [];
		const row = rows.find(
			(r: { address?: unknown }) =>
				typeof r?.address === 'string' && r.address.toLowerCase() === address.toLowerCase()
		) as { chainIds?: unknown } | undefined;
		return Array.isArray(row?.chainIds)
			? row.chainIds.filter((id): id is number => Number.isSafeInteger(id) && id > 0)
			: [];
	} catch {
		return [];
	}
}

/**
 * r9-17: what this browser knows when it scans `address` — facts, no choice.
 * Which networks are scanned, how far back and how often is the core's
 * (`incoming_scan.rs`), the same rule the apps run.
 */
export function scanFacts(address: string, tokens: APIToken[]): TrustScanFacts {
	return {
		held_chain_ids: [...new Set(tokens.map(tokenChainId))],
		account_networks: multisigNetworks(address).map((chain_id) => ({
			chain_id,
			deploy_block: null
		})),
		space_chain_ids: spaceNetworksSync().map((net) => net.chainId),
		added_chain_ids: getCustomChainIdsSync()
	};
}

/**
 * Discover and persist new receipts for `address`. Answers how many landed;
 * any failure answers 0 (a failed scan is a scan that found nothing).
 */
export async function syncReceivedTransfers(address: string): Promise<number> {
	if (!address) return 0;
	try {
		const tokens = await fetchTokens(address).catch(() => [] as APIToken[]);
		const facts = scanFacts(address, tokens);
		// r9-17: the networks are the core's — where the account holds, where
		// it lives (a multisig's networks, so its first receipt on a network
		// it holds nothing on yet is seen), else the space's defaults.
		const plan = incomingScanPlan(facts);
		// home-5: a native coin paid to an account with no code leaves no log
		// for the scan below to find — its balance is what moves. Read beside
		// the scan on the networks the core names; a move re-reads the
		// balances (`onNativeBalanceMoved`).
		void watchNativeBalances(address, plan.native_watch_chain_ids);
		const incoming = await fetchIncomingTransfers(address, facts, plan.scan_chain_ids);
		if (incoming.length === 0) return 0;
		const index = buildTokenIndex(tokens);
		await enrichTokenIndex(incoming, index);
		// Non-native tokens whose metadata never resolved stay out (a wrong
		// decimals fallback would persist a misleading "+0"); they are retried
		// on the next sync while still inside the scan window.
		const records = incoming
			.filter((tx) => tx.isNative || index.has(`${tx.chainId}:${(tx.token ?? '').toLowerCase()}`))
			.map((tx) => incomingToRecord(tx, address, index));
		if (records.length === 0) return 0;
		return await mergeTransactions(records);
	} catch {
		return 0;
	}
}
