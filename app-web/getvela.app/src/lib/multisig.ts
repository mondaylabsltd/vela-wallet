/**
 * Read the Safe Genesis Registry — Vela's multisig accounts — one page at a
 * time, straight from public RPCs.
 *
 * A record is the creation call of one Safe (factory, singleton, `setup(...)`
 * initializer, salt nonce) filed under the address it creates, plus a name.
 * Everything in it is untrusted display data: anyone may write a record, the
 * name is whatever the first writer chose (until the Safe renames itself), and
 * the initializer is only bytes until it decodes as a Safe `setup` call.
 *
 * Pure decode + paging here; the network goes through the viem client passed in.
 */
import { decodeFunctionData, getAddress, type Hex, type PublicClient } from 'viem';
import { GENESIS_REGISTRY_ABI } from './chain';
import { GENESIS_REGISTRY_ADDRESS } from './registry-chains';
import { cleanUntrusted } from './untrusted-text';

const ZERO = '0x0000000000000000000000000000000000000000';
/** Safe's linked-list sentinel: never a valid owner. */
const SENTINEL = '0x0000000000000000000000000000000000000001';

// ── The initializer ──────────────────────────────────────────────────────────

export const SAFE_SETUP_ABI = [
	{
		type: 'function',
		name: 'setup',
		stateMutability: 'nonpayable',
		inputs: [
			{ name: 'owners', type: 'address[]' },
			{ name: 'threshold', type: 'uint256' },
			{ name: 'to', type: 'address' },
			{ name: 'data', type: 'bytes' },
			{ name: 'fallbackHandler', type: 'address' },
			{ name: 'paymentToken', type: 'address' },
			{ name: 'payment', type: 'uint256' },
			{ name: 'paymentReceiver', type: 'address' }
		],
		outputs: []
	}
] as const;

/** `setup(address[],uint256,address,bytes,address,address,uint256,address)`. */
export const SAFE_SETUP_SELECTOR = '0xb63e800d';

export interface DecodedSetup {
	ok: true;
	owners: `0x${string}`[];
	threshold: bigint;
	/** Setup hook: a delegatecall the Safe makes during setup (zero = none). */
	to: `0x${string}`;
	data: Hex;
	fallbackHandler: `0x${string}`;
	paymentToken: `0x${string}`;
	payment: bigint;
	paymentReceiver: `0x${string}`;
	/** Reasons Safe's own `setup` would reject this, in plain words. Empty when
	 *  it is a well-formed M-of-N. Shown, never hidden: a record can be filed
	 *  for a setup no Safe could ever have run. */
	problems: string[];
}

export interface UndecodedSetup {
	ok: false;
	/** Why it is not shown as owners + threshold. */
	reason: string;
	/** The first four bytes, or null when there are fewer. */
	selector: Hex | null;
	byteLength: number;
}

export type SetupInfo = DecodedSetup | UndecodedSetup;

function byteLength(hex: string): number {
	return Math.max(0, Math.floor((hex.length - 2) / 2));
}

/** Decode a Safe `setup(...)` initializer. Never throws: anything it cannot
 *  read as a Safe setup comes back as the raw facts (selector, length). */
export function decodeSetup(initializer: Hex): SetupInfo {
	const len = byteLength(initializer ?? '0x');
	const selector = len >= 4 ? (initializer.slice(0, 10).toLowerCase() as Hex) : null;
	if (selector !== SAFE_SETUP_SELECTOR) {
		return {
			ok: false,
			reason: selector ? 'Not a Safe setup() call' : 'Too short to be a call',
			selector,
			byteLength: len
		};
	}
	try {
		const { args } = decodeFunctionData({ abi: SAFE_SETUP_ABI, data: initializer });
		const [owners, threshold, to, data, fallbackHandler, paymentToken, payment, paymentReceiver] =
			args;
		const list = owners.map((o) => getAddress(o));
		return {
			ok: true,
			owners: list,
			threshold,
			to: getAddress(to),
			data,
			fallbackHandler: getAddress(fallbackHandler),
			paymentToken: getAddress(paymentToken),
			payment,
			paymentReceiver: getAddress(paymentReceiver),
			problems: setupProblems(list, threshold)
		};
	} catch {
		return {
			ok: false,
			reason: 'Safe setup() call that does not decode',
			selector,
			byteLength: len
		};
	}
}

