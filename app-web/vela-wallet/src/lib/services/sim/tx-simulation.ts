// Ported from src/services/tx-simulation.ts @ f9bcb278 — RN seams rewritten to the web modules; logic verbatim.
/**
 * Client-side transaction simulation.
 *
 * Two layers, both driven by the user's own RPC pool — no third-party
 * "simulation" service, no new dependency:
 *
 *   1. `simulateCall`  — the revert pre-check. A single `eth_call` of the inner
 *      Safe→target call against live state: does it revert, and why. Conservative
 *      by design: an unknown/network failure returns `null` ("no info"), never a
 *      false "will fail".
 *
 *   2. `simulateAssetChanges` — the richer balance-change simulation. Runs the
 *      inner call(s) through a pluggable engine (primary: `eth_simulateV1`;
 *      optional fallback: a local Tevm fork) and reports the wallet's net asset
 *      deltas (native + ERC-20), enriched with on-chain symbol/decimals. It still
 *      carries the revert signal, and degrades to layer (1) when no engine can
 *      compute changes — so callers always get at least as much as before.
 *
 * Because Vela settles gas through a 4337 bundler/gas-account (not from the
 * Safe's native balance), the native delta of the inner call is pure value flow
 * with no gas noise — cleaner than simulating an EOA transaction.
 */
import { poolRpcCall } from '../rpc-pool';
// The asymmetric-trust judgment is a platform seam (spec 017, G7): native uses
// the TypeScript rule this file used to hold inline, web decides it in the
// `token_trust` core. Same signature, same conservative fallbacks.
import { enrichDeltas } from './sim-trust';
import { parseRevertReason, simValueParam, type SimCall } from './sim-assets';
import { rpcSimulate } from './sim-engine-rpc';
import { tevmSimulate } from './sim-engine-tevm';
import type { TrustSimDirection } from '$lib/core/generated/TrustSimDirection';

// Re-exported so existing import sites (and tests) keep their path.
export { parseRevertReason } from './sim-assets';
export type { SimCall, AssetDelta, AssetKind } from './sim-assets';

export interface SimResult {
	/** true = expected to succeed, false = expected to revert. */
	ok: boolean;
	/** Decoded revert reason when ok === false and one was available. */
	revertReason?: string;
}

/** Which engine produced an asset-change result (or `none` when degraded). */
export type SimEngine = 'rpc' | 'tevm' | 'none';

/** A net balance change for one asset, ready for display. */
export interface AssetChange {
	kind: 'native' | 'erc20';
	/** Lowercased ERC-20 address; undefined for the native coin. */
	token?: string;
	/**
	 * Signed smallest-unit change: positive = received, negative = sent.
	 * ABSENT for an unverified token (PR 3): its figure is whatever the site
	 * being signed for chose to emit, the core hands none over, and what is
	 * not here cannot be printed. Such a change says only its `direction`.
	 */
	delta?: bigint;
	/**
	 * An unverified token's direction, the core's (`TrustSimDirection`) — all
	 * that may be said of it: `in` "+", `out` "−", `unreadable` neither. Absent
	 * on every change that carries a `delta`.
	 */
	direction?: TrustSimDirection;
	/** Display symbol (native symbol, or on-chain ERC-20 symbol). */
	symbol?: string;
	/** Decimals for formatting `delta`. */
	decimals?: number;
	/** True when ERC-20 symbol/decimals couldn't be verified on-chain → show with caution. */
	unverified?: boolean;
}

/** Result of `simulateAssetChanges` — a superset of `SimResult`. */
export interface AssetSimResult extends SimResult {
	/**
	 * Net asset changes, or `null` when no engine could compute them (the result
	 * then degrades to a pure revert signal). An empty array means "ran, nothing
	 * moved" (e.g. an approval) — distinct from `null`.
	 */
	changes: AssetChange[] | null;
	engine: SimEngine;
	/**
	 * True when the sim shows a native outflow larger than the wallet's real
	 * balance — the preview looks successful but the transfer can't be funded.
	 */
	underfundedNative?: boolean;
}

/**
 * JSON-safe form of `AssetSimResult`, persisted on a signing record so the
 * "what moved" preview can be replayed from history. Identical shape except each
 * `delta` bigint is a decimal string (AsyncStorage holds JSON, not bigints) —
 * and, like the change it stores, an unverified token's has none (PR 3).
 */
export interface StoredAssetSim {
	ok: boolean;
	revertReason?: string;
	underfundedNative?: boolean;
	engine: SimEngine;
	changes: (Omit<AssetChange, 'delta'> & { delta?: string })[] | null;
}

/** Capture a live sim into its persistable form (bigint delta → decimal string). */
export function serializeAssetSim(r: AssetSimResult): StoredAssetSim {
	return {
		ok: r.ok,
		revertReason: r.revertReason,
		underfundedNative: r.underfundedNative,
		engine: r.engine,
		changes: r.changes
			? r.changes.map(({ delta, ...change }) =>
					delta === undefined ? change : { ...change, delta: delta.toString() }
				)
			: null
	};
}

