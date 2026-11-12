/**
 * RPC & Bundler endpoint pool — WEB, routed by the portable Rust machine.
 *
 * Ported from src/services/rpc-pool.ts @ c13e89d4 (spec 025). Every routing
 * rule — six-tier source scoring, EMA latency, failure cooldown, temp/perm
 * bans, the four-way error classification, the three-pass jittered sweep,
 * the fastest-RPC race behind `X-Rpc-Url`, the all-banned self-rescue and
 * the failed-vs-rate-limited verdict — lives in Rust and is tested there.
 * This file owns exactly two things the core cannot have: the fetch, and the
 * promise the caller is waiting on.
 *
 * ONE module-level session (research D9): the ban map, per-endpoint stats and
 * fastest-RPC winners are facts about the network every caller shares.
 */

import { loadCore } from '$lib/core/client';
import {
	poolShouldFault,
	rpcLatencyMs,
	rpcShouldFail,
	rpcShouldRateLimit
} from './fault-injection';
import { recordNet } from './metrics';
import { noteOwnNodes } from './own-node.svelte';
import {
	collectBundlerUrls,
	collectRpcUrls,
	getBuiltinBundlerUrl,
	NEVER_BANNED,
	type CollectedEndpoint,
	type RPCResponse
} from './rpc-pool-endpoints';
import { readStoredBans } from '$lib/wallet/core/rpc-pool-executor';
import { createRpcPoolSession, type RpcPoolSession } from '$lib/wallet/core/rpc-pool-session';
import type { RpcCallVerdict } from '$lib/core/generated/RpcCallVerdict';
import type { RpcKind } from '$lib/core/generated/RpcKind';
import type { RpcPoolView } from '$lib/core/generated/RpcPoolView';
import type { RpcPoolCallRegistry } from '$lib/wallet/core/rpc-pool-types';

export { getBuiltinBundlerUrl, getLogsRangeCap } from './rpc-pool-endpoints';

/**
 * The pool gave up on a call: every endpoint failed every pass (the core's
 * `RpcCallVerdict::Failed`). `maybeDelivered` is the core's OR over every POST
 * of the call (spec 082 RA1): false only when no POST can have been acted on —
 * which is what lets a submit say "not sent" instead of "may have been sent".
 */
export class PoolFailedError extends Error {
	readonly maybeDelivered: boolean;
	readonly rateLimited: boolean;
	/**
	 * The pool itself failed (its core faulted), not the chain's nodes: the
	 * call never got an answer from anyone outside the app (issue 483). Never
	 * told as "can't reach the chain".
	 */
	readonly internal: boolean;
	/**
	 * Round 3 (send-5): an endpoint answered with an HTTP server error whose
	 * body is a JSON-RPC `error` object — that object, as JSON text. A submit
	 * hands it to the tracker, which judges whether it was a refusal; `null`
	 * when no endpoint said anything of the kind.
	 */
	readonly relayRefusal: string | null;
	constructor(
		message: string,
		facts: {
			maybeDelivered: boolean;
			rateLimited: boolean;
			internal?: boolean;
			relayRefusal?: string | null;
		}
	) {
		super(message);
		this.name = 'PoolFailedError';
		this.maybeDelivered = facts.maybeDelivered;
		this.rateLimited = facts.rateLimited;
		this.internal = facts.internal ?? false;
		this.relayRefusal = facts.relayRefusal ?? null;
	}
}

/**
 * Could an EARLIER endpoint of the call that answered `body` have acted on it
 * too (the core's `Respond.maybe_delivered`)? `false` for a body the pool did
 * not route (a fault-injected answer).
 */
const deliveredMaybe = new WeakMap<object, boolean>();
export function maybeDeliveredOf(body: object): boolean {
	return deliveredMaybe.get(body) ?? false;
}

// ---------------------------------------------------------------------------
// Projected core state
// ---------------------------------------------------------------------------

let coreFailedChains: number[] = [];
let coreRateLimitedChains: number[] = [];
/** The pool's whole latest view — where a hedge goes (perf-6). */
let latestView: RpcPoolView | null = null;

/**
 * Chains whose person's own node is not answering and whose figures come from
 * public nodes for now (owner decision 2026-10-28) — the core's list.
 */
export function getFallbackChains(): readonly number[] {
	return latestView?.fallback_chains ?? [];
}

/**
 * Chains read through the person's own node alone ("Only use my node") that
 * cannot be read now — said as "Can't reach your node for …".
 */
export function getOwnNodeUnreachedChains(): readonly number[] {
	return latestView?.own_node_unreached_chains ?? [];
}

