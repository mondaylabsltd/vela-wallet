/**
 * The landing a dApp transaction shows (spec 077 FR-002, owner's B-6).
 *
 * The owner asked for "和转账一样，有上链的那个等待动画". Same component, same
 * words, same three states — so these tests are about the states being the
 * SEND's, not about a second design.
 */
import { describe, expect, it } from 'vitest';
import {
	dappReceiptModel,
	landingToRaise,
	receiptProgress,
	type DappReceiptCopy
} from './dapp-receipt';
import { ringProgress } from '$lib/flows/ui/ring';

const copy: DappReceiptCopy = {
	confirming: 'Confirming…',
	confirmingHint: 'Sent! Confirming on-chain — the explorer link will appear shortly.',
	submitted: 'Submitted',
	confirmed: 'Confirmed',
	failed: 'Failed',
	failedHint: 'The transaction reverted on-chain.',
	opHashLabel: 'UserOp Hash',
	txHashLabel: 'Tx Hash',
	explorer: 'Explorer',
	done: 'Done'
};

const OP = '0x' + 'a1'.repeat(32);
const TX = '0x' + 'b2'.repeat(32);
const explorer = (hash: string) => `https://gnosisscan.io/tx/${hash}`;
const none = () => null;

describe('the receipt a dApp transaction lands on', () => {
	it('spins while it is being submitted, and names no hash it does not have', () => {
		const model = dappReceiptModel({ kind: 'submitting' }, copy, explorer);
		expect(model.stage).toBe('submitting');
		expect(model.title).toBe(copy.confirming);
		expect(model.hash).toBeUndefined();
		expect(model.explorer).toBeUndefined();
	});

	it('shows the OPERATION hash while it is submitted, never a transaction hash', () => {
		// There is no transaction until it lands. Labelling the operation hash
		// "Tx Hash" sends a person to search an explorer for something that is
		// not there yet.
		const model = dappReceiptModel({ kind: 'submitted', opHash: OP }, copy, explorer);
		expect(model.stage).toBe('submitted');
		expect(model.hash).toEqual({ label: copy.opHashLabel, value: OP });
		expect(model.captions).toEqual([copy.confirmingHint]);
		// And no explorer button while there is nothing on one to look at.
		expect(model.explorer).toBeUndefined();
	});

	it('ends on the transaction hash and its explorer', () => {
		const model = dappReceiptModel({ kind: 'confirmed', opHash: OP, txHash: TX }, copy, explorer);
		expect(model.stage).toBe('confirmed');
		expect(model.hash).toEqual({ label: copy.txHashLabel, value: TX });
		expect(model.explorer).toEqual({ label: copy.explorer, url: explorer(TX) });
	});

	it('a chain with no explorer draws no button rather than a dead one', () => {
		const model = dappReceiptModel({ kind: 'confirmed', opHash: OP, txHash: TX }, copy, none);
		expect(model.explorer).toBeUndefined();
		// …and still says what happened.
		expect(model.stage).toBe('confirmed');
		expect(model.hash?.value).toBe(TX);
	});

	it('a reverted transaction says so, and still names the operation', () => {
		const model = dappReceiptModel({ kind: 'failed', opHash: OP }, copy, explorer);
		expect(model.stage).toBe('failed');
		expect(model.captions).toEqual([copy.failedHint]);
		expect(model.hash?.value).toBe(OP);
	});

	it('every state offers the one way out', () => {
		const states = [
			{ kind: 'submitting' as const },
			{ kind: 'submitted' as const, opHash: OP },
			{ kind: 'confirmed' as const, opHash: OP, txHash: TX },
			{ kind: 'failed' as const, opHash: OP }
		];
		for (const state of states) {
			expect(dappReceiptModel(state, copy, explorer).cta).toBe(copy.done);
		}
	});
});

describe('which operation raises a landing', () => {
	it('raises one for an operation it has not shown yet', () => {
		expect(landingToRaise(OP, null)).toBe(OP);
	});

	it('does NOT raise the same one again — which is what made Done work', () => {
		// The handoff STAYS in the view after the receipt is dismissed. Gating on
		// "no landing is showing" meant Done cleared it and the watcher put it
		// straight back: a receipt a person could not get out of. Measured in the
		// packaged extension, where neither a trusted click nor a synthetic one
		// could dismiss it.
		expect(landingToRaise(OP, OP)).toBeNull();
	});

	it('raises a new one for the NEXT operation', () => {
		// The panel stays open and takes request after request; a dismissed
		// receipt must not deafen it to the one after.
		expect(landingToRaise(TX, OP)).toBe(TX);
	});

	it('raises nothing when there is no operation to follow', () => {
		// A message, a refusal, a transaction with nothing at the bundler.
		expect(landingToRaise(null, null)).toBeNull();
		expect(landingToRaise(undefined, OP)).toBeNull();
		expect(landingToRaise('', null)).toBeNull();
	});
});

describe('the ring that fills while it waits', () => {
	it("draws the SEND receipt's curve, not a second one that looks like it", () => {
		// The owner asked for "和转账一样". `ringProgress` is the send's own, so
		// this checks the two agree rather than re-deriving the easing here.
		expect(receiptProgress(1_000, 20, 1_000)).toBe(ringProgress(0, 20));
		expect(receiptProgress(1_000, 20, 21_000)).toBe(ringProgress(20, 20));
	});

	it('is about 70% at the time the chain usually takes', () => {
		expect(receiptProgress(1_000, 20, 21_000)).toBeCloseTo(0.69, 2);
	});

	it('never closes before the tick does', () => {
		// A full ring beside the word "Submitted" reads as a finished
		// transaction that is not finished. Only the confirmation closes it.
		expect(receiptProgress(1_000, 20, 1_000_000)).toBeCloseTo(0.92, 4);
		expect(receiptProgress(1_000, 20, 1_000_000)).toBeLessThan(0.92001);
	});

	it('a chain with no typical time circles instead of filling', () => {
		expect(receiptProgress(1_000, 0, 5_000)).toBeUndefined();
	});

	it('a clock that went backwards does not draw a negative ring', () => {
		expect(receiptProgress(10_000, 20, 1_000)).toBe(0);
	});
});
