// Ported from src/services/wallet-state-core/fee-types.ts @ f9bcb278 — RN seams rewritten to the web modules; logic verbatim.
/**
 * Platform-neutral types for the `fee_policy` core (spec 017, the last machine
 * to be wired).
 *
 * Standalone rather than folded into `types.ts`, for the reason that file
 * states for itself: the native stub (`fee-session.ts`) needs these
 * declarations, and importing them from a `.web` module would drag the web-only
 * service graph into the native bundle.
 */

import type { FeeOperation } from '#lib/core/generated/FeeOperation.ts';
import type { FeeView } from '#lib/core/generated/FeeView.ts';
import type { SessionOptions } from '#lib/core/types.ts';

/** One request from the core, carrying the id it will be answered by. */
export type FeeEffect = { id: number; operation: FeeOperation };

export type FeeSessionOptions = SessionOptions<FeeView> & {
	/**
	 * The passkey public key of the account being quoted — needed to build the
	 * real initCode for a Safe that is not deployed yet.
	 *
	 * A getter, not a value, because it resolves asynchronously (the SigningSheet
	 * loads it from the credential store while the sheet is already open) and
	 * because the account can be switched under a live session. Read at the
	 * moment the core asks for a simulation, so the key that builds the initCode
	 * is the one belonging to the account the core is pricing for.
	 *
	 * `undefined` is a fact the executor reports as `context_unavailable`; it is
	 * never treated as permission to simulate a draft that cannot match the
	 * submitted operation.
	 */
	publicKeyHex: () => string | undefined;
};