/**
 * r12-3: who hears the pool's out-of-reach networks (the core's
 * `out_of_reach_chains`) each time that set changes — the balance machine,
 * which reads a lost network again at once (`pool_unreached`).
 */
const outOfReachListeners = new Set<(chainIds: number[], silent: number[]) => void>();
let lastOutOfReach = '';

/**
 * Listen for the pool's out-of-reach networks, and of them the silent ones
 * (r13-4: `silent_chains`, named at once); told the current sets at once.
 * Returns the unsubscribe.
 */
export function onPoolOutOfReach(
	listener: (chainIds: number[], silent: number[]) => void
): () => void {
	outOfReachListeners.add(listener);
	const now = latestView?.out_of_reach_chains;
	if (now && now.length > 0) listener([...now], [...(latestView?.silent_chains ?? [])]);
	return () => outOfReachListeners.delete(listener);
}

function noteOutOfReach(view: RpcPoolView): void {
	const ids = view.out_of_reach_chains ?? [];
	const silent = view.silent_chains ?? [];
	const key = `${ids.join(',')}|${silent.join(',')}`;
	if (key === lastOutOfReach) return;
	lastOutOfReach = key;
	// Never from inside the pool core's own dispatch: the next task.
	setTimeout(() => {
		for (const listener of [...outOfReachListeners]) listener([...ids], [...silent]);
	}, 0);
}

/** Fault-injected chains, apart from the projected sets (the view is a full
 *  replacement on every push and would erase a folded-in fault). */
const faultFailedChains = new Set<number>();
const faultRateLimitedChains = new Set<number>();
const faultHardFailedChains = new Set<number>();

/** Chains whose RPC endpoints are all currently failing. */
/**
 * r17-12: whether the pool finds `chainId` failing — failed or out of reach,
 * not merely rate limited (the desktop's `failed_chains ∪ unreached_chains`).
 */
export function chainFailing(chainId: number): boolean {
	const view = latestView;
	if (!view) return false;
	return view.failed_chains.includes(chainId) || (view.out_of_reach_chains ?? []).includes(chainId);
}

export function getFailedRpcChains(): ReadonlySet<number> {
	return new Set([...coreFailedChains, ...faultFailedChains]);
}

/** The transient, self-healing subset — the UI must NOT nag to swap RPCs
 *  for these (`rpc_pool.rs` invariant ④). */
export function getRateLimitedChains(): ReadonlySet<number> {
	const merged = new Set([...coreRateLimitedChains, ...faultRateLimitedChains]);
	for (const chainId of faultHardFailedChains) merged.delete(chainId);
	return merged;
}

// ---------------------------------------------------------------------------
// The calls in flight
// ---------------------------------------------------------------------------

type PendingCall = {
	kind: RpcKind;
	chainId: number;
	method: string;
	resolve: (body: RPCResponse) => void;
	reject: (error: unknown) => void;
	/**
	 * R22: a corroborated read — resolved with BOTH bodies (`settle`), never
	 * early: a hedge's answer would skip the witness (`answerEarly`).
	 */
	corroborated?: (answer: CorroboratedAnswer) => void;
};

/**
 * R22 (review 1-2): what a corroborated read came back with — the first
 * node's body, the second, independent node's (`null`: nobody independent
 * answered) and whether the pool holds the person's own node alone ("Only
 * use my node"). What the two MEAN is the core's: hand them to
 * `rpcCorroboratedEnvelope`, never compare them here.
 */
export interface CorroboratedAnswer {
	body: RPCResponse;
	witnessBody: RPCResponse | null;
	ownNode: boolean;
	/**
	 * §39 (r22-2): nobody independent was asked BECAUSE the pool holds no
	 * other provider for this chain (the verdict's `sole_provider`) —
	 * configuration, not a failure. Carried into the core's envelope.
	 */
	soleProvider: boolean;
}

const payloads = new Map<
	string,
	{ method: string; params: unknown[]; kind: RpcKind; corroborate?: boolean }
>();
const bodies = new Map<string, Map<string, RPCResponse>>();
const pendingCalls = new Map<string, PendingCall>();
const pendingBases = new Map<string, (baseUrl: string) => void>();
const pendingBestRpc = new Map<string, (url: string | null) => void>();
const seeds = new Map<number, { rpc: CollectedEndpoint[]; bundler: CollectedEndpoint[] }>();

let sequence = 0;

function forget(callId: string): void {
	payloads.delete(callId);
	bodies.delete(callId);
	pendingCalls.delete(callId);
}

