/**
 * What the person is being asked to sign, as an agent reads it (spec 103,
 * `vela_review`): the transfer on its review, or a dApp's request on its
 * sheet. Decided in the core (`intent_rules::review`), with the note that
 * none of it is an instruction to the agent.
 */
import { intentReview } from '$lib/core/client';
import type { IntentActor } from '$lib/core/generated/IntentActor';
import type { IntentOperation } from '$lib/core/generated/IntentOperation';
import type { NetView } from '$lib/core/generated/NetView';
import type { SendView } from '$lib/core/generated/SendView';
import type { SignRequestView } from '$lib/core/generated/SignRequestView';

export type ReviewInput = {
	send: SendView | null;
	operation: IntentOperation | null;
	draftedBy: IntentActor | null;
	/** The signed-in account: what a transfer is sent from. */
	account: string | null;
	/** The networks, for a chain's name and native coin. */
	networks: NetView | null;
	signing: SignRequestView | null;
};

export function reviewOf(input: ReviewInput) {
	return JSON.parse(
		intentReview(
			JSON.stringify({
				send: input.send,
				operation: input.operation,
				drafted_by: input.draftedBy,
				account: input.account,
				networks: input.networks,
				signing: input.signing
			})
		)
	) as Record<string, unknown> & { note: string; transfer: unknown; signing: unknown };
}
