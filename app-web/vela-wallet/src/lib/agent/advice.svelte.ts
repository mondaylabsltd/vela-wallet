/**
 * The signing sheet and the agent's opinion on it (spec 103): what the sheet
 * holds beside the request, published for `vela_review`, and the one
 * opinion the agent gave on the request on screen.
 *
 * Light on purpose — the signing host imports it wherever a request can
 * arrive, the extension's request window included, and the bridge itself is
 * only loaded when the person has turned the agent on.
 */
import { intentAdviceStanding } from '$lib/core/client';
import type { FeeView } from '$lib/core/generated/FeeView';
import type { NetView } from '$lib/core/generated/NetView';
import { reviewOf } from '$lib/intents/review';
import { multisig } from '$lib/multisig/core/store.svelte';
import { session } from '$lib/session/core/session.svelte';
import { getEthereumDataURL } from '$lib/services/endpoints';
import { signingSheet } from '$lib/signing/core/sheet.svelte';
import { signRequest } from '$lib/signing/core/sign-resident.svelte';
import type { SimEnded } from '$lib/signing/verdict';
import type { AcceptedAdvice, AdviceStanding, OpenReview } from './advisor';

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
 * A multisig proposal's review on screen (§3.6): the request the core's
 * `multisig_review` names for it (`adviceRequest` — bound to its
 * safeTxHash on its chain) and what its sheet reads of it. An opinion on it
 * is read, bound and drawn as one on a dApp's request is: the same digest,
 * the same standing (current, or given for an earlier version).
 */
export interface ProposalFacts {
	id: string | null;
	signing: Record<string, unknown>;
	sheet: Record<string, unknown>;
}

class ProposalBeside {
	open = $state.raw<ProposalFacts | null>(null);

	publish(facts: ProposalFacts): void {
		this.open = facts;
		// An opinion goes with its proposal.
		const held = sheetBeside.advice;
		if (held !== null && held.requestId.startsWith('multisig:') && held.requestId !== facts.id) {
			sheetBeside.advice = null;
		}
	}

	withdraw(): void {
		this.open = null;
	}
}

export const proposalBeside = new ProposalBeside();

/** The id an opinion given now is held under: the dApp request's, else the proposal's. */
export function openRequestId(): string | null {
	return signRequest.view.request?.id ?? proposalBeside.open?.id ?? null;
}

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
	if (request === null) return openProposalReview(networks);
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

/**
 * A multisig proposal's review on screen, read by the core as any signing
 * request (`intent_rules::review`): its digest covers the proposal's
 * safeTxHash, chain, reading, state-diff guard and simulation.
 */
function openProposalReview(networks: NetView | null): OpenReview | null {
	const open = proposalBeside.open;
	if (open === null || open.id === null) return null;
	const review = reviewOf({
		send: null,
		operation: null,
		draftedBy: null,
		account: (open.signing.signer_address as string | undefined) ?? null,
		networks,
		signing: open.signing as unknown as Parameters<typeof reviewOf>[0]['signing'],
		sheet: { ...open.sheet, descriptor_base: getEthereumDataURL() }
	});
	const digest = (review.signing as { review_digest?: unknown } | null)?.review_digest;
	return typeof digest === 'string' ? { review_digest: digest, review } : null;
}

/** The fee an open review shows (its `signing.fee`), as the core reads it. */
export function reviewFee(open: OpenReview | null): unknown {
	return (open?.review as { signing?: { fee?: unknown } } | undefined)?.signing?.fee ?? null;
}

/**
 * How the opinion held stands against the review on the sheet now — its
 * digest, and the fee it was given for against the fee on screen — the
 * core's call (`agent_review::advice_standing`).
 */
export function adviceStanding(advice: AcceptedAdvice, open: OpenReview | null): AdviceStanding {
	return JSON.parse(
		intentAdviceStanding(
			JSON.stringify({
				advice,
				current_digest: open?.review_digest ?? null,
				current_fee: reviewFee(open)
			})
		)
	) as AdviceStanding;
}
