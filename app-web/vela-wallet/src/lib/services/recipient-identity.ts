/**
 * Recipient identity resolution — the waterfall behind a counterparty's name.
 *
 * Ported from src/services/recipient-identity.ts @ c13e89d4 (rpc-adapter →
 * the pool facade; AsyncStorage → the KV; keccak via `services/ens`).
 *
 * Resolution priority:
 *   1. Passkey Index — the name a Vela user registered, by address
 *      (chain → founding key → index units; `public-key-index.ts`)
 *   2. Name services via on-chain RPC (no third-party API dependencies):
 *      .bnb (BSC) · .arb (Arbitrum) · .g (Gravity) · Basenames (Base) · ENS
 *
 * Adding a name service = one entry in NAME_SERVICES. Anything following the
 * ENS registry pattern (registry.resolver(node) → resolver.name(node)) works
 * as is. Only POSITIVE results are cached (KV, 24h).
 *
 * The consumers are executors: contacts `resolve_identity`, the feed's alias
 * arm. The core decides what a name means (display, trust line); this file
 * only finds one.
 *
 * ## A reverse record is a claim, not a name (spec 081, FR-010)
 *
 * `addr.reverse` is writable by the address itself, so whoever controls an
 * address controls the string this file used to hand the UI: fund a fresh
 * address, name it after the contact somebody is about to pay, and the wallet
 * drew that name beside the payment. Every name a service returns is now put to
 * `vela_core::app::name_verify`, which resolves it FORWARD and compares; only a
 * verified name is shown or cached. The rule is the core's, for all four
 * shells; what is left here is the transport (`core-walk.ts`, the same one the
 * registry walk uses).
 */
import * as core from '$lib/core/client';
import { runWalk } from './core-walk';
import { namehash } from './ens';
import { queryWalletName } from './public-key-index';
import { poolRpcCall } from './rpc-pool';
import { getItem, setItem } from './storage';

export type IdentitySource = 'passkey' | 'ens' | string;

export interface RecipientIdentity {
	/** Display name. */
	name: string;
	/** Source label for display (e.g. "ENS", ".bnb", "Basename"). */
	source: IdentitySource;
}

interface NameServiceConfig {
	/** Human-readable label shown in UI. */
	label: string;
	/** Chain ID to send RPC calls to. */
	chainId: number;
	/** ENS-compatible Registry contract address on this chain. */
	registry: string;
	/**
	 * Optional: ReverseRegistrar address for ENSIP-19 chains. When set,
	 * `reverseRegistrar.node(address)` gives the chain-specific reverse node
	 * instead of `namehash("<addr>.addr.reverse")`.
	 */
	reverseRegistrar?: string;
}

const NAME_SERVICES: NameServiceConfig[] = [
	// SPACE ID name services (their own SID registry per chain)
	{ label: '.bnb', chainId: 56, registry: '0x08CEd32a7f3eeC915Ba84415e9C07a7286977956' },
	{ label: '.arb', chainId: 42161, registry: '0x4a067EE58e73ac5E4a43722E008DFdf65B2bF348' },
	{ label: '.g', chainId: 1625, registry: '0x5dC881dDA4e4a8d312be3544AD13118D1a04Cb17' },
	// Basenames (Base) — ENSIP-19 chain-specific reverse node
	{
		label: 'Basename',
		chainId: 8453,
		registry: '0xb94704422c2a1e396835a571837aa5ae53285a95',
		reverseRegistrar: '0x79ea96012eea67a83431f1701b3dff7e37f9e282'
	},
	// ENS on Ethereum mainnet
	{ label: 'ENS', chainId: 1, registry: '0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e' }
];

// ---------------------------------------------------------------------------
// Cache (KV, positive results only)
// ---------------------------------------------------------------------------

/**
 * The `.v2` is the forward-verification rule arriving (FR-010). Everything the
 * old prefix holds was written under no rule at all, so a poisoned name would
 * have kept its place for a day after the fix shipped. A new prefix retires the
 * lot at once: one extra lookup per contact, and the guarantee back.
 */
const CACHE_PREFIX = 'recipient_id.v2:';
const CACHE_TTL = 24 * 60 * 60 * 1000;

interface CachedEntry {
	identity: RecipientIdentity;
	cachedAt: number;
}

async function getCache(address: string): Promise<RecipientIdentity | undefined> {
	try {
		const raw = await getItem(CACHE_PREFIX + address.toLowerCase());
		if (!raw) return undefined;
		const entry = JSON.parse(raw) as CachedEntry;
		if (Date.now() - entry.cachedAt > CACHE_TTL) return undefined;
		return entry.identity;
	} catch {
		return undefined;
	}
}

