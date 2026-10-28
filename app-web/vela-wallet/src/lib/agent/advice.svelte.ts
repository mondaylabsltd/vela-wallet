/**
 * The signing sheet and the agent's opinion on it (spec 103): what the sheet
 * holds beside the request, published for `vela_review`, and the one
 * opinion the agent gave on the request on screen.
 *
 * Light on purpose — the signing host imports it wherever a request can
 * arrive, the extension's request window included, and the bridge itself is
 * only loaded when the person has turned the agent on.
 */
import type { FeeView } from '$lib/core/generated/FeeView';
import type { NetView } from '$lib/core/generated/NetView';
import { reviewOf } from '$lib/intents/review';
import { multisig } from '$lib/multisig/core/store.svelte';
import { session } from '$lib/session/core/session.svelte';
import { getEthereumDataURL } from '$lib/services/endpoints';
import { signingSheet } from '$lib/signing/core/sheet.svelte';
import { signRequest } from '$lib/signing/core/sign-resident.svelte';
import type { SimEnded } from '$lib/signing/verdict';
import type { AcceptedAdvice, OpenReview } from './advisor';

/** What the signing host knows of the request on its sheet, beside the machines' views. */
export interface SigningFacts {
	requestId: string | null;
	/** The sheet's simulation, once it ended. */
	sim: SimEnded | null;
	/** A simulation is out for this request. */
	simPending: boolean;
	/** The fee in force for this request. */
	fee: FeeView | null;
}

class SheetBeside {
	facts = $state<SigningFacts>({ requestId: null, sim: null, simPending: false, fee: null });
	/** The agent's opinion, for the request it was given on. */
	advice = $state<{ requestId: string; advice: AcceptedAdvice } | null>(null);

	publish(facts: SigningFacts): void {
		this.facts = facts;
		// An opinion goes with its request.
		if (this.advice !== null && this.advice.requestId !== facts.requestId) this.advice = null;
	}

	advise(requestId: string, advice: AcceptedAdvice): void {
		this.advice = { requestId, advice };
	}
}

export const sheetBeside = new SheetBeside();

/**
 * The sheet's facts for `intent_rules::review` (`agent_review::SheetFacts`):
 * the clear-signing and guard views, and — for this request only — the
 * simulation and the fee.
 */
export function sheetFacts(): Record<string, unknown> {
	const id = signRequest.view.request?.id ?? null;
	const facts = sheetBeside.facts;
	const mine = id !== null && facts.requestId === id;
	return {
		clear: signingSheet.clear,
		guard: signingSheet.guard,
		fee: mine ? facts.fee : null,
		simulation: mine ? facts.sim : null,
		simulation_pending: mine && facts.simPending,
		descriptor_base: getEthereumDataURL()
	};
}

/** The account a review is read for: the multisig in front, else the session's own. */
function reviewAccount(): string | null {
	return (multisig.active?.address ?? session.view.address) || null;
}

/**
 * The dApp signing review on the sheet now, with its digest — `null` when
 * no request is open. `networks` only names the fee's coin; the digest does
 * not read it.
 */
export function openReview(networks: NetView | null = null): OpenReview | null {
	const request = signRequest.view.request ?? null;
	if (request === null) return null;
	const review = reviewOf({
		send: null,
		operation: null,
		draftedBy: null,
		account: reviewAccount(),
		networks,
		signing: request,
		sheet: sheetFacts()
	});
	const digest = (review.signing as { review_digest?: unknown } | null)?.review_digest;
	return typeof digest === 'string' ? { review_digest: digest, review } : null;
}
