// Ported from src/services/wallet-state-core/balance-types.ts @ c13e89d4 (spec 025).
/**
 * Platform-neutral types for the `balance_dashboard` core (spec 017, group G10).
 *
 * Standalone, and NOT folded into `types.ts`, for the reason that file states
 * for itself: the native stub (`balance-session.ts`) needs these declarations,
 * and importing them from a `.web` module would drag the web-only service graph
 * into the native bundle. One module per machine also keeps the parallel
 * integration waves off each other's files.
 */

import type { BalanceOperation } from '$lib/core/generated/BalanceOperation';
import type { BalanceToken } from '$lib/core/generated/BalanceToken';
import type { BalanceView } from '$lib/core/generated/BalanceView';
import type { SessionOptions } from '$lib/core/types';

/** One request from the core, carrying the id it will be answered by. */
export type BalanceEffect = { id: number; operation: BalanceOperation };

/**
 * The persisted "hide amounts" byte. Declared here — the graph's leaf — so the
 * boot-time read (`balance-resident.web.ts`), the core's `WritePrivacy` write
 * (`balance-executor.web.ts`) and the native store (`use-balance-privacy.ts`)
 * all name the same key without importing one another.
 */
export const BALANCE_PRIVACY_KEY = 'vela.balanceHidden';

/**
 * Where the mid-fetch stream goes.
 *
 * `FetchTokens` is the one operation that speaks twice: it streams every
 * chain's snapshot as it lands and *then* settles once. The settle rides the
 * operation's own result; the snapshots have nowhere to ride, so they are
 * dispatched as [`Event::ChainAssetsArrived`] through this sink. Injected
 * rather than imported so the executor keeps no module-level session handle
 * (the `RpcPoolCallRegistry` split, same reason: it stays drivable from a test
 * with its own sink).
 */
export interface BalanceStreamSink {
	/** One `onProgress` snapshot: the accumulated, USD-sorted tokens so far. */
	chainAssetsArrived(address: string, tokens: BalanceToken[]): void;
	/**
	 * A `FetchTokens` round for `address` has ended — settled OR errored. Said
	 * just before its answer goes back to the core, so a listener that wants
	 * the view the answer produces reads it on a later task. An errored round
	 * may change nothing on the view, which is why this is not read off it.
	 */
	roundEnded?(address: string): void;
}

export type BalanceSessionOptions = SessionOptions<BalanceView> & {
	/** Where the executor pushes `fetchTokens`'s progress snapshots. */
	stream: BalanceStreamSink;
};