async function setCache(address: string, identity: RecipientIdentity): Promise<void> {
	try {
		const entry: CachedEntry = { identity, cachedAt: Date.now() };
		await setItem(CACHE_PREFIX + address.toLowerCase(), JSON.stringify(entry));
	} catch {
		/* best-effort */
	}
}

// ---------------------------------------------------------------------------
// ENS-compatible reverse resolution via raw RPC
// ---------------------------------------------------------------------------

const RESOLVER_SELECTOR = '0x0178b8bf'; // resolver(bytes32)
const NAME_SELECTOR = '0x691f3431'; // name(bytes32)
const NODE_SELECTOR = '0xbffbe61c'; // node(address) — ENSIP-19 ReverseRegistrar

async function ethCall(to: string, data: string, chainId: number): Promise<string | null> {
	const response = await poolRpcCall('eth_call', [{ to, data }, 'latest'], chainId);
	if (response.error || typeof response.result !== 'string' || response.result === '0x')
		return null;
	return response.result;
}

/**
 * Reverse-resolve an address with an ENS-compatible registry.
 *
 * Standard: registry.resolver(namehash(addr.addr.reverse)) → resolver.name(node)
 * ENSIP-19: reverseRegistrar.node(addr) → registry.resolver(node) → resolver.name(node)
 */
async function reverseResolveRegistry(
	address: string,
	config: NameServiceConfig
): Promise<string | null> {
	try {
		let reverseNode: string;
		if (config.reverseRegistrar) {
			const addrPadded = '000000000000000000000000' + address.toLowerCase().slice(2);
			const node = await ethCall(
				config.reverseRegistrar,
				NODE_SELECTOR + addrPadded,
				config.chainId
			);
			if (!node || node.length < 66) return null;
			reverseNode = node;
		} else {
			reverseNode = namehash(`${address.toLowerCase().slice(2)}.addr.reverse`);
		}

		// Step 1: registry.resolver(node) → address
		const resolverWord = await ethCall(
			config.registry,
			RESOLVER_SELECTOR + reverseNode.slice(2),
			config.chainId
		);
		if (!resolverWord) return null;
		const resolverAddr = '0x' + resolverWord.slice(26);
		if (/^0x0+$/.test(resolverAddr)) return null;

		// Step 2: resolver.name(node) → string
		const nameWord = await ethCall(
			resolverAddr,
			NAME_SELECTOR + reverseNode.slice(2),
			config.chainId
		);
		if (!nameWord) return null;
		// What the address CLAIMS to be called. Nothing may draw it yet.
		const claimed = decodeString(nameWord);
		if (!claimed) return null;
		return await forwardVerified(claimed, address, config);
	} catch {
		return null;
	}
}

// ---------------------------------------------------------------------------
// Forward verification (FR-010) — the core's rule, this shell's transport
// ---------------------------------------------------------------------------

/**
 * `vela_core::app::name_verify::step_json` as the wasm binding exposes it:
 * transcript in, next step out.
 */
type VerifiedNameStep = (
	chainId: number,
	registry: string,
	address: string,
	name: string,
	answersJson: string
) => string;

/** `name_verify::VerifyStep::Done`. */
interface VerifyDone {
	type: 'done';
	forward_state: 'unchecked' | 'verified' | 'mismatch' | 'unavailable';
	/** The name exactly as the core PROVED it, normalised. */
	name: string | null;
}

/**
 * The core's verifier, if the loaded build carries it.
 *
 * Looked up rather than imported by name because the wasm bindings are a
 * GENERATED artifact: `rust/pkg-web` is rebuilt by `build:wasm`, and a build
 * that predates this rule simply has no such function. A static named import of
 * a missing export does not degrade — it fails the module link and takes the
 * whole app with it — so the function is read off the module instead, and its
 * absence is treated as exactly what it is: a check that cannot be made, no
 * different from a resolver that did not answer, and the address shown alone.
 *
 * The facade is asked first, because `$lib/core/client` is where the core's
 * functions are named and where this one belongs once it exists; the generated
 * module is the fallback so that regenerating the bindings is enough to switch
 * the rule on, with no second edit to remember.
 */
async function coreVerifier(): Promise<VerifiedNameStep | undefined> {
	const named = (core as Record<string, unknown>).verifiedNameStep;
	if (typeof named === 'function') return named as VerifiedNameStep;
	try {
		// Imported dynamically: `loadCore()` has already instantiated this
		// module, and nothing that was not loading the core should start.
		const wasm = (await import('../../../../../rust/pkg-web/vela_core.js')) as unknown as Record<
			string,
			unknown
		>;
		const step = wasm.verifiedNameStep;
		return typeof step === 'function' ? (step as VerifiedNameStep) : undefined;
	} catch {
		return undefined;
	}
}

/**
 * `claimed`, but only if resolving it forward lands back on `address`.
 *
 * Which calls are made, in what order, what an unanswered one means and the
 * comparison itself are all `name_verify`'s; this drives the transcript and
 * performs the `eth_call`s. `null` covers both "this name is somebody else's"
 * and "nobody answered" — failing open on the second would let whoever poisons
 * a record choose the moment.
 */