function setupProblems(owners: readonly string[], threshold: bigint): string[] {
	const problems: string[] = [];
	if (owners.length === 0) problems.push('no owners');
	if (threshold < 1n) problems.push('threshold is 0');
	else if (threshold > BigInt(owners.length)) problems.push('threshold exceeds the owner count');
	const seen = new Set<string>();
	for (const o of owners) {
		const k = o.toLowerCase();
		if (k === ZERO || k === SENTINEL) problems.push(`invalid owner ${o}`);
		else if (seen.has(k)) problems.push(`duplicate owner ${o}`);
		seen.add(k);
	}
	return problems;
}

// ── The name ─────────────────────────────────────────────────────────────────

export interface DecodedName {
	/** Display text: decoded, hidden characters removed. May be empty. */
	text: string;
	/** False when the bytes are not valid UTF-8 (the contract does not check):
	 *  `text` then carries replacement characters and `raw` should be shown. */
	utf8: boolean;
	/** True when invisible or bidi-control characters were removed. */
	hadHidden: boolean;
	raw: Hex;
}

function hexToBytes(hex: Hex): Uint8Array {
	const body = hex.slice(2);
	const out = new Uint8Array(Math.floor(body.length / 2));
	for (let i = 0; i < out.length; i++) out[i] = parseInt(body.slice(i * 2, i * 2 + 2), 16);
	return out;
}

export function decodeName(raw: Hex): DecodedName {
	const bytes = hexToBytes(raw ?? '0x');
	let decoded: string;
	let utf8 = true;
	try {
		decoded = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
	} catch {
		decoded = new TextDecoder('utf-8').decode(bytes);
		utf8 = false;
	}
	const { text, hadHidden } = cleanUntrusted(decoded);
	return { text, utf8, hadHidden, raw };
}

// ── Known Safe contracts (display only) ──────────────────────────────────────

/** Canonical Safe deployments, by lowercase address — a label to help a reader,
 *  not a check: the same address can hold other code on another chain. */
const KNOWN: Record<string, string> = Object.fromEntries(
	Object.entries({
		'0xd9Db270c1B5E3Bd161E8c8503c55cEABeE709552': 'Safe 1.3.0',
		'0x3E5c63644E683549055b9Be8653de26E0B4CD36E': 'SafeL2 1.3.0',
		'0x69f4D1788e39c87893C980c06EdF4b7f686e2938': 'Safe 1.3.0 (eip155)',
		'0xfb1bffC9d739B8D520DaF37dF666da4C687191EA': 'SafeL2 1.3.0 (eip155)',
		'0x41675C099F32341bf84BFc5382aF534df5C7461a': 'Safe 1.4.1',
		'0x29fcB43b46531BcA003ddC8FCB67FFE91900C762': 'SafeL2 1.4.1',
		'0xFf51A5898e281Db6DfC7855790607438dF2ca44b': 'Safe 1.5.0',
		'0xEdd160fEBBD92E350D4D398fb636302fccd67C7e': 'SafeL2 1.5.0',
		'0xa6B71E26C5e0845f74c812102Ca7114b6a896AB2': 'SafeProxyFactory 1.3.0',
		'0xC22834581EbC8527d974F8a1c97E1bEA4EF910BC': 'SafeProxyFactory 1.3.0 (eip155)',
		'0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67': 'SafeProxyFactory 1.4.1',
		'0x14F2982D601c9458F93bd70B218933A6f8165e7b': 'SafeProxyFactory 1.5.0',
		'0xf48f2B2d2a534e402487b3ee7C18c33Aec0Fe5e4': 'CompatibilityFallbackHandler 1.3.0',
		'0x017062a1dE2FE6b99BE3d9d37841FeD19F573804': 'CompatibilityFallbackHandler 1.3.0 (eip155)',
		'0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99': 'CompatibilityFallbackHandler 1.4.1',
		'0x3EfCBb83A4A7AfcB4F68D501E2c2203a38be77f4': 'CompatibilityFallbackHandler 1.5.0'
	}).map(([a, n]) => [a.toLowerCase(), n])
);