/**
 * Rehydrate a persisted sim back into the shape `BalanceChangePreview` renders.
 *
 * Both shapes of an unverified token's change are read (PR 3): one stored
 * since carries its `direction` and no figure; one stored before carries the
 * simulation's raw `delta`, which is read for its sign and dropped — the
 * figure does not come back with it.
 */
export function deserializeAssetSim(s: StoredAssetSim): AssetSimResult {
	return {
		ok: s.ok,
		revertReason: s.revertReason,
		underfundedNative: s.underfundedNative,
		engine: s.engine,
		changes: s.changes ? s.changes.map(storedChange) : null
	};
}

function storedChange({
	delta,
	...change
}: NonNullable<StoredAssetSim['changes']>[number]): AssetChange {
	if (!change.unverified) return { ...change, delta: safeBigInt(delta ?? '') };
	return { ...change, direction: change.direction ?? storedDirection(delta) };
}

/** The direction an older record's stored `delta` had; a figure that does not read has none. */
function storedDirection(delta: string | undefined): TrustSimDirection {
	// A signed run of decimal digits, as the simulation wrote it; anything
	// else has no direction to state (`BigInt` alone would read "" as zero).
	if (delta === undefined || !/^[+-]?[0-9]+$/.test(delta.trim())) return 'unreadable';
	const value = BigInt(delta.trim());
	return value > 0n ? 'in' : value < 0n ? 'out' : 'still';
}

/** Parse a stored decimal delta back to bigint; a corrupt value reads as 0. */
function safeBigInt(v: string): bigint {
	try {
		return BigInt(v);
	} catch {
		return 0n;
	}
}

/**
 * Simulate the inner Safe→target call. Returns null when the result is unknown
 * (RPC unreachable) — callers must treat null as "no info", never as failure.
 */
export async function simulateCall(
	from: string,
	to: string,
	data: string | undefined,
	value: string | undefined,
	chainId: number
): Promise<SimResult | null> {
	if (!to) return null;
	try {
		const res = await poolRpcCall(
			'eth_call',
			[
				{ from, to, data: data && data !== '0x' ? data : '0x', value: simValueParam(value) },
				'latest'
			],
			chainId
		);
		if (res?.error) {
			// The endpoint responded with an execution error — a genuine revert.
			return { ok: false, revertReason: parseRevertReason(res.error) };
		}
		return { ok: true };
	} catch {
		// Every endpoint failed (network) — unknown, not a revert.
		return null;
	}
}

/**
 * Simulate one or more inner calls and report the wallet's net asset changes.
 *
 * `from` is the Safe (the wallet whose balances we track). `calls` is the inner
 * Safe→target call(s) — pass one for a single tx, or several for a batch
 * (executed sequentially, sharing state, like MultiSend).
 *
 * Returns `null` only when nothing at all could be learned (no engine, and the
 * revert pre-check was also unreachable). Otherwise `changes` is the asset
 * deltas (possibly `null` if only the revert signal was available).
 */
export async function simulateAssetChanges(
	from: string,
	calls: SimCall[],
	chainId: number
): Promise<AssetSimResult | null> {
	if (!from || calls.length === 0 || !calls[0]?.to) return null;

	// 1) Primary engine: eth_simulateV1.
	let engineRes = await rpcSimulate(from, calls, chainId);
	let engine: SimEngine = engineRes ? 'rpc' : 'none';

	// 2) Optional fallback: local Tevm fork (no-op unless explicitly enabled).
	if (!engineRes) {
		engineRes = await tevmSimulate(from, calls, chainId);
		if (engineRes) engine = 'tevm';
	}

	// 3) Degrade to the revert-only pre-check — still better than nothing.
	if (!engineRes) {
		const c = calls[0];
		const rev = await simulateCall(from, c.to, c.data, c.value, chainId);
		if (!rev) return null; // fully unknown
		return { ok: rev.ok, revertReason: rev.revertReason, changes: null, engine: 'none' };
	}

	const changes = await enrichDeltas(engineRes.deltas, chainId, from);
	// `validation:false` lets the sim move native value the Safe doesn't actually
	// hold, so a successful-looking preview can still fail on-chain for lack of
	// funds. When the sim reports a native outflow, cross-check the real balance.
	const underfundedNative = engineRes.ok ? await nativeUnderfunded(from, changes, chainId) : false;
	return {
		ok: engineRes.ok,
		revertReason: engineRes.revertReason,
		changes,
		engine,
		...(underfundedNative ? { underfundedNative: true } : {})
	};
}

/**
 * True when the simulated native outflow exceeds the wallet's real native
 * balance — i.e. the preview shows success but the transfer can't actually be
 * funded. Returns false (don't warn) whenever the balance is unknown.
 */
async function nativeUnderfunded(
	from: string,
	changes: AssetChange[],
	chainId: number
): Promise<boolean> {
	// The native coin's change always carries its figure; only an unverified
	// token's has none.
	const out = changes
		.map((c) => (c.kind === 'native' ? c.delta : undefined))
		.find((delta) => delta !== undefined && delta < 0n);
	if (out === undefined) return false;
	try {
		const res = await poolRpcCall('eth_getBalance', [from, 'latest'], chainId);
		if (res?.error || typeof res?.result !== 'string') return false; // unknown → don't warn
		return BigInt(res.result) < -out;
	} catch {
		return false;
	}
}