async function forwardVerified(
	claimed: string,
	address: string,
	config: NameServiceConfig
): Promise<string | null> {
	try {
		await core.loadCore();
		const verify = await coreVerifier();
		if (!verify) return null;
		const done = await runWalk<VerifyDone>((answers) =>
			verify(config.chainId, config.registry, address, claimed, answers)
		);
		if (done === null || done.forward_state !== 'verified') return null;
		// The core hands back the name it PROVED, normalised. Drawing that and
		// not the reverse record's own spelling is what keeps what is shown and
		// what was checked from drifting apart.
		return done.name;
	} catch {
		return null;
	}
}

/** Throws on invalid UTF-8 instead of substituting U+FFFD — see below. */
const UTF8_STRICT = new TextDecoder('utf-8', { fatal: true });

/**
 * Decode a Solidity `string` return value: offset (32 bytes) + length (32
 * bytes) + data (padded).
 *
 * A name is drawn next to somebody's money, so a malformed answer is REFUSED
 * rather than shown as a plausible-looking fragment — the rule the desktop
 * shell's `decode_name` states and tests ("non-UTF-8 bytes must not become a
 * mojibake name", `executor/identity.rs`). `fatal: true` is what makes that
 * true here: the default decoder substitutes U+FFFD for every byte it cannot
 * read, and that name was then cached for a day, adopted by the address book
 * as a contact's resolved name, written to storage and drawn in the recipient
 * picker — where nothing ever replaced it (issue 200).
 */
export function decodeString(hex: string): string | null {
	try {
		const data = hex.startsWith('0x') ? hex.slice(2) : hex;
		if (data.length < 128) return null;
		const offset = parseInt(data.slice(0, 64), 16) * 2;
		const strLen = parseInt(data.slice(offset, offset + 64), 16);
		if (strLen === 0 || strLen > 256) return null;
		const strHex = data.slice(offset + 64, offset + 64 + strLen * 2);
		const bytes = new Uint8Array(strLen);
		for (let i = 0; i < strLen; i++) bytes[i] = parseInt(strHex.slice(i * 2, i * 2 + 2), 16);
		// Throws on invalid UTF-8 — caught below, so the service answers "I do
		// not know them" instead of naming them something they are not.
		const name = UTF8_STRICT.decode(bytes).trim();
		return name === '' ? null : name;
	} catch {
		return null;
	}
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Resolve a recipient address to a display identity:
 *   1. local cache · 2. passkey index · 3. name services (all queried in
 *   parallel; first match by priority wins).
 */
export async function resolveRecipientIdentity(address: string): Promise<RecipientIdentity | null> {
	if (!/^0x[0-9a-fA-F]{40}$/.test(address)) return null;
	// The zero address is a mint/burn counterparty (EIP-7708 native events),
	// not a recipient — no identity, and it would 404 the index.
	if (/^0x0{40}$/.test(address)) return null;

	// One waterfall per address at a time. The address book and the activity
	// feed ask about the same counterparty in the same tick on the contacts
	// page; both missed the cache the other had not written yet, and the index
	// and five name services were asked twice (issue 191 made the book ask on
	// load, which is what made the doubling worth a line).
	const key = address.toLowerCase();
	const pending = inflight.get(key);
	if (pending !== undefined) return pending;
	const work = runWaterfall(address).finally(() => inflight.delete(key));
	inflight.set(key, work);
	return work;
}

const inflight = new Map<string, Promise<RecipientIdentity | null>>();

async function runWaterfall(address: string): Promise<RecipientIdentity | null> {
	const cached = await getCache(address);
	if (cached !== undefined) return cached;

	// The passkey index, by address — three hops and its own long-lived cache
	// (`public-key-index.ts`); it never throws.
	//
	// Not forward-verified, and not an omission: a Vela wallet name is a LABEL
	// its owner registered beside their own founding key, proved by the chain's
	// answer about THIS address (`registry_lookup`). It is not a record anything
	// resolves, so it has no forward direction to check — `Unchecked`, and shown.
	const record = await queryWalletName(address);
	if (record !== null) {
		const identity: RecipientIdentity = { name: record.name, source: 'passkey' };
		await setCache(address, identity);
		return identity;
	}

	const results = await Promise.allSettled(
		NAME_SERVICES.map((config) => reverseResolveRegistry(address, config))
	);
	for (let i = 0; i < results.length; i++) {
		const r = results[i];
		if (r.status === 'fulfilled' && r.value) {
			const identity: RecipientIdentity = { name: r.value, source: NAME_SERVICES[i].label };
			await setCache(address, identity);
			return identity;
		}
	}
	return null;
}