function settle(callId: string, verdict: RpcCallVerdict): void {
	if (verdict.type === 'bundler_base') {
		const resolve = pendingBases.get(callId);
		pendingBases.delete(callId);
		resolve?.(verdict.base_url ?? getBuiltinBundlerUrl());
		return;
	}

	if (verdict.type === 'best_rpc_url') {
		const resolve = pendingBestRpc.get(callId);
		pendingBestRpc.delete(callId);
		// `null` passes straight through: inventing a fallback here is exactly
		// the wrong-chain hand-off invariant ② forbids.
		resolve?.(verdict.url);
		return;
	}

	const call = pendingCalls.get(callId);
	const held = verdict.type === 'failed' ? undefined : bodies.get(callId)?.get(verdict.url);
	const witness =
		verdict.type === 'respond' && verdict.witness_url
			? bodies.get(callId)?.get(verdict.witness_url)
			: undefined;
	// A failed call's JSON-RPC `error` answers — an HTTP error's body
	// (`refusalBody`) or an error the core routed on — the last one, for a
	// submit to hand on (round 3, send-5). What it proves is the core's.
	const refusal =
		verdict.type === 'failed'
			? [...(bodies.get(callId)?.values() ?? [])]
					.reverse()
					.map((body) => body.error)
					.find((error) => error != null)
			: undefined;
	forget(callId);
	if (!call) return;

	const service = call.kind === 'bundler' ? 'bundler' : 'rpc';
	if (verdict.type === 'failed') {
		recordNet(service, 'final_failure', {
			note: `all endpoints failed: ${call.method} chain ${call.chainId}`
		});
		call.reject(
			new PoolFailedError(
				call.kind === 'bundler'
					? `All bundler endpoints failed for chain ${call.chainId}`
					: `All RPC endpoints failed for chain ${call.chainId}`,
				{
					maybeDelivered: verdict.maybe_delivered,
					rateLimited: verdict.rate_limited,
					relayRefusal: refusal ? JSON.stringify(refusal) : null
				}
			)
		);
		return;
	}

	if (!held) {
		// Unreachable by construction; fail rather than hang if it ever isn't.
		recordNet(service, 'final_failure', {
			note: `verdict without body: ${call.method} chain ${call.chainId}`
		});
		call.reject(new Error(`No response body for chain ${call.chainId}`));
		return;
	}

	recordNet(service, 'success');
	if (verdict.type === 'respond') deliveredMaybe.set(held, verdict.maybe_delivered);
	if (call.corroborated) {
		// R22: the witness's body is kept beside the first (`keepBody`, every
		// POST); the verdict names both.
		const witnessUrl = verdict.type === 'respond' ? (verdict.witness_url ?? null) : null;
		const witnessBody = witnessUrl === null ? null : (witness ?? null);
		call.corroborated({
			body: held,
			witnessBody,
			ownNode: verdict.type === 'respond' && verdict.own_node === true,
			soleProvider: verdict.type === 'respond' && verdict.sole_provider === true
		});
		return;
	}
	call.resolve(held);
}

/**
 * perf-6: a hedge's answer the core calls one every node gives alike. The
 * caller has it now; the payload and bodies stay until the core concludes
 * the call in its own order (`settle`), which then finds no caller waiting.
 */
function answerEarly(callId: string, url: string): void {
	const call = pendingCalls.get(callId);
	const held = bodies.get(callId)?.get(url);
	if (!call || !held) return;
	// R22: a corroborated read is settled by the core alone, with its witness.
	if (call.corroborated || payloads.get(callId)?.corroborate === true) return;
	pendingCalls.delete(callId);
	recordNet('rpc', 'success');
	// A hedged read changes nothing on any node (the core's `HEDGED_READS`).
	deliveredMaybe.set(held, false);
	call.resolve(held);
}

const registry: RpcPoolCallRegistry = {
	payload: (callId) => payloads.get(callId),
	view: () => latestView,
	answerEarly,
	keepBody: (callId, url, body) => {
		const perUrl = bodies.get(callId) ?? new Map<string, RPCResponse>();
		perUrl.set(url, body);
		bodies.set(callId, perUrl);
	},
	settle,
	noteEndpoints: (chainId, rpc, bundler) => {
		seeds.set(chainId, { rpc, bundler });
	},
	noteBackoff: (callId) => {
		const call = pendingCalls.get(callId);
		if (call) recordNet(call.kind === 'bundler' ? 'bundler' : 'rpc', 'retry');
	}
};

/**
 * A core-level fault means the machine may never conclude the calls it was
 * routing; silence would freeze balances and sends alike. Fail them instead.
 */
