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

import {
	rpcEarlyVerdict,
	rpcHedgeAfterMs,
	rpcIsHedgedRead,
	transportOutcomeOf
} from '$lib/core/kernels';
import { relayRpcHeader } from '$lib/services/relay-rpc-header';
import { getItem, setItem } from '$lib/services/storage';
import {
	BANNED_STORAGE_KEY,
	collectBundlerUrls,
	collectRpcUrls,
	NEVER_BANNED,
	ownNodeOnly,
	type RPCResponse
} from '$lib/services/rpc-pool-endpoints';

import type { RpcBanEntry } from '$lib/core/generated/RpcBanEntry';
import type { RpcCallVerdict } from '$lib/core/generated/RpcCallVerdict';
import type { RpcKind } from '$lib/core/generated/RpcKind';
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
	// Spec 082 RA1: `fetch` cannot tell a refused connection from a reply lost
	// after the request went out, so a thrown fetch is `network` — "it may have
	// arrived". The one fact the browser does give is that it is offline:
	// then nothing can have left, and the call is `not_connected`.
	const offline = typeof navigator !== 'undefined' && navigator.onLine === false;
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

		if (!res.ok) {
			// The status is the core's to route on. A body that is a JSON-RPC
			// `error` is the endpoint's own words, held beside it (round 3,
			// send-5): a submit's refusal inside a 500 — what it proves is the
			// core's (`user_op::refused_outright`), never this file's.
			const refusal = await refusalBody(res);
			const outcome: RpcTransportOutcome = { type: 'http_error', status: res.status };
			return refusal ? { outcome, body: refusal } : { outcome };
		}

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
		// r11-9: the class is the browser's to name, what it means the core's.
		return {
			outcome: transportOutcomeOf(offline ? 'offline' : timedOut ? 'timeout' : 'unknown')
		};
	} finally {
		clearTimeout(timer);
	}
}

/**
 * An HTTP error's body, when it is a JSON-RPC answer with an `error` object —
 * else nothing (a gateway's page, plain text, an unreadable body). Read only
 * for a server error: a 4xx is refused before any handler, and says nothing
 * of an operation.
 */
