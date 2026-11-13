/**
 * Which of Vela's networks a person's lists show (owner decision
 * 2026-10-29) — WEB.
 *
 * The core decides (`shown_networks`): a network the table marks
 * `shownByDefault: false` is shown once the person shows it, and a device
 * with no record yet is settled ONCE from what it holds — a last-known
 * balance, a record of activity, a multisig, a token added there, settings of
 * the person's own, the network added by hand before Vela shipped it. This
 * file reads those stores as they are (the evidence), hands them over, and
 * stores the record the core answers. Reading nothing found is what the core
 * is told; it decides what that means.
 */
import type { NetNetworkConfig } from '$lib/core/generated/NetNetworkConfig';
import type { NetShownNetworks } from '$lib/core/generated/NetShownNetworks';
import type { NetShownSettled } from '$lib/core/generated/NetShownSettled';
import type { NetUseEvidence } from '$lib/core/generated/NetUseEvidence';
import {
	loadCustomTokens,
	loadTransactions,
	type CustomNetworkRecord,
	type NetworkConfigRecord
} from './records';
import { getItem, setItem } from './storage';

/** The core's `shown_networks::KEY`. */
export const SHOWN_NETWORKS_KEY = 'vela.shownNetworks';
/** The holdings the home last showed (`holdings-cache.ts`). */
const HOLDINGS_KEY = 'vela.holdingsCache';
/** The multisigs this browser added (`multisig/core/store.svelte.ts`). */
const MULTISIG_KEY = 'vela.multisigAccounts';

/** The stored record, or `null` when there is none or it cannot be read. */
export async function loadShownNetworks(): Promise<NetShownNetworks | null> {
	try {
		const raw = await getItem(SHOWN_NETWORKS_KEY);
		if (!raw) return null;
		const parsed = JSON.parse(raw) as Partial<NetShownNetworks> | null;
		if (parsed === null || typeof parsed !== 'object' || !Array.isArray(parsed.shown)) return null;
		return {
			version: typeof parsed.version === 'number' ? parsed.version : 0,
			shown: parsed.shown.filter((id): id is number => typeof id === 'number')
		};
	} catch {
		return null;
	}
}

const isChainId = (value: unknown): value is number =>
	typeof value === 'number' && Number.isInteger(value) && value > 0;

/** Networks where a last-known holding is more than zero (spam is no money). */
async function heldChainIds(): Promise<number[]> {
	try {
		const raw = await getItem(HOLDINGS_KEY);
		if (!raw) return [];
		const parsed = JSON.parse(raw) as Record<string, { tokens?: unknown }> | null;
		const ids = new Set<number>();
		for (const entry of Object.values(parsed ?? {})) {
			if (!Array.isArray(entry?.tokens)) continue;
			for (const token of entry.tokens as Record<string, unknown>[]) {
				if (token === null || typeof token !== 'object' || token.spam === true) continue;
				if (isChainId(token.chain_id) && /[1-9]/.test(String(token.balance ?? '')))
					ids.add(token.chain_id);
			}
		}
		return [...ids];
	} catch {
		return [];
	}
}

/** Networks a multisig on this browser is used on. */
function multisigChainIds(): number[] {
	try {
		const raw = globalThis.localStorage?.getItem(MULTISIG_KEY);
		if (!raw) return [];
		const rows = JSON.parse(raw) as unknown;
		if (!Array.isArray(rows)) return [];
		return rows.flatMap((row: { chainIds?: unknown }) =>
			Array.isArray(row?.chainIds) ? row.chainIds.filter(isChainId) : []
		);
	} catch {
		return [];
	}
}

/** What `networks.ts` has already read: the person's networks and settings. */
export interface StoredNetworks {
	custom: readonly CustomNetworkRecord[];
	configs: readonly NetworkConfigRecord[];
}

/** Everything this browser holds that ties its person to a network. */
export async function gatherNetworkUse({
	custom,
	configs
}: StoredNetworks): Promise<NetUseEvidence> {
	const [held, txs, tokens] = await Promise.all([
		heldChainIds(),
		loadTransactions().catch(() => []),
		loadCustomTokens().catch(() => [])
	]);
	const overrides: NetNetworkConfig[] = configs.map((c) => ({
		chain_id: c.chainId,
		rpc_url: c.rpcURL ?? '',
		explorer_url: c.explorerURL ?? '',
		bundler_url: c.bundlerURL ?? '',
		only_mine: c.onlyMine === true
	}));
	return {
		balance_chain_ids: held,
		activity_chain_ids: txs.map((t) => t.chainId).filter(isChainId),
		multisig_chain_ids: multisigChainIds(),
		chosen_chain_ids: tokens.map((t) => t.chainId).filter(isChainId),
		overrides,
		added_chain_ids: custom.map((n) => n.chainId)
	};
}

/**
 * The device's record: the stored one, else — once — the one the core
 * settles from what this browser holds, stored before it is used. A record
 * that cannot be stored is still used this session (the next load settles
 * again, on the same evidence).
 */
export async function settleShownNetworks(stored: StoredNetworks): Promise<NetShownNetworks> {
	const record = await loadShownNetworks();
	if (record !== null) return record;
	const [{ loadCore, shownNetworksSettle }, evidence] = await Promise.all([
		import('$lib/core/client'),
		gatherNetworkUse(stored)
	]);
	await loadCore();
	const settled = JSON.parse(shownNetworksSettle('', JSON.stringify(evidence))) as NetShownSettled;
	if (settled.write) {
		try {
			await setItem(SHOWN_NETWORKS_KEY, JSON.stringify(settled.record));
		} catch {
			// Best effort: the record still holds for this session.
		}
	}
	return settled.record;
}