function abandonInFlight(error: unknown): void {
	const message = error instanceof Error ? error.message : String(error);
	for (const [callId, call] of [...pendingCalls]) {
		forget(callId);
		// Whether a POST of it went out is unknown here — so it may have.
		call.reject(
			new PoolFailedError(`RPC pool fault for chain ${call.chainId}: ${message}`, {
				maybeDelivered: true,
				rateLimited: false,
				internal: true
			})
		);
	}
	for (const [callId, resolve] of [...pendingBases]) {
		pendingBases.delete(callId);
		resolve(getBuiltinBundlerUrl());
	}
	for (const [callId, resolve] of [...pendingBestRpc]) {
		pendingBestRpc.delete(callId);
		resolve(null);
	}
}

// ---------------------------------------------------------------------------
// The one resident session
// ---------------------------------------------------------------------------

let session: RpcPoolSession | null = null;
let ready: Promise<void> | null = null;

function ensureReady(): Promise<void> {
	if (!ready) {
		const booting = (async () => {
			await loadCore();
			const created = createRpcPoolSession({
				onView: (view: RpcPoolView) => {
					latestView = view;
					coreFailedChains = view.failed_chains;
					noteOutOfReach(view);
					coreRateLimitedChains = view.rate_limited_chains;
					noteOwnNodes(
						view.fallback_chains ?? [],
						view.own_only_chains ?? [],
						view.own_node_unreached_chains ?? []
					);
				},
				onError: (error) => {
					console.error('[rpc-pool] core fault:', error);
					abandonInFlight(error);
				},
				registry
			});
			session = created;
			// The persisted ban map must be in the core before the first
			// selection, or a banned endpoint gets one free attempt.
			const entries = await readStoredBans().catch(() => []);
			created.start({ type: 'bans_loaded', entries });
		})();
		// A boot that failed is not kept (issue 483): the next call boots again,
		// so a retry is a real retry rather than the same failure handed back.
		const attempt: Promise<void> = booting.catch((error: unknown) => {
			if (ready === attempt) ready = null;
			throw error;
		});
		ready = attempt;
	}
	return ready;
}

async function route(
	kind: RpcKind,
	method: string,
	params: unknown[],
	chainId: number,
	person = false
): Promise<RPCResponse> {
	await ensureReady();
	const callId = `${kind}:${chainId}:${(sequence += 1)}`;
	payloads.set(callId, { method, params, kind });
	return new Promise<RPCResponse>((resolve, reject) => {
		pendingCalls.set(callId, { kind, chainId, method, resolve, reject });
		dispatchCall(callId, kind, chainId, method, person, false);
	});
}

function dispatchCall(
	callId: string,
	kind: RpcKind,
	chainId: number,
	method: string,
	person: boolean,
	corroborate: boolean
): void {
	session?.dispatch({
		type: 'call_requested',
		call_id: callId,
		chain_id: chainId,
		kind,
		method,
		now_ms: Date.now(),
		// r17-10: the person's own ask (their Try again, their press) —
		// the core sweeps every node, past cooldowns, and marks each POST
		// `must_send` so the skipper never answers it locally.
		person,
		// R22: a read a signing verdict stands on — answered by two
		// independent endpoints, each of which said its chain first.
		corroborate
	});
}