async function refusalBody(res: Response): Promise<RPCResponse | undefined> {
	if (res.status < 500 || res.status > 599) return undefined;
	try {
		const json: unknown = JSON.parse(await res.text());
		const error = (json as { error?: unknown } | null)?.error;
		return error !== null && typeof error === 'object' ? (json as RPCResponse) : undefined;
	} catch {
		return undefined;
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
// Fail-fast for an endpoint that is not answering (home-17)
// ---------------------------------------------------------------------------

/**
 * Reads only: nothing a fail-fast could stop is a write, so no send is ever
 * told "not sent" by this rather than by the wire.
 */
const READ_METHODS = new Set([
	'eth_call',
	'eth_getBalance',
	'eth_getLogs',
	'eth_blockNumber',
	'eth_chainId',
	'eth_getCode',
	'eth_getTransactionCount',
	'eth_getTransactionReceipt',
	'eth_getTransactionByHash',
	'eth_getBlockByNumber',
	'eth_getBlockByHash',
	'eth_gasPrice',
	'eth_feeHistory',
	'eth_maxPriorityFeePerGas',
	'eth_estimateGas'
]);

/** Transport failures in a row before an endpoint is skipped for a while. */
export const SKIP_AFTER_FAILURES = 3;
/** The first skip window; it doubles with each further failure. */
export const SKIP_BASE_MS = 1_000;
/** The longest an endpoint is skipped before it is tried again. */
export const SKIP_CAP_MS = 30_000;

/**
 * Per endpoint, the transport failures in a row and until when its reads are
 * not put on the wire.
 *
 * During an outage every call still sweeps every endpoint, three passes each
 * (the core's routing, unchanged), and with refused connections failing in a
 * millisecond that was 567 requests to one dead node in 40 s (home-17). Once
 * an endpoint has failed on transport {@link SKIP_AFTER_FAILURES} times in a
 * row, a read bound for it is answered `not_connected` at once — the truth:
 * nothing left the device — and the core routes on exactly as it would
 * after a refused connection. When the window ends one read goes through; an
 * answer of any kind closes it, another failure doubles it. A hung endpoint
 * (`timeout`) is skipped the same way, so after three it no longer holds
 * every read for the whole per-endpoint timeout.
 */
export function createEndpointSkipper(now: () => number = Date.now) {
	const failing = new Map<string, { failures: number; until: number }>();
	return {
		/** Is a read for `url` to be answered without the wire, now? */
		skips(url: string, method: string): boolean {
			if (!READ_METHODS.has(method)) return false;
			const entry = failing.get(url);
			return entry !== undefined && now() < entry.until;
		},
		/** What the wire said for `url` (a skipped read is not a report). */
		note(url: string, outcome: RpcTransportOutcome): void {
			const transport =
				outcome.type === 'network' ||
				outcome.type === 'not_connected' ||
				outcome.type === 'timeout';
			if (!transport) {
				failing.delete(url);
				return;
			}
			const failures = (failing.get(url)?.failures ?? 0) + 1;
			const until =
				failures < SKIP_AFTER_FAILURES
					? 0
					: now() + Math.min(SKIP_CAP_MS, SKIP_BASE_MS * 2 ** (failures - SKIP_AFTER_FAILURES));
			failing.set(url, { failures, until });
		},
		/** The device is back online: every endpoint is tried afresh. */
		reset(): void {
			failing.clear();
		}
	};
}

// ---------------------------------------------------------------------------
// Hedged reads (perf-6) — the desktop's spec 083 H6, in this shell
// ---------------------------------------------------------------------------

/**
 * Which reads may be hedged, after how long, and which early answer a caller
 * may take — all the core's (`is_hedged_read`, `HEDGE_AFTER_MS`,
 * `early_verdict`). Injected so a test can drive the executor without the wasm.
 */
export interface HedgePolicy {
	afterMs(): number;
	isHedged(kind: RpcKind, method: string): boolean;
	early(
		kind: RpcKind,
		method: string,
		url: string,
		outcome: RpcTransportOutcome
	): RpcCallVerdict | null;
}

const CORE_HEDGE_POLICY: HedgePolicy = {
	// Read when asked, not at import: a module that mocks the kernels need not
	// know these exist.
	afterMs: () => rpcHedgeAfterMs(),
	isHedged: (kind, method) => rpcIsHedgedRead(kind, method),
	early: (kind, method, url, outcome) => rpcEarlyVerdict(kind, method, url, outcome)
};

/** One POST's result as the core is told it. */
type Posted = { outcome: RpcTransportOutcome; latencyMs: number };

/**
 * One hedged read's extra asks. The core still walks its endpoints one at a
 * time and is told every outcome, truthfully, in its own order. A hedge only
 * asks EARLY — after {@link HedgePolicy.afterMs} of silence — and only the
 * endpoint the core will ask next (the first of the view's `pending_urls`
 * for this call not already asked and not banned). When the core reaches an
 * endpoint a hedge already asked, that answer is its answer — in hand or
 * still on the wire — never a second request. At most one hedge the core has
 * not reached yet is on the wire per read: one extra request, never a fan-out.
 *
 * home-20: a hedge that comes BACK with an answer no caller may take early (a
 * node error the core would fail over on) is no longer on the wire, so the
 * endpoint after it is asked at once. It used to be the read's only hedge: on
 * a cold load a black-holed Soneium node and a refusing second one held the
 * whole home at "Checking…" for the first node's 8 s timeout.
 */
type Hedge = {
	method: string;
	params: unknown[];
	/** The core's own per-endpoint read timeout: a hedge is the same post, early. */
	timeoutMs: number;
	/** The endpoint the core is waiting on, and its timer. */
	current: { url: string; timer: ReturnType<typeof setTimeout> } | null;
	/** Hedges asked and not yet reached by the core, by URL. */
	asked: Map<string, Promise<Posted>>;
	/** The hedge still waiting for its answer, if any: one at a time. */
	wire: string | null;
};

// ---------------------------------------------------------------------------
// The executor
// ---------------------------------------------------------------------------

export type RpcPoolExecutor = {
	execute(effect: RpcPoolEffect): Promise<RpcShellResult>;
	toFailure(effect: RpcPoolEffect, error: unknown): RpcShellResult;
};

/** A factory bound to the shell's call registry — testable with its own. */
export function createRpcPoolExecutor(
	registry: RpcPoolCallRegistry,
	hedgePolicy: HedgePolicy = CORE_HEDGE_POLICY
): RpcPoolExecutor {
	const skipper = createEndpointSkipper();
	const hedges = new Map<string, Hedge>();
	// Offline, every endpoint failed alike; back online, none of that is news
	// about an endpoint (home-2's re-read must reach the wire).
	if (typeof window !== 'undefined') window.addEventListener('online', () => skipper.reset());

	/** One POST through the skipper, as the core is told it. */
	async function ask(
		callId: string,
		url: string,
		method: string,
		params: unknown[],
		headers: Record<string, string> | undefined,
		timeoutMs: number,
		mustSend = false
	): Promise<Posted> {
		const t0 = Date.now();
		// home-17: an endpoint failing on transport is skipped a while — but
		// never the person's own ask (r17-1 / r17-10, the core's
		// `must_send`): a local refusal is not the network's answer, and one
		// Try again after an outage lifts must reach the node.
		if (!mustSend && skipper.skips(url, method))
			return { outcome: { type: 'not_connected' }, latencyMs: 0 };
		const { outcome, body } = await post(url, method, params, headers, timeoutMs);
		if (body) registry.keepBody(callId, url, body);
		skipper.note(url, outcome);
		return { outcome, latencyMs: Date.now() - t0 };
	}

	function stopTimer(hedge: Hedge): void {
		if (hedge.current) clearTimeout(hedge.current.timer);
		hedge.current = null;
	}

	/** The core is waiting on `url`: ask the next endpoint if it stays silent. */
	function arm(callId: string, hedge: Hedge, url: string, delayMs: number): void {
		stopTimer(hedge);
		const timer = setTimeout(() => fire(callId, url), Math.max(0, delayMs));
		hedge.current = { url, timer };
	}

	function fire(callId: string, waitingOn: string): void {
		const hedge = hedges.get(callId);
		if (!hedge || hedge.current?.url !== waitingOn) return;
		// One unreached hedge on the wire at a time.
		if (hedge.wire !== null) return;
		const view = registry.view?.() ?? null;
		// Banned since the pass was drawn up: not asked. An expired entry the
		// core has not pruned yet is passed over too — a hedge is an
		// optimisation, and the core's own sweep still reaches it.
		const banned = new Set((view?.banned ?? []).map((ban) => ban.url));
		const target = (view?.pending_urls[callId] ?? []).find(
			(url) => !banned.has(url) && !hedge.asked.has(url) && !skipper.skips(url, hedge.method)
		);
		if (target === undefined) return;
		const asked = ask(callId, target, hedge.method, hedge.params, undefined, hedge.timeoutMs);
		hedge.asked.set(target, asked);
		hedge.wire = target;
		void asked.then(({ outcome }) => {
			if (hedge.wire === target) hedge.wire = null;
			// The core concluded meanwhile: a late hedge is dropped.
			if (hedges.get(callId) !== hedge || !hedge.asked.has(target)) return;
			if (hedgePolicy.early('rpc', hedge.method, target, outcome)?.type === 'respond') {
				registry.answerEarly?.(callId, target);
				return;
			}
			// Answered, but not with what a caller may take: the endpoint the
			// core is waiting on is still silent, so the next one is asked now.
			if (hedge.current !== null) fire(callId, hedge.current.url);
		});
	}

	async function execute(effect: RpcPoolEffect): Promise<RpcShellResult> {
		const operation = effect.operation;
		switch (operation.type) {
			case 'load_pool_config': {
				const chainId = operation.chain_id;
				// NEVER_BANNED is load-bearing: the core filters bans at selection
				// (invariant ⑧); filtering here would hide an expired temp ban.
				const [rpcEndpoints, bundlerEndpoints, ownOnly] = await Promise.all([
					collectRpcUrls(chainId, NEVER_BANNED),
					collectBundlerUrls(chainId, NEVER_BANNED),
					ownNodeOnly(chainId)
				]);
				registry.noteEndpoints?.(chainId, rpcEndpoints, bundlerEndpoints);
				return {
					type: 'pool_config',
					chain_id: chainId,
					rpc_endpoints: rpcEndpoints,
					bundler_endpoints: bundlerEndpoints,
					// The person's "Only use my node": what it means is the core's.
					own_only: ownOnly,
					now_ms: Date.now()
				};
			}

			case 'json_rpc_post': {
				const callId = operation.call_id;
				const url = operation.url;
				const held = registry.payload(callId);
				const result = (posted: Posted): RpcShellResult => ({
					type: 'post_outcome',
					call_id: callId,
					url,
					outcome: posted.outcome,
					latency_ms: posted.latencyMs,
					now_ms: Date.now()
				});

				// perf-6: the core reached an endpoint a hedge already asked —
				// its answer, in hand or on the wire, is the core's.
				let hedge = hedges.get(callId);
				const early = hedge?.asked.get(url);
				if (hedge && early) {
					hedge.asked.delete(url);
					// The core's own ask now, not a hedge: the next may be hedged.
					if (hedge.wire === url) hedge.wire = null;
					arm(callId, hedge, url, hedgePolicy.afterMs());
					const posted = await early;
					if (hedge.current?.url === url) stopTimer(hedge);
					return result(posted);
				}

				if (!held) {
					// The caller is already gone. Report the endpoint as failing so
					// the core routes on rather than waiting forever.
					return result({ outcome: { type: 'network' }, latencyMs: 0 });
				}

				if (!hedge && held.kind === 'rpc' && hedgePolicy.isHedged('rpc', operation.method)) {
					hedge = {
						method: operation.method,
						params: held.params,
						timeoutMs: operation.timeout_ms,
						current: null,
						asked: new Map(),
						wire: null
					};
					hedges.set(callId, hedge);
				}
				if (hedge) arm(callId, hedge, url, hedgePolicy.afterMs());
				const posted = await ask(
					callId,
					url,
					operation.method,
					held.params,
					// Spec 098 §5: a bundler call names the chain's RPC to the
					// relay; the core sets `x_rpc_url` on bundler calls only.
					relayRpcHeader(operation.x_rpc_url),
					operation.timeout_ms,
					operation.must_send ?? false
				);
				if (hedge && hedge.current?.url === url) stopTimer(hedge);
				return result(posted);
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

			case 'conclude': {
				// The call is over: its hedges are abandoned, a late one dropped.
				const hedge = hedges.get(operation.call_id);
				if (hedge) {
					stopTimer(hedge);
					hedges.delete(operation.call_id);
				}
				registry.settle(operation.call_id, operation.verdict);
				return { type: 'concluded' };
			}

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
					own_only: false,
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