export function knownContract(address: string): string | null {
	return KNOWN[address.toLowerCase()] ?? null;
}

// ── Records ──────────────────────────────────────────────────────────────────

interface RawGenesis {
	factory: `0x${string}`;
	singleton: `0x${string}`;
	renamedAt: bigint;
	saltNonce: bigint;
	initializer: Hex;
	proxyCreationCodeHash: Hex;
	recorder: `0x${string}`;
	recordedAt: bigint;
	index: number;
	name: Hex;
}

export interface MultisigRecord {
	/** Checksummed Safe address — the record's identity. */
	safe: `0x${string}`;
	/** Position in this chain's write order, from 0 (`safeAt(index)`). */
	index: number;
	/** The current name: `rename` overwrites the stored one, so the record as
	 *  read is authoritative — no event replay needed. */
	name: DecodedName;
	/** True when the name is the Safe's own: it renamed itself, or it wrote the
	 *  record itself. Otherwise it is the first writer's label, unverified. */
	selfNamed: boolean;
	/** Block of the Safe's last rename, 0 if never. */
	renamedAt: bigint;
	recorder: `0x${string}`;
	/** Block number of the write. */
	recordedAt: bigint;
	factory: `0x${string}`;
	singleton: `0x${string}`;
	saltNonce: bigint;
	proxyCreationCodeHash: Hex;
	initializer: Hex;
	setup: SetupInfo;
}

export function formatGenesis(safe: string, g: RawGenesis): MultisigRecord {
	const address = getAddress(safe);
	const recorder = getAddress(g.recorder);
	return {
		safe: address,
		index: Number(g.index),
		name: decodeName(g.name),
		selfNamed: g.renamedAt > 0n || recorder === address,
		renamedAt: g.renamedAt,
		recorder,
		recordedAt: g.recordedAt,
		factory: getAddress(g.factory),
		singleton: getAddress(g.singleton),
		saltNonce: g.saltNonce,
		proxyCreationCodeHash: g.proxyCreationCodeHash,
		initializer: g.initializer,
		setup: decodeSetup(g.initializer)
	};
}

// ── Paging ───────────────────────────────────────────────────────────────────

/**
 * Records per `genesisPage` call. A page's `eth_call` cost grows with its
 * records' initializers (~5.7M gas for 100 typical ones, ~7M for ONE of the
 * 8 KiB maximum), and anyone can write a maximal record — so a 100-row page is
 * read in slices that stay well under public nodes' gas caps, and a slice that
 * still fails is retried one record at a time.
 */
export const SLICE = 20;

/** The `(offset, limit)` slices that make up page `page` (1-based) of `size`. */
export function pageSlices(
	page: number,
	size: number,
	slice = SLICE
): { offset: number; limit: number }[] {
	const start = Math.max(0, (Math.max(1, page) - 1) * size);
	const out: { offset: number; limit: number }[] = [];
	for (let o = 0; o < size; o += slice)
		out.push({ offset: start + o, limit: Math.min(slice, size - o) });
	return out;
}

export interface MultisigPage {
	total: number;
	records: MultisigRecord[];
}

type PageResult = readonly [bigint, readonly `0x${string}`[], readonly RawGenesis[]];

function readSlice(client: PublicClient, offset: number, limit: number, desc: boolean) {
	return client.readContract({
		address: GENESIS_REGISTRY_ADDRESS,
		abi: GENESIS_REGISTRY_ABI,
		functionName: 'genesisPage',
		args: [BigInt(offset), BigInt(limit), desc]
	}) as Promise<PageResult>;
}

/** One slice; if the node refuses it (gas cap, size), the same positions one
 *  record per call. A record-by-record failure is a real failure: it throws. */