/** The chain's candidate lists — the last core-driven load, else collected. */
async function endpointsFor(
	chainId: number
): Promise<{ rpc: CollectedEndpoint[]; bundler: CollectedEndpoint[] }> {
	const cached = seeds.get(chainId);
	if (cached) return cached;
	const [rpc, bundler] = await Promise.all([
		collectRpcUrls(chainId, NEVER_BANNED),
		collectBundlerUrls(chainId, NEVER_BANNED)
	]);
	const collected = { rpc, bundler };
	seeds.set(chainId, collected);
	return collected;
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * An RPC call with core-decided balancing and failover. `options.person`:
 * the person asked for this read themselves (r17-10, the core's
 * `CallRequested.person`) — it is always actually sent.
 */
export async function poolRpcCall(
	method: string,
	params: unknown[],
	chainId: number,
	attempt = 0,
	options: { person?: boolean } = {}
): Promise<RPCResponse> {
	void attempt; // signature compatibility; passes are the core's now
	const injectedLatency = rpcLatencyMs();
	if (injectedLatency > 0) await new Promise((r) => setTimeout(r, injectedLatency));
	if (rpcShouldFail(chainId)) {
		faultFailedChains.add(chainId);
		faultHardFailedChains.add(chainId);
		faultRateLimitedChains.delete(chainId);
		throw new Error(`[fault] RPC forced to fail for chain ${chainId}`);
	}
	if (poolShouldFault(chainId)) {
		// The pool failed inside the app (issue 483): nothing left the device.
		throw new PoolFailedError(`[fault] RPC pool fault for chain ${chainId}`, {
			maybeDelivered: false,
			rateLimited: false,
			internal: true
		});
	}
	if (rpcShouldRateLimit(chainId)) {
		faultFailedChains.add(chainId);
		faultRateLimitedChains.add(chainId);
		faultHardFailedChains.delete(chainId);
		throw new Error(`[fault] RPC rate-limited for chain ${chainId}`);
	}

	return route('rpc', method, params, chainId, options.person ?? false);
}

/**
 * R22 (review 1-2): a read a signing verdict stands on — the sheet's own
 * simulation, a multisig guard's reads, §31's isOwner. The pool asks a
 * second, independent endpoint the same question (each one checked for its
 * chain id first) and this resolves with both bodies; with "Only use my
 * node", the person's node alone, said as such (`ownNode`). Never hedged,
 * never answered early. Rejects with {@link PoolFailedError} when no node
 * answered — the envelope's `unreachable`.
 */
export async function poolCorroboratedCall(
	method: string,
	params: unknown[],
	chainId: number,
	options: { person?: boolean } = {}
): Promise<CorroboratedAnswer> {
	const injectedLatency = rpcLatencyMs();
	if (injectedLatency > 0) await new Promise((r) => setTimeout(r, injectedLatency));
	if (rpcShouldFail(chainId) || rpcShouldRateLimit(chainId)) {
		throw new PoolFailedError(`[fault] RPC forced to fail for chain ${chainId}`, {
			maybeDelivered: false,
			rateLimited: rpcShouldRateLimit(chainId)
		});
	}
	if (poolShouldFault(chainId)) {
		throw new PoolFailedError(`[fault] RPC pool fault for chain ${chainId}`, {
			maybeDelivered: false,
			rateLimited: false,
			internal: true
		});
	}
	await ensureReady();
	const callId = `rpc:${chainId}:${(sequence += 1)}`;
	payloads.set(callId, { method, params, kind: 'rpc', corroborate: true });
	return new Promise<CorroboratedAnswer>((resolve, reject) => {
		pendingCalls.set(callId, {
			kind: 'rpc',
			chainId,
			method,
			// Never reached: `settle` hands a corroborated read both bodies.
			resolve: (body) => resolve({ body, witnessBody: null, ownNode: false, soleProvider: false }),
			reject,
			corroborated: resolve
		});
		dispatchCall(callId, 'rpc', chainId, method, options.person ?? false, true);
	});
}

/** A bundler call. Since spec 081 the pick is never sent to the relay. */
export async function poolBundlerCall(
	method: string,
	params: unknown[],
	chainId: number,
	retried = false
): Promise<RPCResponse> {
	void retried; // signature compatibility with the Expo surface
	return route('bundler', method, params, chainId);
}

/** True when no user-configured bundler is available for the chain. */
export async function isUsingBuiltinBundler(chainId: number): Promise<boolean> {
	const { bundler } = await endpointsFor(chainId);
	const builtinHost = getBuiltinBundlerUrl();
	return !bundler.some((e) => e.source === 'user' && !e.url.includes(builtinHost));
}

/** REST base of the bundler the pool would submit to (invariant ③). */
export async function getActiveBundlerBaseUrl(chainId: number): Promise<string> {
	await ensureReady();
	const callId = `base:${chainId}:${(sequence += 1)}`;
	return new Promise<string>((resolve) => {
		pendingBases.set(callId, resolve);
		session?.dispatch({
			type: 'bundler_base_requested',
			call_id: callId,
			chain_id: chainId,
			now_ms: Date.now()
		});
	});
}

/** The pool's own ranking — used locally for fork seeding (spec 081: not sent). */
export async function getChainRpcUrl(chainId: number): Promise<string | null> {
	await ensureReady();
	const callId = `rpc-url:${chainId}:${(sequence += 1)}`;
	return new Promise<string | null>((resolve) => {
		pendingBestRpc.set(callId, resolve);
		session?.dispatch({
			type: 'best_rpc_url_requested',
			call_id: callId,
			chain_id: chainId,
			now_ms: Date.now()
		});
	});
}

/** Force refresh a chain's pool (config re-read, winner dropped). */
export async function refreshPool(chainId: number): Promise<void> {
	await ensureReady();
	seeds.delete(chainId);
	session?.dispatch({ type: 'refresh_chain', chain_id: chainId });
}

/** Invalidate all pools so they re-read config on next use. */
export function invalidateAllPools(): void {
	seeds.clear();
	session?.dispatch({ type: 'invalidate_all' });
}
