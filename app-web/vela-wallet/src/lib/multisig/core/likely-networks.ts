/**
 * Which network a multisig is likely used on, for the two places a person
 * picks one — the add sheet when nothing says (r4b-8, r4D-4), and the create
 * sheet's first network (r4a-14). The order is the core's rule
 * (`multisig_rules::networks`, asked through `multisigLinkCall`): what a link
 * named, the network the wallet is on now (r5reg-19), the networks the
 * multisigs here were last used on, where this device last transacted, where
 * the account shown holds funds, then the wallet's own order. The multisigs'
 * networks and this device's recent ones are the accounts machine's view
 * (`used_networks`, `recent_networks`); here only the wallet's own hints are
 * gathered and the core asked. Only an order to offer them in: the pick
 * stays the person's.
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

/**
 * core-ms-2: the core's catalog's name for a network this device may not
 * have (`networks.name`), or `null` when the catalog does not know it.
 */
export function catalogName(chainId: number): string | null {
	try {
		return rule<string | null>('name', { chainId });
	} catch {
		// The core is not loaded here (yet): the catalog says nothing.
		return null;
	}
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
