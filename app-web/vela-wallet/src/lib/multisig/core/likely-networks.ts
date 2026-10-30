/**
 * Which network a multisig is likely used on, for the two places a person
 * picks one — the add sheet when nothing says (r4b-8, r4D-4), and the create
 * sheet's first network (r4a-14): what a link named, then the network the
 * wallet is on now (its network filter, r5reg-19), then the network the
 * multisigs here were last used on, then where this device last transacted,
 * then where the account shown holds funds, then the wallet's own order. Only an order to offer them in: the
 * pick stays the person's, and nothing here is evidence of where a multisig
 * is used (that is the core's `multisigNetworks`).
 */
import { networkRank } from '$lib/services/networks';
import { multisig } from './store.svelte';

export interface Hints {
	/** Networks a link named, first first. */
	named?: number[];
	/**
	 * r5reg-19: the network the wallet is on now — the one its network
	 * filter shows — so it is never under "More networks" (Arc read there).
	 */
	here?: number[];
	/** Networks the account shown holds funds on, most first. */
	held?: number[];
	/** Networks the multisigs on this device were last used on, latest first. */
	used?: number[];
	/** r5reg-19: networks this device's own transactions were on, newest first. */
	recent?: number[];
}

/** The networks the multisigs here were first used on, the newest first. */
export function lastUsedChains(): number[] {
	return [...multisig.accounts]
		.sort((a, b) => b.addedMs - a.addedMs)
		.flatMap((a) => a.chainIds.slice(0, 1));
}

/** `networks`, the likely ones first (each hint in its own order), the rest in the wallet's. */
export function likelyNetworks<N extends { chainId: number }>(networks: N[], hints: Hints): N[] {
	const ranked = [
		...(hints.named ?? []),
		...(hints.here ?? []),
		...(hints.used ?? []),
		...(hints.recent ?? []),
		...(hints.held ?? [])
	];
	const at = (id: number) => {
		const i = ranked.indexOf(id);
		return i === -1 ? ranked.length : i;
	};
	return [...networks].sort(
		(a, b) => at(a.chainId) - at(b.chainId) || networkRank(a.chainId) - networkRank(b.chainId)
	);
}

/**
 * The create sheet's first network (r4a-14): the one the wallet is on now
 * (r5reg-19), else the one the multisigs here were last used on, else the
 * one this device last transacted on, else where the account shown holds
 * the most, among `networks` — never one picked for the person when nothing
 * says (it was
 * Sepolia by accident); `null` then, and the person picks.
 */
export function firstNetworkDefault(networks: { chainId: number }[], hints: Hints): number | null {
	const offered = new Set(networks.map((n) => n.chainId));
	const said = [
		...(hints.here ?? []),
		...(hints.used ?? []),
		...(hints.recent ?? []),
		...(hints.held ?? [])
	];
	for (const id of said) {
		if (offered.has(id)) return id;
	}
	return null;
}

/**
 * r5reg-19: the networks `records` (this device's transaction history, any
 * of its accounts) were on, the newest first — where this device was used.
 */
export function recentChains(records: { chainId: number; timestamp: number }[]): number[] {
	const out: number[] = [];
	for (const r of [...records].sort((a, b) => b.timestamp - a.timestamp)) {
		if (Number.isInteger(r.chainId) && !out.includes(r.chainId)) out.push(r.chainId);
	}
	return out;
}

/**
 * The networks `tokens` (the balance core's view of the account shown) hold
 * anything on, the most valuable first; a token with no price counts after
 * the priced ones (a testnet's coins have none).
 */
export function heldChains(
	tokens: { chain_id: number; balance: string; price_usd: number | null; spam?: boolean }[]
): number[] {
	const worth = new Map<number, number>();
	for (const t of tokens) {
		const amount = Number(t.balance);
		// A spam token's "balance" is no one's funds.
		if (t.spam || !(amount > 0)) continue;
		worth.set(t.chain_id, (worth.get(t.chain_id) ?? 0) + amount * (t.price_usd ?? 0));
	}
	return [...worth.entries()].sort((a, b) => b[1] - a[1]).map(([id]) => id);
}
