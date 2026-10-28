// Ported from src/services/wallet-state-core/clear-executor.ts @ f9bcb278 — RN seams rewritten to the web modules; logic verbatim.
/**
 * The only place the `clear_signing` core touches the outside world.
 *
 * Five operations, each one existing service call. Nothing here interprets:
 * the five-level decode fallback, the ERC-165 revert-vs-unreachable ruling, the
 * decimals trust rule, the risk floors and the SIWE domain adjudication are all
 * in Rust. This file fetches, calls, waits and reports RAW answers — including
 * the difference between "the RPC answered with an error object" and "the RPC
 * could not be reached", which is exactly the distinction a wrong guess would
 * turn into a permanently mis-cached token standard.
 *
 * Two timeouts, two owners, deliberately:
 * - the descriptor fetch's 5 s stays in the fetch layer (`NET_TIMEOUTS.descriptor`),
 *   because it collapses to the same observable "no descriptor" the core already
 *   models;
 * - the 3 s ERC-165 race and the 4 s decimals warm are core `Timer` operations,
 *   because the core decides what a timeout MEANS (render ERC-20 but cache
 *   nothing; fall back to 18 decimals but flag `unverified`).
 *
 * Failure contract (shared effect loop): nothing rejects. Every rejection is
 * converted into the result variant that operation answers with.
 */

import { fetchWithTimeout, NET_TIMEOUTS } from '$lib/services/net';
import { poolRpcCall } from '$lib/services/rpc-pool';
import { lookupSelector } from '$lib/services/selector-registry';
import { getEthereumDataURL } from '$lib/services/endpoints';

import type { ClearShellResult } from '$lib/core/generated/ClearShellResult';
import type { ClearEffect } from './clear-types';

/**
 * In-flight coalescing for the read-only lookups. NOT a cache: an entry exists
 * only while the request it stands for is outstanding, and nothing is retained
 * once it settles, so no answer can go stale here and no retention policy is
 * being invented outside the core.
 *
 * It exists because the N legs of one `wallet_sendCalls` each run on their OWN
 * core session (the machine resolves one request at a time and supersedes
 * anything in flight, so legs cannot share one) and therefore on their own
 * descriptor / ERC-165 / decimals caches. The legs start in the same tick, so
 * two legs touching the same token used to issue two descriptor fetches, two
 * ERC-165 probes and two `decimals()` reads — and, because the core's 3 s
 * ERC-165 race and 4 s decimals warm are real races, could receive DIFFERENT
 * answers and print two rows that disagree about one token. Coalesced, they are
 * handed the same bytes at the same instant and cannot.
 *
 * Answers stay raw. What a revert means, what a timeout means and what may be
 * cached remain the core's rulings. `timer` and `now` are deliberately never
 * coalesced: their answers are per-core (a timer token, a clock reading).
 */
const inFlight = new Map<string, Promise<ClearShellResult>>();

function coalesced(key: string, run: () => Promise<ClearShellResult>): Promise<ClearShellResult> {
	const outstanding = inFlight.get(key);
	if (outstanding) return outstanding;
	const started = run();
	inFlight.set(key, started);
	// Settled either way → the entry is gone, so the next asker starts a fresh
	// request rather than replaying an old answer.
	void started.then(
		() => {
			if (inFlight.get(key) === started) inFlight.delete(key);
		},
		() => {
			if (inFlight.get(key) === started) inFlight.delete(key);
		}
	);
	return started;
}

