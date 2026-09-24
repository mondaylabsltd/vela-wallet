/**
 * The session machine's only contact with the outside world.
 *
 * Eight operations, all storage. The machine is app-resident — constructed once
 * per page load and outliving every screen — because "which wallet is this
 * browser signed into" is not a property of any one screen.
 */

import * as Storage from '$lib/onboarding/core/storage';
import type { SessionOperation } from '../generated/SessionOperation';
import type { SessionShellResult } from '../generated/SessionShellResult';

export type SessionEffect = { id: number; operation: SessionOperation };

export async function executeSession(effect: SessionEffect): Promise<SessionShellResult> {
	const operation = effect.operation;
	switch (operation.type) {
		case 'load_accounts':
			return { type: 'accounts_loaded', accounts: Storage.loadAccounts() };

		case 'load_active_index':
			return { type: 'active_index_loaded', index: Storage.loadActiveIndex() };

		case 'save_account':
			Storage.saveAccount(operation.account);
			return { type: 'account_saved' };

		case 'save_active_index':
			Storage.saveActiveIndex(operation.index);
			return { type: 'active_index_saved' };

		case 'check_pending_uploads':
			return { type: 'pending_uploads', has_pending: Storage.loadPendingUploads().length > 0 };

		case 'remove_account':
			Storage.removeAccount(operation.address);
			return { type: 'account_removed' };

		case 'clear_signed_in_wallet':
			Storage.clearSignedInWallet();
			return { type: 'signed_in_wallet_cleared' };

		case 'clear_extension_cache':
			// No browser extension shares this origin's storage, so there is no
			// snapshot to drop. Answering rather than skipping keeps the core's
			// sequence intact — a shell that silently ignored an operation would
			// leave it waiting.
			return { type: 'extension_cache_cleared' };

		default: {
			const never: never = operation;
			throw new Error(`unhandled session operation: ${JSON.stringify(never)}`);
		}
	}
}

/**
 * What each operation answers with when it threw.
 *
 * Every write here is best effort by the core's own design: a storage failure
 * leaves the session correct in memory, and the next launch reads whatever
 * actually landed. Only the two reads have failure variants, because a failed
 * read is a fact the core has to reason about.
 */
export function sessionFailure(effect: SessionEffect): SessionShellResult {
	const operation = effect.operation;
	switch (operation.type) {
		case 'load_accounts':
			return { type: 'accounts_unavailable' };
		case 'load_active_index':
			// No failure variant: the shell's read already maps missing, garbage
			// and errors to 0.
			return { type: 'active_index_loaded', index: 0 };
		case 'check_pending_uploads':
			// Fail closed: with the answer unknown, the sign-out dialog must not
			// open unwarned.
			return { type: 'pending_uploads_unavailable' };
		case 'save_account':
			return { type: 'account_saved' };
		case 'save_active_index':
			return { type: 'active_index_saved' };
		case 'remove_account':
			// The list is unchanged, and the core's own state already dropped
			// the row: answering keeps the sequence going, and the next write
			// re-states the truth.
			return { type: 'account_removed' };
		case 'clear_signed_in_wallet':
			return { type: 'signed_in_wallet_cleared' };
		case 'clear_extension_cache':
			return { type: 'extension_cache_cleared' };
		default: {
			const never: never = operation;
			throw new Error(`no failure variant for session operation: ${JSON.stringify(never)}`);
		}
	}
}
