/**
 * The passkey-index client — WEB (spec 025 Phase 5). The read path the
 * identity waterfall needs: the name a Vela user registered, by ADDRESS.
 *
 * The v2 index cannot be asked about an address (`?walletRef=` answers 400:
 * an address is `f(all founding keys)`, no entry owns one), so the name is
 * reached in three hops — the chain's `SafeWebAuthnSharedSigner` configuration
 * → `?publicKey=` → `?unitId=`. WHICH chain is asked first, which unit is
 * believed, what a malformed blob means and how long a verdict may be kept are
 * all `vela_core::registry_lookup` — written once for four shells (issue 191).
 *
 * What is left here is what only a shell can do: carry the requests
 * (`core-walk.ts`, shared with the Ethereum backup) and keep the verdict for as
 * long as the core says it may be kept.
 */
import { loadCore, registryNameStep } from '$lib/core/client';
import { runWalk } from './core-walk';
import { getItem, setItem } from './storage';

export interface IndexedWalletName {
	/** The wallet's name as its owner registered it. */
	name: string;
	/** The founding public key, uncompressed (`04‖x‖y`), lowercase bare hex. */
	publicKey: string;
}

/** `registry_lookup::LookupStep::Done`. */
interface LookupDone {
	type: 'done';
	found: { name: string; public_key: string } | null;
	remember: 'forever' | 'briefly' | 'no';
}

/** `registry_lookup::MISS_TTL_MS` — a miss is only true for now. */
const MISS_TTL_MS = 6 * 60 * 60 * 1000;

// --- Cache: the key and the name for good, a miss briefly -------------------

const CACHE_PREFIX = 'vela.indexName:';

type Cached = { hit: IndexedWalletName } | { missAt: number };

async function readCache(address: string): Promise<IndexedWalletName | null | undefined> {
	try {
		const raw = await getItem(CACHE_PREFIX + address);
		if (!raw) return undefined;
		const entry = JSON.parse(raw) as Cached;
		if ('hit' in entry) return entry.hit;
		return Date.now() - entry.missAt < MISS_TTL_MS ? null : undefined;
	} catch {
		return undefined;
	}
}

async function writeCache(address: string, entry: Cached): Promise<void> {
	try {
		await setItem(CACHE_PREFIX + address, JSON.stringify(entry));
	} catch {
		/* best-effort */
	}
}

/**
 * The name a Vela user registered for the wallet at `address`, or `null`.
 *
 * Best-effort enrichment: a slow index, an unreachable chain or a malformed
 * record all degrade to "unknown recipient" and never throw past here.
 */
export async function queryWalletName(address: string): Promise<IndexedWalletName | null> {
	if (!/^0x[0-9a-fA-F]{40}$/.test(address)) return null;
	const key = address.toLowerCase();

	const cached = await readCache(key);
	if (cached !== undefined) return cached;

	try {
		await loadCore();
		const done = await runWalk<LookupDone>((answers) => registryNameStep(key, answers));
		if (done === null) return null;
		const hit =
			done.found === null ? null : { name: done.found.name, publicKey: done.found.public_key };
		if (hit !== null && done.remember === 'forever') await writeCache(key, { hit });
		else if (hit === null && done.remember === 'briefly')
			await writeCache(key, { missAt: Date.now() });
		return hit;
	} catch {
		return null;
	}
}
