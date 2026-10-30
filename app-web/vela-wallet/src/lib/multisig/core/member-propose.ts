/**
 * Proposing a member change on one network — the ONE way the members panel
 * (`MemberChanges.svelte`) proposes: a row of the members machine's plan
 * (`PlanNetwork`), its `proposal` (always a `member_change`) on its
 * `chain_id`, as it is.
 *
 * W8 (proposals surface): this goes into the proposals machine's
 * `{ type: 'draft', chain_id, proposal }`, which builds the transaction again
 * against that chain at propose time. Until it drives the web, the web's
 * `proposeMemberChange` does (one passkey, approved at once). What came of it
 * is said in {@link ProposeOutcome}, whichever proposes.
 */
import type { MultisigProposalKind } from '../generated/MultisigProposalKind';
import type { MemberChangeRequest, MemberRefusal } from './kernel';
import { MemberChangeRefusedError } from './member-changes';
import { MailboxUnreadableError, proposeMemberChange } from './proposals.svelte';

export type ProposeOutcome =
	| { ok: true; safeTxHash: string }
	/** The passkey (or the risk question) was declined: nothing to say. */
	| { ok: false; declined: true }
	| {
			ok: false;
			declined?: false;
			/** The core refused it on that network now (its members moved, say). */
			refusal?: MemberRefusal;
			/** The mailbox could not be read: no free number to propose at. */
			mailboxDown?: true;
			/** Anything else, in the error's own words. */
			message?: string;
	  };

/** The proposal's change as the web's proposer takes it (no threshold: the network's). */
function request(proposal: MultisigProposalKind): MemberChangeRequest | null {
	if (proposal.kind !== 'member_change') return null;
	const c = proposal.change;
	switch (c.kind) {
		case 'add':
		case 'remove':
			return { kind: c.kind, owner: c.owner, threshold: c.threshold ?? undefined };
		case 'replace':
			return { kind: 'replace', oldOwner: c.oldOwner, newOwner: c.newOwner };
		case 'threshold':
			return { kind: 'threshold', threshold: c.threshold };
	}
}

export async function proposeChange(
	address: string,
	chainId: number,
	proposal: MultisigProposalKind
): Promise<ProposeOutcome> {
	const change = request(proposal);
	if (!change) return { ok: false, message: `not a member change: ${proposal.kind}` };
	try {
		const { safeTxHash } = await proposeMemberChange(address, chainId, change);
		return { ok: true, safeTxHash };
	} catch (error) {
		const name = error instanceof DOMException ? error.name : '';
		if (name === 'NotAllowedError' || name === 'AbortError') return { ok: false, declined: true };
		if (error instanceof MemberChangeRefusedError) return { ok: false, refusal: error.refusal };
		if (error instanceof MailboxUnreadableError) return { ok: false, mailboxDown: true };
		return { ok: false, message: error instanceof Error ? error.message : String(error) };
	}
}
