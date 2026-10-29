/**
 * Changing a multisig's members or the approvals it needs, network by
 * network (owner, 2026-10-29: a Safe's owners on one chain never change
 * another's). One action of the person's — add, remove, replace, a new
 * threshold — is one proposal per network the multisig is deployed on, each
 * built by the core against THAT network's members as its chain holds them
 * (`memberChangeTx`: the previous owner from that network's own list); a
 * network it is not deployed on starts with the original members when first
 * used, and nothing here deploys it.
 *
 * Nothing here signs or sends: `proposeMemberChange` (proposals) does, one
 * network at a time, and the proposals then go the way every other does —
 * approved, executed, read back from the chain.
 */
import type { ChainMembers } from './chain';
import {
	memberChangeTx,
	MEMBER_CHANGES,
	sameAddress,
	type Action,
	type MemberChange,
	type MemberRefusal,
	type MemberState,
	type MemberVerdict
} from './kernel';
import type { ProposalItem, ProposalStatus } from './proposals.svelte';

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

/** A network as Settings knows it: its members now, and what waits there. */
export interface NetworkMembers {
	chainId: number;
	members: ChainMembers;
	items: Pick<ProposalItem, 'status' | 'review'>[];
}

export interface MemberPlan {
	/** Each network it is deployed on, and what the change is there (or why not). */
	networks: { chainId: number; verdict: MemberVerdict }[];
	/** The networks a proposal goes to: those where the core built it. */
	ready: number[];
	/** Networks it is not deployed on: they start with the original members; nothing is offered. */
	undeployed: number[];
}

/**
 * What `change` would be on each network, as shown before anything is
 * signed: before → after where the core builds it, its reason where it
 * does not. The nonce is the proposal's to take when it is made (read again
 * then); this is the picture, not the transaction.
 */
export function planMemberChange(
	multisig: string,
	chains: NetworkMembers[],
	change: MemberChange
): MemberPlan {
	const networks = chains
		.filter((c) => c.members.deployed)
		.map((c) => ({
			chainId: c.chainId,
			verdict: memberChangeTx(multisig, memberStateOf(c), change, '0x0')
		}));
	return {
		networks,
		ready: networks.filter((n) => n.verdict.kind === 'ready').map((n) => n.chainId),
		undeployed: chains.filter((c) => !c.members.deployed).map((c) => c.chainId)
	};
}

/**
 * Everyone who is a member on some network it is deployed on, in the first
 * network's order and then whoever only another has — the rows a member is
 * removed or replaced from (where a network does not have one, the core
 * says so for that network).
 */
export function deployedMembers(chains: Pick<NetworkMembers, 'members'>[]): string[] {
	const out: string[] = [];
	for (const c of chains) {
		if (!c.members.deployed) continue;
		for (const o of c.members.owners) if (!out.some((x) => sameAddress(x, o))) out.push(o);
	}
	return out;
}

/**
 * One change as the person asked for it, whatever network it waits on — the
 * same member added, removed or replaced, or the same new threshold — so the
 * proposals of one action read as one change with per-network progress. On
 * any device: nothing is stored to tie them, the transactions say it.
 * `null`: not a member change on its own (a batch is its own thing).
 */
export function changeKey(action: Action | null): string | null {
	switch (action?.kind) {
		case 'addOwner':
			return `add:${action.owner.toLowerCase()}`;
		case 'removeOwner':
			return `remove:${action.owner.toLowerCase()}`;
		case 'swapOwner':
			return `replace:${action.oldOwner.toLowerCase()}:${action.newOwner.toLowerCase()}`;
		case 'changeThreshold':
			return `threshold:${action.threshold}`;
		default:
			return null;
	}
}

export interface ChangePart {
	chainId: number;
	safeTxHash: string;
	approved: number;
	threshold: number;
	status: ProposalStatus;
}

export interface ChangeGroup {
	key: string;
	action: Action;
	/** One per network it waits on, in the order the networks are listed. */
	parts: ChangePart[];
}

/**
 * The member changes still open (not executed, not voided), one group per
 * change; each group's networks in `order` (the wallet's network order),
 * whichever was proposed first.
 */
export function memberChangeGroups(
	items: Pick<
		ProposalItem,
		'chainId' | 'safeTxHash' | 'approved' | 'threshold' | 'status' | 'review'
	>[],
	order: number[] = []
): ChangeGroup[] {
	const rank = (chainId: number) => {
		const i = order.indexOf(chainId);
		return i === -1 ? order.length : i;
	};
	const groups: ChangeGroup[] = [];
	for (const i of items) {
		const key = changeKey(i.review.action);
		if (key === null || i.status === 'done' || !i.review.action) continue;
		const part: ChangePart = {
			chainId: i.chainId,
			safeTxHash: i.safeTxHash,
			approved: i.approved,
			threshold: i.threshold,
			status: i.status
		};
		const group = groups.find((g) => g.key === key);
		if (group) group.parts.push(part);
		else groups.push({ key, action: i.review.action, parts: [part] });
	}
	for (const g of groups) g.parts.sort((a, b) => rank(a.chainId) - rank(b.chainId));
	return groups;
}

/** "Arc 1/2 · Base 0/2": each network's approvals so far, of what it needs there. */
export function progressLine(group: ChangeGroup, chainName: (chainId: number) => string): string {
	return group.parts
		.map((p) => `${chainName(p.chainId)} ${Math.min(p.approved, p.threshold)}/${p.threshold}`)
		.join(' · ');
}
