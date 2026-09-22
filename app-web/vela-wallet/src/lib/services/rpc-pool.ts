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
import { rpcLatencyMs, rpcShouldFail, rpcShouldRateLimit } from './fault-injection';
import { recordNet } from './metrics';
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

// ---------------------------------------------------------------------------
// Projected core state
// ---------------------------------------------------------------------------

let coreFailedChains: number[] = [];
let coreRateLimitedChains: number[] = [];

/** Fault-injected chains, apart from the projected sets (the view is a full
 *  replacement on every push and would erase a folded-in fault). */
const faultFailedChains = new Set<number>();
const faultRateLimitedChains = new Set<number>();
const faultHardFailedChains = new Set<number>();

/** Chains whose RPC endpoints are all currently failing. */
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
};

const payloads = new Map<string, { method: string; params: unknown[] }>();
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
	forget(callId);
	if (!call) return;

	const service = call.kind === 'bundler' ? 'bundler' : 'rpc';
	if (verdict.type === 'failed') {
		recordNet(service, 'final_failure', {
			note: `all endpoints failed: ${call.method} chain ${call.chainId}`
		});
		call.reject(
			new Error(
				call.kind === 'bundler'
					? `All bundler endpoints failed for chain ${call.chainId}`
					: `All RPC endpoints failed for chain ${call.chainId}`
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
	call.resolve(held);
}

const registry: RpcPoolCallRegistry = {
	payload: (callId) => payloads.get(callId),
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
		call.reject(new Error(`RPC pool fault for chain ${call.chainId}: ${message}`));
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
		ready = (async () => {
			await loadCore();
			const created = createRpcPoolSession({
				onView: (view: RpcPoolView) => {
					coreFailedChains = view.failed_chains;
					coreRateLimitedChains = view.rate_limited_chains;
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
	}
	return ready;
}

async function route(
	kind: RpcKind,
	method: string,
	params: unknown[],
	chainId: number
): Promise<RPCResponse> {
	await ensureReady();
	const callId = `${kind}:${chainId}:${(sequence += 1)}`;
	payloads.set(callId, { method, params });
	return new Promise<RPCResponse>((resolve, reject) => {
		pendingCalls.set(callId, { kind, chainId, method, resolve, reject });
		session?.dispatch({
			type: 'call_requested',
			call_id: callId,
			chain_id: chainId,
			kind,
			method,
			now_ms: Date.now()
		});
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

/** An RPC call with core-decided balancing and failover. */
export async function poolRpcCall(
	method: string,
	params: unknown[],
	chainId: number,
	attempt = 0
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
	if (rpcShouldRateLimit(chainId)) {
		faultFailedChains.add(chainId);
		faultRateLimitedChains.add(chainId);
		faultHardFailedChains.delete(chainId);
		throw new Error(`[fault] RPC rate-limited for chain ${chainId}`);
	}

	return route('rpc', method, params, chainId);
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