async function readSliceResilient(
	client: PublicClient,
	offset: number,
	limit: number,
	desc: boolean
): Promise<PageResult[]> {
	try {
		return [await readSlice(client, offset, limit, desc)];
	} catch (e) {
		if (limit <= 1) throw e;
		return Promise.all(
			Array.from({ length: limit }, (_, i) => readSlice(client, offset + i, 1, desc))
		);
	}
}

/**
 * Fetch one page (1-based) of multisig records, newest first when `desc`.
 * Throws when the chain cannot be read — the caller says so with a retry, it
 * never shows an empty list for a failed read.
 *
 * Records written while the slices are in flight can shift a newest-first
 * window by one; rows are de-duplicated by write-order index and re-sorted, so
 * the worst case is a page one row short, never a doubled row.
 */
export async function fetchMultisigPage(
	client: PublicClient,
	page: number,
	size: number,
	desc: boolean
): Promise<MultisigPage> {
	const results = (
		await Promise.all(
			pageSlices(page, size).map((s) => readSliceResilient(client, s.offset, s.limit, desc))
		)
	).flat();

	let total = 0;
	const byIndex = new Map<number, MultisigRecord>();
	for (const [t, safes, records] of results) {
		total = Math.max(total, Number(t));
		safes.forEach((safe, i) => {
			const raw = records[i];
			if (!raw) return;
			const rec = formatGenesis(safe, raw);
			if (!byIndex.has(rec.index)) byIndex.set(rec.index, rec);
		});
	}
	const records = [...byIndex.values()].sort((a, b) =>
		desc ? b.index - a.index : a.index - b.index
	);
	return { total, records };
}

// ── Is the registry there at all? ───────────────────────────────────────────

export type Deployment = 'deployed' | 'not-deployed';

/**
 * Whether the registry contract has code on this client's chain. Only a
 * well-formed `"0x"` answer counts as "not deployed"; anything malformed (some
 * public nodes answer errors inside `result`) throws, so a broken RPC reads as
 * "couldn't check", never as "not deployed".
 */
export async function registryDeployment(
	client: PublicClient,
	address: `0x${string}` = GENESIS_REGISTRY_ADDRESS
): Promise<Deployment> {
	return (await hasCode(client, address)) ? 'deployed' : 'not-deployed';
}

/** Whether `safe` has a record in this chain's registry. */
export function isRecordedOn(client: PublicClient, safe: `0x${string}`): Promise<boolean> {
	return client.readContract({
		address: GENESIS_REGISTRY_ADDRESS,
		abi: GENESIS_REGISTRY_ABI,
		functionName: 'isRecorded',
		args: [safe]
	}) as Promise<boolean>;
}

/** Whether any contract lives at `address` on this chain. Only a well-formed
 *  `"0x"` answer is "no"; a malformed answer throws. */
export async function hasCode(client: PublicClient, address: `0x${string}`): Promise<boolean> {
	const code = await client.request({ method: 'eth_getCode', params: [address, 'latest'] });
	if (typeof code !== 'string' || !/^0x([0-9a-fA-F]{2})*$/.test(code)) {
		throw new Error('malformed eth_getCode answer');
	}
	return code !== '0x';
}

/** Block timestamps (unix ms) for the given blocks; a block that cannot be read
 *  is simply absent — the row then shows its block number instead of a date. */
export async function blockTimes(
	client: PublicClient,
	blocks: bigint[]
): Promise<Map<string, number>> {
	const unique = [...new Set(blocks.map(String))];
	const settled = await Promise.allSettled(
		unique.map((b) => client.getBlock({ blockNumber: BigInt(b) }))
	);
	const out = new Map<string, number>();
	settled.forEach((s, i) => {
		if (s.status !== 'fulfilled') return;
		// Untrusted RPC data: keep it inside the valid Date range.
		const secs = Number(s.value.timestamp);
		if (Number.isFinite(secs) && secs > 0 && secs < 8_640_000_000_000)
			out.set(unique[i], secs * 1000);
	});
	return out;
}
