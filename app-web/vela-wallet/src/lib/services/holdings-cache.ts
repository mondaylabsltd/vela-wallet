/**
 * The rows the home last showed, per account and space — WEB.
 *
 * Kept so a page opened while a network is down, or before the slowest
 * network has answered, opens its Assets list on what was last known instead
 * of a skeleton (home-1, home-3): the balance core is handed them once
 * (`HoldingsRestored`) and decides what they may stand for — last-known rows
 * a chain's own answer replaces, kept for a chain that does not answer.
 *
 * The total's twin (`balance-cache.ts`): one KV key (`vela.holdingsCache`),
 * hydrated once, the same 24h TTL, persisted fire-and-forget. Unlike the
 * total, the testnet space keeps its rows too — they are coins, not a value.
 */
import type { BalanceToken } from '$lib/core/generated/BalanceToken';
import { getItem, setItem } from './storage';

const STORAGE_KEY = 'vela.holdingsCache';
const TTL_MS = 24 * 60 * 60 * 1000;

interface CacheEntry {
	at: number;
	tokens: BalanceToken[];
}

const mem = new Map<string, CacheEntry>();
let loaded: Promise<void> | null = null;

function key(address: string, space: string): string {
	return `${address.trim().toLowerCase()}#${space}`;
}

/** A stored row is only believed in the shape the core reads. */
function isToken(raw: unknown): raw is BalanceToken {
	if (raw === null || typeof raw !== 'object') return false;
	const t = raw as Record<string, unknown>;
	return (
		typeof t.chain_id === 'number' &&
		typeof t.symbol === 'string' &&
		typeof t.name === 'string' &&
		typeof t.balance === 'string' &&
		typeof t.decimals === 'number' &&
		(t.token_address === null || typeof t.token_address === 'string') &&
		(t.price_usd === null || typeof t.price_usd === 'number') &&
		typeof t.spam === 'boolean'
	);
}

function ensureLoaded(): Promise<void> {
	loaded ??= (async () => {
		try {
			const raw = await getItem(STORAGE_KEY);
			if (!raw) return;
			const parsed = JSON.parse(raw) as Record<string, { at?: unknown; tokens?: unknown }>;
			for (const [k, entry] of Object.entries(parsed)) {
				if (typeof entry?.at !== 'number' || !Array.isArray(entry.tokens)) continue;
				mem.set(k, { at: entry.at, tokens: entry.tokens.filter(isToken) });
			}
		} catch {
			// A corrupt store reads as empty.
		}
	})();
	return loaded;
}

async function persist(): Promise<void> {
	try {
		await setItem(STORAGE_KEY, JSON.stringify(Object.fromEntries(mem)));
	} catch {
		// Best effort.
	}
}

/** Keep the rows a settled round left on the home for `address` in `space`. */
export async function saveHoldings(
	address: string,
	space: string,
	tokens: readonly BalanceToken[]
): Promise<void> {
	await ensureLoaded();
	mem.set(key(address, space), { at: Date.now(), tokens: [...tokens] });
	void persist();
}

/** The rows last kept, or `[]` when there are none or they are a day old. */
export async function loadHoldings(address: string, space: string): Promise<BalanceToken[]> {
	await ensureLoaded();
	const entry = mem.get(key(address, space));
	if (!entry || Date.now() - entry.at > TTL_MS) return [];
	return entry.tokens.map((t) => ({ ...t }));
}

/**
 * Forget the memory copy once the stored key has been removed (Settings'
 * storage clears): the next read hydrates from what the store holds now.
 */
export function dropHoldingsMemory(): void {
	mem.clear();
	loaded = null;
}
