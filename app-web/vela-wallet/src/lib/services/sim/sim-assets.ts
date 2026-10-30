// Ported from src/services/sim-assets.ts @ f9bcb278 — RN seams rewritten to the web modules; logic verbatim.
/* eslint-disable @typescript-eslint/no-explicit-any -- verbatim port: these walk dynamic wire
   shapes (decoded params, eth_simulateV1 results, selector DB rows) that Expo typed `any`;
   narrowing them is a rewrite, not a port (spec 026 results.md, Phase 2). */
/**
 * Asset-change primitives — the pure, network-free core of transaction
 * simulation. Everything here is deterministic and unit-tested in isolation:
 * no RPC, no clock, no storage. The engines (RPC `eth_simulateV1`, optional
 * Tevm) produce raw logs; this module turns logs into signed per-asset deltas
 * for one wallet, and parses revert reasons.
 *
 * Why log-based diffing: a transfer of value emits a `Transfer` event, and
 * `eth_simulateV1` with `traceTransfers` additionally synthesises ERC-20-style
 * `Transfer` logs for *native* value (sender = the magic 0xeee…eee address).
 * Netting those logs for the user's address yields "what moved" without a full
 * state-diff engine. Known limit: a fee-on-transfer / rebasing token reports the
 * gross logged amount, which can differ from what actually lands — the same
 * caveat every log-based simulator (Tenderly/Alchemy) carries.
 */

import { toQuantity } from '$lib/core/kernels';

/**
 * Normalise a call's value to a canonical hex quantity the simulation RPCs
 * accept. dApp-supplied values (e.g. ethers' zero-padded `0x0de0…`) would
 * otherwise be rejected by go-ethereum with "leading zero digits" and surface
 * as a bogus "Expected to fail" in the preview.
 *
 * A malformed value must not take down the whole simulation: losing the preview
 * silently would hide the balance-change and revert warnings on exactly the
 * hostile input the strict shared core exists to catch. Simulate the call as
 * zero-value (what the legacy quantity helper silently did) and let the sheet's
 * own estimate surface the real problem. Both engines share this one policy.
 */
