/**
 * Proposing a member change on one network — the ONE way the members panel
 * (`MemberChanges.svelte`) proposes: a row of the members machine's plan
 * (`PlanNetwork`), its `proposal` (always a `member_change`) on its
 * `chain_id`, as it is, to the proposals machine (`draft`, which builds the
 * transaction again against that chain at propose time, its review, then
 * one passkey). What came of it is said in {@link ProposeOutcome}.
 */
import type { MultisigProposalKind } from '../generated/MultisigProposalKind';
import type { MemberRefusal } from './kernel';
import { multisig } from './store.svelte';
import { proposalsHost, ProposeFailedError } from '../shell/proposals-host.svelte';
import { actionError, memberRefusal } from '../shell/proposal-failure';

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
			/** r13-3: the read the core named (`ActionState.unread.reason_key`). */
			unreadKey?: string;
			/** Anything else, in the error's own words. */
			message?: string;
	  };

export async function proposeChange(
	address: string,
	chainId: number,
	proposal: MultisigProposalKind
): Promise<ProposeOutcome> {
	if (proposal.kind !== 'member_change') {
		return { ok: false, message: `not a member change: ${proposal.kind}` };
	}
	const account = multisig.find(address);
	if (!account) return { ok: false, message: 'not a multisig on this device' };
	try {
		const { safeTxHash } = await proposalsHost.propose(account, chainId, proposal);
		return { ok: true, safeTxHash };
	} catch (error) {
		if (!(error instanceof ProposeFailedError)) {
			return { ok: false, message: error instanceof Error ? error.message : String(error) };
		}
		const state = error.state;
		switch (state.kind) {
			case 'cancelled':
			case 'riskDeclined':
				return { ok: false, declined: true };
			case 'mailboxUnreadable':
				return { ok: false, mailboxDown: true };
			case 'unread':
				return { ok: false, unreadKey: state.reason_key };
			case 'refused': {
				const refusal = memberRefusal(state.reason);
				return refusal
					? { ok: false, refusal: refusal as MemberRefusal }
					: { ok: false, message: state.reason };
			}
			default: {
				// Anything else, in the error's own words (as the sheet reads it).
				const said = actionError(state, chainId);
				return { ok: false, message: said instanceof Error ? said.message : state.kind };
			}
		}
	}
}
