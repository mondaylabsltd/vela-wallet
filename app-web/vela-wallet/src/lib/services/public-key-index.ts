/**
 * The passkey-index client — WEB (spec 025 Phase 5). The read path the
 * identity waterfall needs: the name a Vela user registered, by ADDRESS. The
 * writes (create / reveal) belong to onboarding, which has its own client. The
 * base URL is the configured passkey-index endpoint, read per call (a settings
 * edit reaches the next lookup).
 *
 * ## An address is not a key the index knows — so it is reached in three hops
 *
 * The v1 index answered `?walletRef=<address>`. The v2 index (the default
 * since multi-passkey wallets) does not: an address is `f(all founding keys)`,
 * no single entry owns one, and the query answers `400 publicKey, entryId,
 * unitId or groupPublicKey is required`. For two specs every shell kept asking
 * the v1 question and the registry step of the waterfall silently never
 * answered (issue 191).
 *
 * The chain already holds the missing link. A Vela Safe's founding key is
 * configured on the Safe `SafeWebAuthnSharedSigner`, which stores (x, y) in
 * the SAFE's own storage and hands them to anyone who asks:
 *
 *   1. `sharedSigner.getConfiguration(address)` → the founding public key
 *   2. `GET /api/query?publicKey=`              → the units that key founded
 *   3. `GET /api/query?unitId=`                 → frozen metadata; the unit
 *      whose `address` is the one asked about names it (`key_names[0]`, the
 *      same field `login` recovers a wallet's name from).
 *
 * Why that is not spoofable by a stranger: hop 1 is the chain's answer about
 * THIS address, and a unit only lists keys that signed their own membership.
 * Whoever can put a name there holds the wallet's founding key — it is their
 * name to give.
 *
 * ## What is remembered
 *
 * A founding key, once configured, and a unit's metadata, once registered, do
 * not change — a hit is kept for good. A miss is only true for now (a Safe is
 * counterfactual until its first operation; a name can be registered later),
 * so it is kept briefly: long enough that a book full of plain EOAs does not
 * re-ask seven chains on every visit, short enough that a new name shows up.
 */
import { getPasskeyIndexURL } from './endpoints';
import { fetchWithTimeout, NET_TIMEOUTS } from './net';
import { poolRpcCall } from './rpc-pool';
import { getItem, setItem } from './storage';

export interface IndexedWalletName {
	/** The wallet's name as its owner registered it. */
	name: string;
	/** The founding public key, uncompressed (`04‖x‖y`), lowercase hex. */
	publicKey: string;
}

/** `SafeWebAuthnSharedSigner` — same address on every chain (`safe.rs::WEBAUTHN_SIGNER`). */
const SHARED_SIGNER = '0x94a4F6affBd8975951142c3999aEAB7ecee555c2';
/** `getConfiguration(address)` → `(uint256 x, uint256 y, uint176 verifiers)`. */
const GET_CONFIGURATION = '0xc44b11f7';

/**
 * Where a Vela Safe is looked for. The address is the same everywhere, but the
 * configuration exists only where the Safe has been DEPLOYED. Gnosis first and
 * alone — it is the registry's own chain and the cheapest place most Vela
 * wallets have transacted — then the rest at once. A short list on purpose:
 * every entry is a call spent on every address that is not a Vela wallet.
 */
const HOME_CHAIN = 100;
const OTHER_CHAINS = [8453, 56, 42161, 10, 137, 1];

/** A key founds few wallets; past this the newest are the ones worth a call. */
const MAX_UNITS = 8;

const CACHE_PREFIX = 'vela.indexName:';
const MISS_TTL_MS = 6 * 60 * 60 * 1000;

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
 * Hop 1 on one chain: the key, `null` when the chain says this is not a
 * configured Vela Safe there, `undefined` when the chain did not answer — which
 * is not a verdict and must not be remembered as one.
 */
async function foundingKeyOn(address: string, chainId: number): Promise<string | null | undefined> {
	const data = GET_CONFIGURATION + address.slice(2).padStart(64, '0');
	let result: unknown;
	try {
		const response = await poolRpcCall(
			'eth_call',
			[{ to: SHARED_SIGNER, data }, 'latest'],
			chainId
		);
		if (response.error != null) return undefined;
		result = response.result;
	} catch {
		return undefined;
	}
	if (typeof result !== 'string') return undefined;
	// No contract at the signer's address on this chain answers `0x`.
	if (result.length < 2 + 128) return null;
	const x = result.slice(2, 66);
	const y = result.slice(66, 130);
	// An unconfigured account — every EOA, every other contract — reads as zero.
	if (/^0+$/.test(x) || /^0+$/.test(y)) return null;
	return ('04' + x + y).toLowerCase();
}

