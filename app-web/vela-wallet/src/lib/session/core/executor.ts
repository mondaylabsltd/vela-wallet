/**
 * The session machine's only contact with the outside world.
 *
 * Eleven operations. The machine is app-resident — constructed once per page
 * load and outliving every screen — because "which wallet is this browser
 * signed into" is not a property of any one screen.
 *
 * All storage, but for one read: the landing watch (issue 409). A one-key
 * wallet is entered at the registry's 202, and the session — still here when
 * the create screen is gone — reads the outbox, waits on each accepted
 * record's registry task, and removes a record once its task has landed. A
 * read and a local write; no passkey anywhere.
 */

import * as Storage from '$lib/onboarding/core/storage';
import * as Registry from '$lib/onboarding/core/registry';
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

		// Issue 409 — the landing watch. The outbox goes over as stored; the
		// core decides which records a read can settle, and a record it cannot
		// read costs only itself.
		case 'load_pending_uploads':
			return { type: 'pending_uploads_loaded', records: Storage.loadPendingUploads() };

		// The same poll, interval and budget the create's publish always waited
		// with — now after "Wallet created" instead of before it.
		case 'await_registry_landing':
			try {
				await Registry.awaitTask(operation.task_id);
				return { type: 'registry_landed' };
			} catch (error) {
				return {
					type: 'registry_landing_unconfirmed',
					message: error instanceof Error ? error.message : String(error)
				};
			}

		// Only ever after `registry_landed` — the core's rule.
		case 'remove_pending_upload':
			Storage.removePendingUpload(operation.credential_id);
			return { type: 'pending_upload_removed' };

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
		// The landing watch fails toward keeping the record: the watch stops,
		// the warning stays, the next load asks again.
		case 'load_pending_uploads':
			return { type: 'pending_uploads_unavailable' };
		case 'await_registry_landing':
			return { type: 'registry_landing_unconfirmed', message: 'the wait did not answer' };
		case 'remove_pending_upload':
			return { type: 'pending_upload_removed' };
		default: {
			const never: never = operation;
			throw new Error(`no failure variant for session operation: ${JSON.stringify(never)}`);
		}
	}
}
