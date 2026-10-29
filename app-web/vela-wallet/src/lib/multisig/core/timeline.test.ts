/**
 * r4D-17: an executed transaction's story — Created → Approved (each, when;
 * counted when it ran) → Executed (who, the transaction, when) — from the
 * mailbox's times and the facts frozen from its receipt, never guessed.
 */
import { describe, expect, it, vi } from 'vitest';

vi.mock('./kernel', () => ({
	sameAddress: (a?: string | null, b?: string | null) =>
		!!a && !!b && a.toLowerCase() === b.toLowerCase()
}));

const { timelineOf } = await import('./timeline');

const ALICE = '0x00000000000000000000000000000000000000a1';
const BOB = '0x00000000000000000000000000000000000000b0';
const CAROL = '0x00000000000000000000000000000000000000c0';
const TX = `0x${'ee'.repeat(32)}`;

const approval = (owner: string, createdMs: number, counts = true) =>
	({ owner, createdMs, counts, approval: { owner, signature: '0x' } }) as never;

const item = {
	proposer: ALICE,
	createdMs: 1_000,
	// Given out of order; Carol's is not a member's now.
	approvals: [approval(BOB, 3_000), approval(ALICE, 2_000), approval(CAROL, 2_500, false)]
};

describe('an executed transaction’s story', () => {
	it('created, each approval in order with whether it counted, then executed', () => {
		const steps = timelineOf(item, TX, {
			executor: BOB,
			threshold: 2,
			approvers: [ALICE, BOB],
			executorApproved: true,
			source: 'chain',
			atMs: 9_000
		});
		expect(steps).toEqual([
			{ kind: 'created', who: ALICE, atMs: 1_000 },
			{ kind: 'approved', who: ALICE, atMs: 2_000, counted: true, byExecuting: false },
			{ kind: 'approved', who: BOB, atMs: 3_000, counted: true, byExecuting: false },
			{ kind: 'executed', who: BOB, txHash: TX, atMs: 9_000 }
		]);
	});

	it('an approval given but not counted when it ran says so', () => {
		const steps = timelineOf(item, TX, {
			executor: CAROL,
			threshold: 1,
			approvers: [ALICE],
			executorApproved: false,
			source: 'chain',
			atMs: 9_000
		});
		expect(steps.filter((s) => s.kind === 'approved')).toEqual([
			{ kind: 'approved', who: ALICE, atMs: 2_000, counted: true, byExecuting: false },
			{ kind: 'approved', who: BOB, atMs: 3_000, counted: false, byExecuting: false }
		]);
	});

	it('the executor approving by sending it is a step of its own, at its time', () => {
		const steps = timelineOf({ ...item, approvals: [approval(ALICE, 2_000)] }, TX, {
			executor: BOB,
			threshold: 2,
			approvers: [ALICE, BOB],
			executorApproved: true,
			source: 'device',
			atMs: 9_000
		});
		expect(steps[2]).toEqual({
			kind: 'approved',
			who: BOB,
			atMs: 9_000,
			counted: true,
			byExecuting: true
		});
	});

	it('before its facts are read: who approved, counted unknown, executor and time unknown', () => {
		const steps = timelineOf({ ...item, createdMs: 0 }, TX, null);
		expect(steps[0]).toEqual({ kind: 'created', who: ALICE, atMs: null });
		expect(steps.filter((s) => s.kind === 'approved').map((s) => s.counted)).toEqual([null, null]);
		expect(steps.at(-1)).toEqual({ kind: 'executed', who: null, txHash: TX, atMs: null });
	});
});