export async function executeClearOperation(
	effect: ClearEffect,
	signal: AbortSignal
): Promise<ClearShellResult> {
	const operation = effect.operation;
	switch (operation.type) {
		case 'http_get': {
			// `fetchDescriptor`'s body: a non-200, a timeout and a network error are
			// one and the same "no descriptor" (the core caches that null too, so a
			// 404 isn't re-fetched every render).
			return coalesced(`http_get\u0001${operation.path}`, async () => {
				const response = await fetchWithTimeout(
					`${getEthereumDataURL()}${operation.path}`,
					{},
					{ timeoutMs: NET_TIMEOUTS.descriptor }
				);
				return {
					type: 'descriptor_fetched',
					path: operation.path,
					json: response.ok ? await response.text() : null
				};
			});
		}

		case 'rpc_eth_call': {
			// Keyed on the whole question (probe included), so one leg's answer is
			// only ever reused for the identical question.
			const key = `rpc\u0001${operation.probe}\u0001${operation.chain_id}\u0001${operation.to}\u0001${operation.data}`;
			return coalesced(key, async () => {
				const response = await poolRpcCall(
					'eth_call',
					[{ to: operation.to, data: operation.data }, 'latest'],
					operation.chain_id
				);
				return {
					type: 'rpc_answer',
					probe: operation.probe,
					chain_id: operation.chain_id,
					to: operation.to,
					// A revert arrives as an error OBJECT, not a rejection. Reporting it as
					// "unreachable" would let a transient outage look identical to a plain
					// ERC-20's revert — and the core would then never cache the verdict.
					result: typeof response?.result === 'string' ? response.result : null,
					rpc_error: response?.error != null
				};
			});
		}

		case 'rpc_get_code': {
			// cs-13: whether anything lives at the target on this chain. The
			// answer stays raw — "0x" means no code, and what that means for a
			// name or a descriptor is the core's ruling.
			const key = `code\u0001${operation.chain_id}\u0001${operation.address}`;
			return coalesced(key, async () => {
				const response = await poolRpcCall(
					'eth_getCode',
					[operation.address, 'latest'],
					operation.chain_id
				);
				return {
					type: 'rpc_answer',
					probe: 'code',
					chain_id: operation.chain_id,
					to: operation.address,
					result: typeof response?.result === 'string' ? response.result : null,
					rpc_error: response?.error != null
				};
			});
		}

		case 'selector_db_lookup':
			// `lookupSelector` owns its own network policy, merge order and cache.
			return { type: 'selector_candidates', sigs: await lookupSelector(operation.selector) };

		case 'timer':
			await new Promise<void>((resolve) => {
				const timer = setTimeout(resolve, operation.ms);
				// The core cancels a timer whose phase is over; leaving it armed would
				// hold a wasm callback (and the run's memory) for the full window.
				signal.addEventListener(
					'abort',
					() => {
						clearTimeout(timer);
						resolve();
					},
					{ once: true }
				);
			});
			return { type: 'timed_out', token: operation.token };

		case 'now':
			return { type: 'clock', now_ms: Date.now() };
	}
}

export function clearOperationFailure(effect: ClearEffect): ClearShellResult {
	const operation = effect.operation;
	switch (operation.type) {
		case 'http_get':
			return { type: 'descriptor_fetched', path: operation.path, json: null };
		case 'rpc_eth_call':
			// Threw / unreachable — NOT a revert. `rpc_error: false` with no result
			// is the core's "unknown", which it refuses to cache (invariant ②).
			return {
				type: 'rpc_answer',
				probe: operation.probe,
				chain_id: operation.chain_id,
				to: operation.to,
				result: null,
				rpc_error: false
			};
		case 'rpc_get_code':
			// Unreachable: the core's "unknown", which names as it did before.
			return {
				type: 'rpc_answer',
				probe: 'code',
				chain_id: operation.chain_id,
				to: operation.address,
				result: null,
				rpc_error: false
			};
		case 'selector_db_lookup':
			// `lookupSelector` already swallows per-source failures; a total failure
			// is "no candidate", which resolves blind rather than guessing.
			return { type: 'selector_candidates', sigs: [] };
		case 'timer':
			return { type: 'timed_out', token: operation.token };
		case 'now':
			return { type: 'clock', now_ms: Date.now() };
	}
}
