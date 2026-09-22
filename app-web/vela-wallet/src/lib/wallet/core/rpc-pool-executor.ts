/**
 * The only place the `rpc_pool` core touches the outside world — WEB.
 *
 * Ported from src/services/wallet-state-core/rpc-pool-executor.ts @ c13e89d4
 * (spec 025). Execution here, meaning in Rust: this file performs the fetch,
 * applies the timeout, reads the clock, draws the jitter
 * and writes the ban map — and never decides what any of it means. Which
 * endpoint next, ban vs cool-down vs deliver, 401 vs 429, permanent vs
 * transient vs range cap, how many passes: all the core's.
 *
 * Ban map: the core speaks `RpcBanEntry` (`banned_at_ms`); the store holds
 * `{url, bannedAt, permanent}` under `vela.rpc.banned` — the same key and
 * bytes every client shares.
 *
 * Failure contract: nothing rejects; a failed POST is a `network` outcome —
 * one more endpoint failure for the core to route around.
 */

import { getItem, setItem } from '$lib/services/storage';
import {
	BANNED_STORAGE_KEY,
	collectBundlerUrls,
	collectRpcUrls,
	NEVER_BANNED,
	type RPCResponse
} from '$lib/services/rpc-pool-endpoints';

import type { RpcBanEntry } from '$lib/core/generated/RpcBanEntry';
import type { RpcShellResult } from '$lib/core/generated/RpcShellResult';
import type { RpcTransportOutcome } from '$lib/core/generated/RpcTransportOutcome';
import type { RpcPoolCallRegistry, RpcPoolEffect } from './rpc-pool-types';

// ---------------------------------------------------------------------------
// Ban map storage codec
// ---------------------------------------------------------------------------

type StoredBan = { url: string; bannedAt: number; permanent: boolean };

/** A codec, not a policy: junk coerces; a record without a URL bans nothing. */
function decodeBan(raw: unknown): RpcBanEntry | null {
	if (raw === null || typeof raw !== 'object') return null;
	const record = raw as Record<string, unknown>;
	if (typeof record.url !== 'string' || record.url === '') return null;
	const bannedAt = Number(record.bannedAt);
	return {
		url: record.url,
		banned_at_ms: Number.isFinite(bannedAt) ? bannedAt : 0,
		permanent: record.permanent === true
	};
}

function encodeBan(entry: RpcBanEntry): StoredBan {
	return { url: entry.url, bannedAt: entry.banned_at_ms, permanent: entry.permanent };
}

/** Absent/unparseable/non-array reads as "no bans", as `loadBans()` always did. */
export async function readStoredBans(): Promise<RpcBanEntry[]> {
	try {
		const raw = await getItem(BANNED_STORAGE_KEY);
		if (!raw) return [];
		const parsed: unknown = JSON.parse(raw);
		if (!Array.isArray(parsed)) return [];
		return parsed.map(decodeBan).filter((entry): entry is RpcBanEntry => entry !== null);
	} catch {
		return [];
	}
}

// ---------------------------------------------------------------------------
// Transport
// ---------------------------------------------------------------------------

/** A chain id must survive the trip through the core's `u32`. */
function asChainId(value: number): number | null {
	return Number.isInteger(value) && value >= 0 && value <= 0xffff_ffff ? value : null;
}

type PostResult = { outcome: RpcTransportOutcome; body?: RPCResponse };

/** One JSON-RPC POST, classified mechanically; every decision left to Rust. */
async function post(
	url: string,
	method: string,
	params: unknown[],
	extraHeaders: Record<string, string> | undefined,
	timeoutMs: number
): Promise<PostResult> {
	const controller = new AbortController();
	let timedOut = false;
	const timer = setTimeout(() => {
		timedOut = true;
		controller.abort();
	}, timeoutMs);

	try {
		const res = await fetch(url, {
			method: 'POST',
			headers: { 'Content-Type': 'application/json', ...extraHeaders },
			body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
			signal: controller.signal
		});

		if (!res.ok) return { outcome: { type: 'http_error', status: res.status } };

		const contentType = res.headers.get('content-type') ?? '';
		if (!contentType.includes('json')) return { outcome: { type: 'non_json' } };

		const json: unknown = await res.json();
		if (!json || typeof json !== 'object') return { outcome: { type: 'non_json' } };

		const body = json as RPCResponse;
		const error = body.error
			? { code: body.error.code ?? null, message: body.error.message ?? null }
			: null;
		return { outcome: { type: 'response', error }, body };
	} catch {
		return { outcome: { type: timedOut ? 'timeout' : 'network' } };
	} finally {
		clearTimeout(timer);
	}
}

/** The fastest-RPC ping, reduced to its observation. Mismatch = core's call. */
async function probe(
	url: string,
	timeoutMs: number
): Promise<{ reported: number | null; latencyMs: number }> {
	const controller = new AbortController();
	const timer = setTimeout(() => controller.abort(), timeoutMs);
	const t0 = Date.now();
	try {
		const res = await fetch(url, {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'eth_chainId', params: [] }),
			signal: controller.signal
		});
		if (!res.ok) return { reported: null, latencyMs: Date.now() - t0 };
		const json: unknown = await res.json();
		const result = (json as { result?: unknown } | null)?.result;
		if (typeof result !== 'string') return { reported: null, latencyMs: Date.now() - t0 };
		return { reported: asChainId(parseInt(result, 16)), latencyMs: Date.now() - t0 };
	} catch {
		return { reported: null, latencyMs: Date.now() - t0 };
	} finally {
		clearTimeout(timer);
	}
}

