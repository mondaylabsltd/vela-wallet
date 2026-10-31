/**
 * The one review of a multisig transaction is the proposals machine's
 * (`review` event → `view.reviews`, docs/multisig-vault-design.md §3.6): it
 * reads, decides and keeps. What stays here is how a sheet draws its
 * simulation (the token rules for the Safe's moves) and how often an open
 * sheet asks the machine for the review again.
 */
import { judgeCheck } from '$lib/services/sim/sim-trust';
import type { SimEnded } from '$lib/signing/verdict';
import type { MultisigReviewOut } from './kernel';

/**
 * The simulation as the signing sheet draws it (`SimEnded`): the core's
 * verdict, its moves judged by the token rules for the Safe.
 */
export async function simEndedOf(
	review: MultisigReviewOut,
	chainId: number,
	safe: string
): Promise<SimEnded> {
	const verdict = review.simulation;
	if (verdict.kind !== 'deltas') return { verdict, judged: null };
	const judged = await judgeCheck(verdict.deltas, chainId, safe).catch(() => null);
	return { verdict, judged };
}

/** How often a sheet reads its review again while it is open. */
export const REVIEW_FRESH_MS = 30_000;
/**
 * r5reg-14: how soon a review that could not read the chain (its state or
 * its simulation unreachable — an RPC outage) is read again. Recovery took
 * 25–35 s: such a review waited out the 30 s round, "couldn't check" on
 * screen long after the network was back.
 */
export const REVIEW_RETRY_MS = 5_000;
