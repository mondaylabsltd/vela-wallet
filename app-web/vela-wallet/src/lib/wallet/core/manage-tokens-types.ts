// Ported from src/services/wallet-state-core/manage-tokens-types.ts @ c13e89d4 (spec 025).
/**
 * Platform-neutral types for the `manage_tokens` core (spec 017, group G2).
 *
 * Standalone, and NOT folded into `types.ts`, for the reason that file states
 * for itself: the native stub (`manage-tokens-session.ts`) needs these
 * declarations, and importing them from a `.web` module would drag the
 * web-only service graph into the native bundle. Keeping one module per
 * machine also keeps the parallel integration waves from editing one file.
 */

import type { MtokOperation } from '#lib/core/generated/MtokOperation.ts';
import type { MtokView } from '#lib/core/generated/MtokView.ts';
import type { SessionOptions } from '#lib/core/types.ts';

/** One request from the core, carrying the id it will be answered by. */
export type ManageTokensEffect = { id: number; operation: MtokOperation };

export type ManageTokensSessionOptions = SessionOptions<MtokView> & {
	/**
	 * The active wallet address, read at the moment the core asks for a cache
	 * invalidation — `fetchTokens` is keyed by wallet address and the account
	 * can be switched while the panel is open. A getter, not a value, so the
	 * panel's session survives an account switch instead of being rebuilt (which
	 * would wipe the form).
	 */
	account: () => string;
	/**
	 * Fired with the same cache drop the core orders after a confirmed save or
	 * delete — this is today's `onChanged` host refresh, now on the machine's
	 * schedule rather than the component's.
	 */
	onInvalidated?: () => void;
};
