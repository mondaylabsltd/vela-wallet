/**
 * Where an approved request stands between the confirm and the answer (spec 079).
 *
 * The sheet stops being a form the moment the person approves: it says
 * "preparing" (the funding check, the nonce, the estimate), then "waiting for
 * the passkey" while the prompt is up (083 H3), then "submitting", and its ✕
 * may close it (the operation carrying on, the page still answered) once the
 * signature exists. The core's view cannot tell these apart — `is_signing`
 * and `is_submitting` are both true through its `Submitting` stage, which
 * spans the passkey prompt AND the submission — so the passkey ceremony
 * tells the shell itself (`onSignCeremony`), and this folds the two streams
 * into one fact per approval.
 *
 * Why it matters for money: closed while the prompt is up (or before it has
 * appeared), a cancelled prompt ends the attempt with no answer at all — the
 * sheet that would have carried the request is gone, and the page waits
 * forever. So the ✕ opens only once `signed` is true and no ceremony is up.
 */
import type { SignCeremonyEvent } from '$lib/onboarding/core/passkey';

export interface ApprovalProgress {
	/** The request the current approval belongs to; `null` when none is in flight. */
	requestId: string | null;
	/** A signature was produced since this approval began. */
	signed: boolean;
	/** A signing ceremony (the passkey prompt) is up right now. */
	ceremonyUp: boolean;
}

export const IDLE_APPROVAL: ApprovalProgress = {
	requestId: null,
	signed: false,
	ceremonyUp: false
};

export type ApprovalInput =
	/** A sign view: the request on the sheet, and whether a pipeline is running for it. */
	| { type: 'view'; requestId: string | null; inFlight: boolean }
	| { type: 'ceremony'; event: SignCeremonyEvent };

export function approvalProgress(prev: ApprovalProgress, input: ApprovalInput): ApprovalProgress {
	if (input.type === 'ceremony') {
		if (input.event === 'started') return { ...prev, ceremonyUp: true };
		// A signature counts only inside an approval: the same ceremony signs a
		// backup, a sign-in, a Settings key — none of them this sheet's.
		if (input.event === 'signed') {
			return { ...prev, ceremonyUp: false, signed: prev.requestId !== null || prev.signed };
		}
		return { ...prev, ceremonyUp: false };
	}
	if (!input.inFlight || input.requestId === null) {
		// Answered, refused, failed or closed: the next approval starts clean.
		return { requestId: null, signed: false, ceremonyUp: prev.ceremonyUp };
	}
	// A new approval (or a new request) starts unsigned; the same one keeps
	// what it has.
	if (prev.requestId !== input.requestId) {
		return { requestId: input.requestId, signed: false, ceremonyUp: prev.ceremonyUp };
	}
	return prev;
}