/** `settled: false` = at least one chain did not answer, so "no key" is not known. */
async function foundingKey(address: string): Promise<{ key: string | null; settled: boolean }> {
	const home = await foundingKeyOn(address, HOME_CHAIN);
	if (typeof home === 'string') return { key: home, settled: true };
	const others = await Promise.all(OTHER_CHAINS.map((id) => foundingKeyOn(address, id)));
	const found = others.find((answer) => typeof answer === 'string');
	if (typeof found === 'string') return { key: found, settled: true };
	return { key: null, settled: home !== undefined && others.every((a) => a !== undefined) };
}

async function getJson(path: string): Promise<unknown> {
	const baseUrl = getPasskeyIndexURL().trim().replace(/\/$/, '');
	const response = await fetchWithTimeout(
		baseUrl + path,
		{},
		{ timeoutMs: NET_TIMEOUTS.keyIndexRead }
	);
	// "No such key / unit" is an answer; anything else that is not OK is the
	// index failing, and a failure must not be remembered as a miss.
	if (response.status === 404) return null;
	if (!response.ok) throw new Error(`index answered ${response.status}`);
	return response.json();
}

const UTF8_STRICT = new TextDecoder('utf-8', { fatal: true });

/** The unit's metadata blob: hex of canonical JSON (`registry_metadata.rs`). */
function decodeMetadata(hex: unknown): { address: string; name: string } | null {
	if (typeof hex !== 'string') return null;
	const data = hex.startsWith('0x') ? hex.slice(2) : hex;
	if (data.length === 0 || data.length % 2 !== 0 || !/^[0-9a-fA-F]+$/.test(data)) return null;
	try {
		const bytes = new Uint8Array(data.length / 2);
		for (let i = 0; i < bytes.length; i++) bytes[i] = parseInt(data.slice(i * 2, i * 2 + 2), 16);
		// Strict, for the reason `recipient-identity.ts` gives: a name is drawn
		// beside somebody's money, and a lenient decoder invents one (issue 200).
		const meta = JSON.parse(UTF8_STRICT.decode(bytes)) as {
			address?: unknown;
			key_names?: unknown;
		};
		const first = Array.isArray(meta.key_names) ? meta.key_names[0] : undefined;
		if (typeof meta.address !== 'string' || typeof first !== 'string') return null;
		const name = first.trim();
		return name === '' ? null : { address: meta.address.toLowerCase(), name };
	} catch {
		return null;
	}
}

/**
 * The name a Vela user registered for the wallet at `address`, or `null`.
 *
 * Best-effort enrichment: a slow index, an unreachable chain or a malformed
 * record all degrade to "unknown recipient" and never throw past here.
 */
export async function queryWalletName(address: string): Promise<IndexedWalletName | null> {
	if (!/^0x[0-9a-fA-F]{40}$/.test(address) || /^0x0+$/.test(address)) return null;
	const key = address.toLowerCase();

	const cached = await readCache(key);
	if (cached !== undefined) return cached;

	try {
		const { key: publicKey, settled } = await foundingKey(key);
		if (publicKey === null) {
			if (settled) await writeCache(key, { missAt: Date.now() });
			return null;
		}
		const profile = (await getJson(`/api/query?publicKey=${publicKey}`)) as {
			groups?: { unitIds?: unknown };
		} | null;
		const listed = profile?.groups?.unitIds;
		const unitIds = (Array.isArray(listed) ? listed : [])
			.filter((id): id is number => Number.isInteger(id) && id >= 0)
			.slice(0, MAX_UNITS);
		// In the index's order — newest first — so a wallet registered twice is
		// called by its latest name.
		for (const unitId of unitIds) {
			const detail = (await getJson(`/api/query?unitId=${unitId}&pageSize=1`)) as {
				unit?: { metadata?: unknown };
			} | null;
			const meta = decodeMetadata(detail?.unit?.metadata);
			if (meta !== null && meta.address === key) {
				const hit = { name: meta.name, publicKey };
				await writeCache(key, { hit });
				return hit;
			}
		}
		await writeCache(key, { missAt: Date.now() });
		return null;
	} catch {
		// A transport failure is not a verdict: nothing is remembered.
		return null;
	}
}