export function simValueParam(value: string | undefined): string {
	try {
		return toQuantity(value);
	} catch {
		console.warn('[sim] malformed call value, simulating as 0:', value);
		return '0x0';
	}
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/** One inner call to simulate (the Safe→target call, mirroring tx-simulation). */
export interface SimCall {
	to: string;
	data?: string;
	value?: string;
}

/** A raw EVM log as returned by `eth_simulateV1` (and how Tevm would shape it). */
export interface SimLog {
	address: string;
	topics: string[];
	data: string;
}

export type AssetKind = 'native' | 'erc20';

/** A signed balance change for one asset, from the simulated wallet's view. */
export interface AssetDelta {
	kind: AssetKind;
	/** Lowercased ERC-20 contract address; undefined for the native coin. */
	token?: string;
	/** Signed change in the asset's smallest unit: positive = received, negative = sent. */
	delta: bigint;
}

/**
 * What every simulation engine (RPC, Tevm) returns. `null` (not this type) is
 * the engines' "can't answer" signal; a real revert is `{ ok: false }`.
 */
export interface EngineResult {
	/** true = the simulated call(s) all succeeded. */
	ok: boolean;
	/** Decoded revert reason for the first failing call, when available. */
	revertReason?: string;
	/** Net per-asset changes for the simulated wallet (empty when nothing moved). */
	deltas: AssetDelta[];
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** keccak256("Transfer(address,address,uint256)") — ERC-20 & ERC-721 Transfer. */
export const TRANSFER_TOPIC = '0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef';

/**
 * keccak256("Deposit(address,uint256)") and keccak256("Withdrawal(address,uint256)")
 * — WETH9's own events (WETH, WXDAI, WMATIC…): wrapping mints the wrapped
 * token with no `Transfer`, unwrapping burns it with none (cs-21: a WXDAI
 * deposit showed "xDAI −1" and never "+1 WXDAI").
 */
export const WETH_DEPOSIT_TOPIC =
	'0xe1fffcc4923d04b559f4d29a8bfc6cda04eb5b0d3c460751c2402c5c5cc9109c';
export const WETH_WITHDRAWAL_TOPIC =
	'0x7fcf532c15f0a6db0bd6d0e038bea71d30d808c7d98cb3bf7268a95bf5081b65';

/**
 * Magic "sender contract" address `eth_simulateV1` uses for synthetic native
 * (ETH/BNB/…) transfer logs when `traceTransfers` is enabled. A log whose
 * `address` is this sentinel is a native-value move, not a token at that
 * address (no real contract lives there).
 */
export const NATIVE_TRANSFER_SENTINEL = '0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee';

const ZERO_ADDRESS = '0x0000000000000000000000000000000000000000';

/**
 * The addresses a chain's OWN native-transfer log comes from (EIP-7708; Arc
 * logs every native move from `0xffff…fffe`) — the core's
 * `NATIVE_LOG_ADDRESSES` beside the traced sentinel above.
 */
const CHAIN_NATIVE_LOG_ADDRESSES = new Set([
	'0xfffffffffffffffffffffffffffffffffffffffe',
	ZERO_ADDRESS
]);
const ERROR_STRING_SELECTOR = '0x08c379a0'; // Error(string)
const PANIC_SELECTOR = '0x4e487b71'; // Panic(uint256)

// ---------------------------------------------------------------------------
// Log → asset deltas
// ---------------------------------------------------------------------------

/** Lowercased last-20-bytes address from a 32-byte indexed topic. */
function topicToAddress(topic: string): string {
	const h = topic.startsWith('0x') ? topic.slice(2) : topic;
	return '0x' + h.slice(-40).toLowerCase();
}

/** First 32-byte word of a log's data as an unsigned bigint (0n if absent/garbage). */
function firstWord(data: string | undefined): bigint {
	if (!data) return 0n;
	const h = data.startsWith('0x') ? data.slice(2) : data;
	if (h.length === 0) return 0n;
	try {
		return BigInt('0x' + h.slice(0, 64));
	} catch {
		return 0n;
	}
}

/**
 * Net the value transfers in `logs` that touch `user`, returning one signed
 * delta per asset (native + ERC-20). Self-transfers cancel to zero and are
 * dropped, as are any assets with a net zero change.
 *
 * Only fungible `Transfer(address,address,uint256)` logs count: those have
 * exactly three topics (the value is in `data`, not indexed). ERC-721 transfers
 * share the signature but index the tokenId as a fourth topic, so the
 * `topics.length === 3` guard excludes NFTs — intentional for the v1 scope.
 * A wrapped coin's `Deposit`/`Withdrawal` (two topics, the account indexed)
 * counts as the mint or burn it is.
 */
export function deriveAssetDeltas(logs: SimLog[], user: string): AssetDelta[] {
	const u = user.toLowerCase();
	// key: 'native' for the native coin, else the lowercased token address.
	const acc = new Map<string, AssetDelta>();
	// r5reg-1 (the core's `derive_deltas`): a traced answer reports every
	// native move from the sentinel; a chain that logs its own (Arc) reports
	// the same move again. With traced logs there, the chain's are the copy;
	// without them (a receipt), the chain's are the record.
	const traced = (logs ?? []).some(
		(log) => (log?.address ?? '').toLowerCase() === NATIVE_TRANSFER_SENTINEL
	);

	for (const log of logs ?? []) {
		const topics = log?.topics;
		if (!topics) continue;
		const topic = (topics[0] ?? '').toLowerCase();
		let from: string;
		let to: string;
		if (topics.length === 3 && topic === TRANSFER_TOPIC) {
			from = topicToAddress(topics[1]);
			to = topicToAddress(topics[2]);
		} else if (topics.length === 2 && topic === WETH_DEPOSIT_TOPIC) {
			from = ZERO_ADDRESS;
			to = topicToAddress(topics[1]);
		} else if (topics.length === 2 && topic === WETH_WITHDRAWAL_TOPIC) {
			from = topicToAddress(topics[1]);
			to = ZERO_ADDRESS;
		} else continue;
		if (from !== u && to !== u) continue; // not our wallet — ignore

		const value = firstWord(log.data);
		if (value === 0n) continue;

		const addrLc = (log.address ?? '').toLowerCase();
		const chainNative = CHAIN_NATIVE_LOG_ADDRESSES.has(addrLc);
		if (chainNative && traced) continue;
		const isNative = addrLc === NATIVE_TRANSFER_SENTINEL || chainNative;
		const key = isNative ? 'native' : addrLc;
		if (!key) continue;

		let entry = acc.get(key);
		if (!entry) {
			entry = {
				kind: isNative ? 'native' : 'erc20',
				token: isNative ? undefined : addrLc,
				delta: 0n
			};
			acc.set(key, entry);
		}
		if (to === u) entry.delta += value;
		if (from === u) entry.delta -= value; // self-transfer (from===to===u) nets to 0
	}

	return [...acc.values()].filter((e) => e.delta !== 0n);
}

// ---------------------------------------------------------------------------
// Display formatting (pure)
// ---------------------------------------------------------------------------

function withCommas(intStr: string, group = ',', indian = false): string {
	if (intStr.length <= 3) return intStr;
	if (!indian) return intStr.replace(/\B(?=(\d{3})+(?!\d))/g, group);
	const head = intStr.slice(0, -3);
	const tail = intStr.slice(-3);
	return head.replace(/\B(?=(\d{2})+(?!\d))/g, group) + group + tail;
}

/**
 * Format an *absolute* smallest-unit amount with up to `maxFrac` significant
 * fractional digits (trailing zeros trimmed). Sign is the caller's job — deltas
 * carry it. Mirrors the token formatting used in clear signing so amounts read
 * consistently across the signing surfaces.
 */
export function formatTokenAmount(
	abs: bigint,
	decimals: number,
	maxFrac = 6,
	sep: { group: string; decimal: string; indian?: boolean } = { group: ',', decimal: '.' }
): string {
	if (abs < 0n) abs = -abs;
	if (abs === 0n) return '0';
	const d = decimals > 0 ? decimals : 0;
	const divisor = 10n ** BigInt(d);
	const whole = abs / divisor;
	const frac = abs % divisor;
	if (frac === 0n || d === 0) return withCommas(whole.toString(), sep.group, sep.indian);

	const fracStr = frac.toString().padStart(d, '0');
	const trimmed = fracStr.slice(0, Math.min(maxFrac, d)).replace(/0+$/, '');
	if (!trimmed) {
		// Amount is non-zero but rounds below display precision — show a small marker.
		return whole === 0n
			? `<0${sep.decimal}${'0'.repeat(Math.max(0, Math.min(maxFrac, d) - 1))}1`
			: withCommas(whole.toString(), sep.group, sep.indian);
	}
	return `${withCommas(whole.toString(), sep.group, sep.indian)}${sep.decimal}${trimmed}`;
}

// ---------------------------------------------------------------------------
// Revert-reason parsing (moved here from tx-simulation so engines can share it)
// ---------------------------------------------------------------------------

/** Extract a human revert reason from a JSON-RPC error (Error(string)/Panic/message). */
export function parseRevertReason(error: any): string | undefined {
	const data: string | undefined =
		typeof error?.data === 'string'
			? error.data
			: typeof error?.data?.data === 'string'
				? error.data.data
				: typeof error?.data?.originalError?.data === 'string'
					? error.data.originalError.data
					: undefined;

	if (data && data.startsWith(ERROR_STRING_SELECTOR) && data.length >= 10 + 128) {
		try {
			const lenHex = data.slice(10 + 64, 10 + 128);
			const len = parseInt(lenHex, 16);
			// Require the declared string bytes to actually be present — a truncated
			// payload (len points past the buffer) must not slice to '' and silently
			// drop the reason.
			if (Number.isFinite(len) && len > 0 && len < 1024 && 10 + 128 + len * 2 <= data.length) {
				const strHex = data.slice(10 + 128, 10 + 128 + len * 2);
				const bytes = strHex.match(/.{1,2}/g)?.map((b) => parseInt(b, 16)) ?? [];
				const s = new TextDecoder().decode(new Uint8Array(bytes)).replace(/ +$/, '').trim();
				if (s) return s;
			}
		} catch {
			/* fall through */
		}
	}

	if (data && data.startsWith(PANIC_SELECTOR)) {
		return 'Panic (arithmetic / assertion failure)';
	}

	const msg = typeof error?.message === 'string' ? error.message : '';
	const cleaned = msg.replace(/^execution reverted:?\s*/i, '').trim();
	// Don't echo a bare "execution reverted" — the caller already says "expected to fail".
	return cleaned && !/^execution reverted$/i.test(msg) ? cleaned : undefined;
}
