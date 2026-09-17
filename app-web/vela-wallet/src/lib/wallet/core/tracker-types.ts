// Ported from src/services/wallet-state-core/tx-tracker-types.ts @ f9bcb278 — RN seams rewritten to the web modules; logic verbatim.
/**
 * Platform-neutral types for the `tx_tracker` core (spec 017, group G13).
 *
 * Standalone for the reason `token-trust-types.ts` states for itself: the
 * native stub (`tx-tracker-session.ts`) needs these declarations, and importing
 * them from a `.web` module would drag the web-only service graph into the
 * native bundle.
 */

import type { TrackOperation } from '$lib/core/generated/TrackOperation';
import type { TrackView } from '$lib/core/generated/TrackView';
import type { TrustReceiptLog } from '$lib/core/generated/TrustReceiptLog';
import type { SessionOptions } from '$lib/core/types';

/** One request from the core, carrying the id it will be answered by. */
export type TrackEffect = { id: number; operation: TrackOperation };

/**
 * The two hand-offs this machine owes other machines. They are ports rather
 * than direct imports so the executor stays the only I/O site AND stays
 * testable against the real core without constructing two other cores.
 */
export interface TrackShellPorts {
	/**
	 * `activity_feed`'s `ReconcileCompleted` — the feed re-reads the store
	 * (without celebrating) when this machine's verdicts landed. `0` is a whole
	 * no-op in that core, so it is always safe to call.
	 */
	feedReconciled(resolvedCount: number): void;
	/**
	 * `token_trust`'s `ReceiptLogsConfirmed` — the ONLY legal auto-add entry
	 * point. Reached from `NotifyConfirmed` and from nowhere else; a sign-time
	 * simulation must never be routed here. The logs are the AUTHENTIC ones the
	 * receipt poll just returned, which is why the core's `NotifyConfirmed`
	 * carries no payload of its own: the shell already holds them.
	 */
	receiptLogsConfirmed(from: string, chainId: number, logs: TrustReceiptLog[]): void;
	/**
	 * A submission confirmed on-chain — money left this wallet, and the hero
	 * total must follow (issue 188). Reached from `NotifyConfirmed` for EVERY
	 * confirmation, logs or not. The balance core's refresh has to be forced:
	 * the send flow clears the token cache at submit, before the op has
	 * landed, so a silent refresh in between re-fills it with the pre-send
	 * balances for five minutes.
	 */
	confirmed(chainId: number): void;
}

export type TxTrackerSessionOptions = SessionOptions<TrackView> & {
	ports: TrackShellPorts;
};
