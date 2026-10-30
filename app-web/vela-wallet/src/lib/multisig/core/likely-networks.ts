/**
 * Which network a multisig is likely used on, for the two places a person
 * picks one — the add sheet when nothing says (r4b-8, r4D-4), and the create
 * sheet's first network (r4a-14). The order is the core's rule
 * (`multisig_rules::networks`, asked through `multisigLinkCall`): what a link
 * named, the network the wallet is on now (r5reg-19), the networks the
 * multisigs here were last used on, where this device last transacted, where
 * the account shown holds funds, then the wallet's own order. Here only the
 * hints are gathered and the core asked. Only an order to offer them in: the
 * pick stays the person's.
 */
import * as wasm from '../../../../../../rust/pkg-web/vela_core.js';
import { multisig } from './store.svelte';

export interface Hints {
	/** Networks a link named, first first. */
	named?: number[];
	/** r5reg-19: the network the wallet is on now — the one its network filter shows. */
	here?: number[];
	/** Networks the account shown holds funds on, most first. */
	held?: number[];
	/** Networks the multisigs on this device were last used on, latest first. */
	used?: number[];
	/** r5reg-19: networks this device's own transactions were on, newest first. */
	recent?: number[];
}

function rule<T>(method: string, args: Record<string, unknown>): T {
	return JSON.parse(wasm.multisigLinkCall(`networks.${method}`, JSON.stringify(args))) as T;
}

/** Whether the core is here to ask (the accounts machine has read the store). */
const ready = () => multisig.view !== null;

/** The networks the multisigs here were first used on, the newest first. */
export function lastUsedChains(): number[] {
	if (!ready()) return [];
	const accounts = multisig.accounts.map((a) => ({ chainIds: a.chainIds, addedMs: a.addedMs }));
	return rule<number[]>('lastUsed', { accounts });
}

/** `networks`, the likely ones first, the rest in the wallet's order (the core's). */
export function likelyNetworks<N extends { chainId: number }>(networks: N[], hints: Hints): N[] {
	if (!ready()) return networks;
	const order = rule<number[]>('likely', { networks: networks.map((n) => n.chainId), hints });
	return order.flatMap((id) => networks.filter((n) => n.chainId === id).slice(0, 1));
}

/**
 * The create sheet's first network (r4a-14), among `networks` — never one
 * picked for the person when nothing says (`null`: the person picks).
 */
export function firstNetworkDefault(networks: { chainId: number }[], hints: Hints): number | null {
	if (!ready()) return null;
	return rule<number | null>('firstDefault', { networks: networks.map((n) => n.chainId), hints });
}

/** r5reg-19: the networks this device's transactions were on, the newest first. */
export function recentChains(records: { chainId: number; timestamp: number }[]): number[] {
	if (!ready()) return [];
	return rule<number[]>('recent', {
		records: records.map((r) => ({ chainId: r.chainId, timestamp: r.timestamp }))
	});
}

/** The networks the account shown holds anything on, the most valuable first. */
export function heldChains(
	tokens: { chain_id: number; balance: string; price_usd: number | null; spam?: boolean }[]
): number[] {
	if (!ready()) return [];
	return rule<number[]>('held', {
		tokens: tokens.map((t) => ({
			chain_id: t.chain_id,
			balance: String(t.balance),
			price_usd: t.price_usd ?? null,
			spam: t.spam ?? false
		}))
	});
}
