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
 * What is left here is what only a shell can do: perform the requests the core
 * hands over (an `eth_call` through the pool, a GET against the configured
 * index, read per call so a settings edit reaches the next lookup) and keep
 * the verdict for as long as the core says it may be kept.
 */
import { loadCore, registryNameStep } from '$lib/core/client';
import { getPasskeyIndexURL } from './endpoints';
import { fetchWithTimeout, NET_TIMEOUTS } from './net';
import { poolRpcCall } from './rpc-pool';
import { getItem, setItem } from './storage';

export interface IndexedWalletName {
	/** The wallet's name as its owner registered it. */
	name: string;
	/** The founding public key, uncompressed (`04‖x‖y`), lowercase bare hex. */
	publicKey: string;
}

// --- The core's contract (`registry_lookup.rs`), as it crosses the boundary ---

type LookupRequest =
	| { type: 'eth_call'; id: string; chain_id: number; to: string; data: string }
	| { type: 'index_get'; id: string; path: string };

interface LookupAnswer {
	id: string;
	outcome: 'ok' | 'not_found' | 'failed';
	body: string | null;
}

type LookupStep =
	| { type: 'ask'; requests: LookupRequest[] }
	| {
			type: 'done';
			found: { name: string; public_key: string } | null;
			remember: 'forever' | 'briefly' | 'no';
	  };

/** `registry_lookup::MISS_TTL_MS` — a miss is only true for now. */
const MISS_TTL_MS = 6 * 60 * 60 * 1000;
/** A lookup is a handful of rounds; this only stops a contract bug from spinning. */
const MAX_ROUNDS = 16;

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

// --- Transport ---------------------------------------------------------------

const failed = (id: string): LookupAnswer => ({ id, outcome: 'failed', body: null });

async function perform(request: LookupRequest): Promise<LookupAnswer> {
	const { id } = request;
	try {
		if (request.type === 'eth_call') {
			const response = await poolRpcCall(
				'eth_call',
				[{ to: request.to, data: request.data }, 'latest'],
				request.chain_id
			);
			return response.error == null && typeof response.result === 'string'
				? { id, outcome: 'ok', body: response.result }
				: failed(id);
		}
		const baseUrl = getPasskeyIndexURL().trim().replace(/\/$/, '');
		const response = await fetchWithTimeout(
			baseUrl + request.path,
			{},
			{ timeoutMs: NET_TIMEOUTS.keyIndexRead }
		);
		// "No such key / unit" is an answer; any other non-OK is the index failing.
		if (response.status === 404) return { id, outcome: 'not_found', body: null };
		if (!response.ok) return failed(id);
		return { id, outcome: 'ok', body: await response.text() };
	} catch {
		return failed(id);
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
		const answers: LookupAnswer[] = [];
		for (let round = 0; round < MAX_ROUNDS; round++) {
			const step = JSON.parse(registryNameStep(key, JSON.stringify(answers))) as LookupStep;
			if (step.type === 'ask') {
				answers.push(...(await Promise.all(step.requests.map(perform))));
				continue;
			}
			const hit =
				step.found === null ? null : { name: step.found.name, publicKey: step.found.public_key };
			if (hit !== null && step.remember === 'forever') await writeCache(key, { hit });
			else if (hit === null && step.remember === 'briefly')
				await writeCache(key, { missAt: Date.now() });
			return hit;
		}
		return null;
	} catch {
		return null;
	}
}
