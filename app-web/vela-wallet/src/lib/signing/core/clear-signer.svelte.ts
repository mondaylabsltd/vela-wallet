/**
 * The Clear Signer's one ceremony at a time — APP-RESIDENT (spec 071).
 *
 * A signature can start from the dApp sheet, from the wallet's own send or
 * from the key backup, and the person waits for the page the same way in
 * each: "Waiting for the Clear Signer…", open the page again, cancel. So the
 * waiting state lives here, once, and `<SigningHost>` — mounted wherever a
 * signature can start — draws it.
 *
 * The request is the core's (`clearSignerRequest`), the page is the one
 * Settings names (`sign_pref`), the verdict is the core's
 * (`clearSignerVerify`, inside the channel). What this adds is the person's
 * side: the waiting sheet and the sentence a refusal ends with.
 */

import { loadCore } from '$lib/core/client';
import { clearSignerRequest, type ClearSignerInput, type ClearSignerKey } from '$lib/core/kernels';
import type { Assertion } from '$lib/onboarding/core/passkey';
import { signPreference } from '$lib/settings/core/sign-pref.svelte';
import {
	ClearSignerRefusedError,
	noticeOf,
	openClearSigner,
	type ClearSignerCeremony,
	type ClearSignerHost,
	type ClearSignerNotice
} from '../clear-signer';

export interface ClearSignerSessionView {
	/** The page is open (or blocked) and this wallet is waiting on its answer. */
	waiting: boolean;
	/** How the last ceremony ended, until the person dismisses it. */
	notice: ClearSignerNotice | null;
}

class ClearSignerSession {
	view = $state<ClearSignerSessionView>({ waiting: false, notice: null });

	#ceremony: ClearSignerCeremony | null = null;
	/** The person pressed Cancel: they know, so no sentence follows. */
	#cancelled = false;
	/** Test seam: the browser the channel opens its window in. */
	host: ClearSignerHost | undefined = undefined;

	/**
	 * Sign `digest` on the Clear Signer page. Resolves with the assertion the
	 * core accepted — the same shape a passkey's is, so the caller continues
	 * exactly as it would have — or rejects with `ClearSignerRefusedError`.
	 */
	async sign(
		input: ClearSignerInput,
		digest: Uint8Array,
		keys: ClearSignerKey[]
	): Promise<Assertion> {
		// One at a time: a new signature supersedes a forgotten one, as a
		// second passkey ceremony aborts the first.
		this.#ceremony?.cancel();
		await Promise.all([loadCore(), signPreference.ready()]);
		const ceremony = openClearSigner({
			signerUrl: signPreference.view.signer_url,
			request: clearSignerRequest(input),
			digest,
			keys,
			host: this.host
		});
		this.#ceremony = ceremony;
		this.#cancelled = false;
		this.view = { waiting: true, notice: null };

		const outcome = await ceremony.outcome;
		if (this.#ceremony !== ceremony) {
			// Superseded: the next ceremony owns the sheet now.
			throw outcome.kind === 'accepted'
				? new ClearSignerRefusedError('declined', 'superseded')
				: new ClearSignerRefusedError(outcome.code, outcome.detail);
		}
		this.#ceremony = null;
		if (outcome.kind === 'accepted') {
			this.view = { waiting: false, notice: null };
			return outcome.assertion;
		}
		this.view = { waiting: false, notice: this.#cancelled ? null : noticeOf(outcome.code) };
		throw new ClearSignerRefusedError(outcome.code, outcome.detail);
	}

	reopen(): void {
		this.#ceremony?.reopen();
	}

	cancel(): void {
		this.#cancelled = true;
		this.#ceremony?.cancel();
	}

	dismiss(): void {
		this.view = { ...this.view, notice: null };
	}
}

export const clearSignerSession = new ClearSignerSession();
