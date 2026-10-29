/**
 * An executed transaction's story (r4D-17): Created (who, when) → Approved
 * (each member, when; whether it counted when it ran) → Executed (who, the
 * transaction, when) — from the facts the wallet has, never guessed: the
 * mailbox's times of the proposal and of each approval, and the facts frozen
 * from its execution's receipt (`executed.ts`). A fact nobody holds is left
 * out (a time it does not know is `null`), never made up.
 */
import { sameAddress } from './kernel';
import type { FrozenFacts } from './executed';
import type { ProposalItem } from './proposals.svelte';

export type TimelineStep =
	| { kind: 'created'; who: string; atMs: number | null }
	| {
			kind: 'approved';
			who: string;
			atMs: number | null;
			/**
			 * Counted when it ran (the receipt's signers, or what this device
			 * sent); `null` while that is not known.
			 */
			counted: boolean | null;
			/** The executor approved by sending it: no approval of its own was given. */
			byExecuting: boolean;
	  }
	| { kind: 'executed'; who: string | null; txHash: string; atMs: number | null };

const known = (ms: number | undefined) => (typeof ms === 'number' && ms > 0 ? ms : null);

/**
 * The steps of `item`, executed by `txHash` with `frozen` facts (`null`: not
 * read yet — its approvals are then the mailbox's verified ones, counted
 * unknown). Approvals in the order they were given.
 */
export function timelineOf(
	item: Pick<ProposalItem, 'proposer' | 'createdMs' | 'approvals'>,
	txHash: string,
	frozen: FrozenFacts | null
): TimelineStep[] {
	const steps: TimelineStep[] = [
		{ kind: 'created', who: item.proposer, atMs: known(item.createdMs) }
	];
	const given = [...item.approvals]
		// An approval the mailbox holds but that is not a member's is no step
		// of this story unless it counted when it ran.
		.filter((a) => a.counts || frozen?.approvers.some((o) => sameAddress(o, a.owner)))
		.sort((a, b) => (known(a.createdMs) ?? Infinity) - (known(b.createdMs) ?? Infinity));
	for (const a of given) {
		steps.push({
			kind: 'approved',
			who: a.owner,
			atMs: known(a.createdMs),
			counted: frozen ? frozen.approvers.some((o) => sameAddress(o, a.owner)) : null,
			byExecuting: false
		});
	}
	// Signers the receipt shows that the mailbox held no approval of: the
	// executor, by sending it (or an approval given on chain).
	for (const owner of frozen?.approvers ?? []) {
		if (given.some((a) => sameAddress(a.owner, owner))) continue;
		const executor = !!frozen?.executor && sameAddress(frozen.executor, owner);
		steps.push({
			kind: 'approved',
			who: owner,
			atMs: executor ? (frozen?.atMs ?? null) : null,
			counted: true,
			byExecuting: executor && !!frozen?.executorApproved
		});
	}
	steps.push({
		kind: 'executed',
		who: frozen?.executor ?? null,
		txHash,
		atMs: frozen?.atMs ?? null
	});
	return steps;
}
