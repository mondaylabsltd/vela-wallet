/**
 * The landing a dApp transaction shows (spec 077 FR-002, owner's B-6).
 *
 * The owner asked for "和转账一样，有上链的那个等待动画". Same component, same
 * words, same three states — so these tests are about the states being the
 * SEND's, not about a second design.
 */
import { describe, expect, it } from 'vitest';
import { dappReceiptModel, receiptProgress, type DappReceiptCopy } from './dapp-receipt';

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

describe('the ring that fills while it waits', () => {
	it('fills with the time the chain usually takes', () => {
		expect(receiptProgress(1_000, 20, 1_000)).toBe(0);
		expect(receiptProgress(1_000, 20, 11_000)).toBeCloseTo(0.5);
	});

	it('never closes before the tick does', () => {
		// A full ring beside the word "Submitted" reads as a finished
		// transaction that is not finished.
		expect(receiptProgress(1_000, 20, 1_000_000)).toBe(0.95);
	});

	it('a chain with no typical time circles instead of filling', () => {
		expect(receiptProgress(1_000, 0, 5_000)).toBeUndefined();
	});

	it('a clock that went backwards does not draw a negative ring', () => {
		expect(receiptProgress(10_000, 20, 1_000)).toBe(0);
	});
});
