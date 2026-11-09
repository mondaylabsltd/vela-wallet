// Ported from src/services/wallet-state-core/rpc-pool-types.ts @ c13e89d4
// (spec 025). One module per machine; the generic options live in $lib/core.

import type { RpcCallVerdict } from '$lib/core/generated/RpcCallVerdict';
import type { RpcKind } from '$lib/core/generated/RpcKind';
import type { RpcOperation } from '$lib/core/generated/RpcOperation';
import type { RpcPoolView } from '$lib/core/generated/RpcPoolView';
import type { CollectedEndpoint, RPCResponse } from '$lib/services/rpc-pool-endpoints';
import type { SessionOptions } from '$lib/core/types';

/** One request from the core, carrying the id it will be answered by. */
export type RpcPoolEffect = { id: number; operation: RpcOperation };

/**
 * The shell's side of a routed call. The core is a pure router: it decides
 * *which URL next and why* and carries no JSON-RPC params and no response
 * bodies. Everything the shell must hold between `CallRequested` and
 * `Conclude` is reached through this handle, which the facade owns and the
 * executor only reads/writes — keeping the executor free of module state and
 * drivable from a test with its own registry.
 */
export interface RpcPoolCallRegistry {
	/**
	 * The payload to POST for `callId`; `undefined` once the call concluded.
	 * `kind` names the pool it routes through — only an `rpc` read is ever
	 * hedged (perf-6); a payload without one is not. `corroborate` (R22): a
	 * read two independent endpoints answer — never hedged, never answered
	 * early.
	 */
	payload(
		callId: string
	): { method: string; params: unknown[]; kind?: RpcKind; corroborate?: boolean } | undefined;
	/**
	 * The pool's latest view — where a hedge goes (`pending_urls`, the core's
	 * own next endpoints) and what is banned. Absent: nothing is hedged.
	 */
	view?(): RpcPoolView | null;
	/**
	 * A hedge's answer the core says every node gives alike
	 * (`early_verdict`): hand the caller the body kept for (`callId`, `url`)
	 * now. The core is still told every outcome in its own order and
	 * concludes the call itself.
	 */
	answerEarly?(callId: string, url: string): void;
	/** Remember the body `url` returned for `callId` (verdicts name URLs). */
	keepBody(callId: string, url: string, body: RPCResponse): void;
	/** A verdict arrived — settle whatever promise the shell is holding. */
	settle(callId: string, verdict: RpcCallVerdict): void;
	/** The chain's candidate lists, as just collected (observational). */
	noteEndpoints?(chainId: number, rpc: CollectedEndpoint[], bundler: CollectedEndpoint[]): void;
	/** The core is about to sleep before another full-pool pass. */
	noteBackoff?(callId: string): void;
}

export type RpcPoolSessionOptions = SessionOptions<RpcPoolView> & {
	/** Where the executor reaches the payloads and delivers bodies/verdicts. */
	registry: RpcPoolCallRegistry;
};