// ---------------------------------------------------------------------------
// The executor
// ---------------------------------------------------------------------------

export type RpcPoolExecutor = {
	execute(effect: RpcPoolEffect): Promise<RpcShellResult>;
	toFailure(effect: RpcPoolEffect, error: unknown): RpcShellResult;
};

/** A factory bound to the shell's call registry — testable with its own. */
export function createRpcPoolExecutor(registry: RpcPoolCallRegistry): RpcPoolExecutor {
	async function execute(effect: RpcPoolEffect): Promise<RpcShellResult> {
		const operation = effect.operation;
		switch (operation.type) {
			case 'load_pool_config': {
				const chainId = operation.chain_id;
				// NEVER_BANNED is load-bearing: the core filters bans at selection
				// (invariant ⑧); filtering here would hide an expired temp ban.
				const [rpcEndpoints, bundlerEndpoints] = await Promise.all([
					collectRpcUrls(chainId, NEVER_BANNED),
					collectBundlerUrls(chainId, NEVER_BANNED)
				]);
				registry.noteEndpoints?.(chainId, rpcEndpoints, bundlerEndpoints);
				return {
					type: 'pool_config',
					chain_id: chainId,
					rpc_endpoints: rpcEndpoints,
					bundler_endpoints: bundlerEndpoints,
					now_ms: Date.now()
				};
			}

			case 'json_rpc_post': {
				const held = registry.payload(operation.call_id);
				const t0 = Date.now();
				const { outcome, body } = held
					? await post(
							operation.url,
							operation.method,
							held.params,
							// Spec 081 FR-007: never forward the user's RPC endpoint.
							undefined,
							operation.timeout_ms
						)
					: // The caller is already gone. Report the endpoint as failing so
						// the core routes on rather than waiting forever.
						{ outcome: { type: 'network' } as RpcTransportOutcome, body: undefined };
				if (body) registry.keepBody(operation.call_id, operation.url, body);
				return {
					type: 'post_outcome',
					call_id: operation.call_id,
					url: operation.url,
					outcome,
					latency_ms: Date.now() - t0,
					now_ms: Date.now()
				};
			}

			case 'probe_chain_id': {
				const { reported, latencyMs } = await probe(operation.url, operation.timeout_ms);
				return {
					type: 'chain_id_probed',
					chain_id: operation.chain_id,
					url: operation.url,
					reported,
					latency_ms: latencyMs,
					now_ms: Date.now()
				};
			}

			case 'draw_jitter':
				// The core has no randomness; this is the only source of it.
				return { type: 'jitter', call_id: operation.call_id, value: Math.random() };

			case 'start_backoff': {
				registry.noteBackoff?.(operation.call_id);
				await new Promise<void>((resolve) => setTimeout(resolve, operation.delay_ms));
				return { type: 'backoff_elapsed', call_id: operation.call_id, now_ms: Date.now() };
			}

			case 'persist_bans':
				await setItem(BANNED_STORAGE_KEY, JSON.stringify(operation.entries.map(encodeBan)));
				return { type: 'persisted' };

			case 'conclude':
				registry.settle(operation.call_id, operation.verdict);
				return { type: 'concluded' };

			default: {
				const never: never = operation;
				throw new Error(`unhandled rpc_pool operation: ${JSON.stringify(never)}`);
			}
		}
	}

	function toFailure(effect: RpcPoolEffect): RpcShellResult {
		const operation = effect.operation;
		switch (operation.type) {
			case 'load_pool_config':
				// Unreadable config is an empty pool, which the core sweeps and
				// fails — the same end the TS initPool rejection reached.
				return {
					type: 'pool_config',
					chain_id: operation.chain_id,
					rpc_endpoints: [],
					bundler_endpoints: [],
					now_ms: Date.now()
				};
			case 'json_rpc_post':
				return {
					type: 'post_outcome',
					call_id: operation.call_id,
					url: operation.url,
					outcome: { type: 'network' },
					latency_ms: 0,
					now_ms: Date.now()
				};
			case 'probe_chain_id':
				return {
					type: 'chain_id_probed',
					chain_id: operation.chain_id,
					url: operation.url,
					reported: null,
					latency_ms: 0,
					now_ms: Date.now()
				};
			case 'draw_jitter':
				return { type: 'jitter', call_id: operation.call_id, value: 0 };
			case 'start_backoff':
				return { type: 'backoff_elapsed', call_id: operation.call_id, now_ms: Date.now() };
			case 'persist_bans':
				// Best-effort; the in-memory ban map stays authoritative.
				return { type: 'persisted' };
			case 'conclude':
				return { type: 'concluded' };
			default: {
				const never: never = operation;
				throw new Error(`unhandled rpc_pool operation: ${JSON.stringify(never)}`);
			}
		}
	}

	return { execute, toFailure };
}
