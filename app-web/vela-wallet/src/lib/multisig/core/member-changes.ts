/**
 * What the web's own proposer (`proposeMemberChange`, proposals) still needs
 * of a member change until the proposals machine proposes it: whether a
 * member change already waits on a network, and the core's refusal as an
 * error.
 *
 * Everything else of changing the members — each network's members, the
 * change composed and planned per deployed network, the changes waiting
 * grouped per action — is the core's members machine's
 * (`vela_core::app::multisig_members`, `MemberChanges.svelte`).
 */
import { MEMBER_CHANGES, type Action, type MemberRefusal, type MemberState } from './kernel';
import type { ChainMembers } from './chain';
import type { ProposalItem } from './proposals.svelte';

/** Whether `action` changes who controls the multisig (alone, or a call of a batch). */
export function changesMembers(action: Action | null): boolean {
	if (!action) return false;
	if (action.kind === 'batch') return action.calls.some((c) => changesMembers(c.action));
	return (MEMBER_CHANGES as readonly string[]).includes(action.kind);
}

/**
 * A network's members as the core builds a change against them: its owners
 * in the chain's list order, and whether a member change already waits
 * there (one at a time: a second, built on today's list, could name the
 * wrong previous owner once the first runs).
 */
export function memberStateOf(chain: {
	members: Pick<ChainMembers, 'owners' | 'threshold'>;
	items: Pick<ProposalItem, 'status' | 'review'>[];
}): MemberState {
	return {
		owners: chain.members.owners,
		threshold: chain.members.threshold,
		changeWaiting: chain.items.some((i) => i.status !== 'done' && changesMembers(i.review.action))
	};
}

/** The core refused a member change on a network (its reason). */
export class MemberChangeRefusedError extends Error {
	constructor(readonly refusal: MemberRefusal) {
		super(`the member change was refused: ${refusal.kind}`);
		this.name = 'MemberChangeRefusedError';
	}
}
