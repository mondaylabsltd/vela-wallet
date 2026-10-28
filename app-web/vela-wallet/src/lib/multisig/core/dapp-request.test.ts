/**
 * A dApp's transaction from a multisig (dapp-1, dapp-2): its review comes
 * first and nothing is signed before "Propose and approve"; every way out of
 * the sheet answers the site, once and honestly. A `wallet_sendCalls` is one
 * proposal of every call, its site answered the batch's id once proposed.
 */
import { describe, expect, it, vi } from 'vitest';
import {
	ANSWER_MARGIN_MS,
	DECLINED,
	DappGoneError,
	PROPOSED_CODE,
	leftProposed,
	multisigRefusal,
	openMultisigReview,
	type DappAnswer,
	type HandOff
} from './dapp-request';
import type { DappProposal } from './ui.svelte';

const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const HASH = `0x${'ab'.repeat(32)}`;
const CALL = { to: '0x000000000000000000000000000000000000dEaD', value: '0x1', data: '0x' };

function harness(overrides: Partial<HandOff> = {}) {
	const answers: DappAnswer[] = [];
	const events: string[] = [];
	let sheet: DappProposal | null = null;
	const propose = vi.fn(async () => {
		events.push('passkey');
		return { safeTxHash: HASH, nonce: '0x3', fromBlock: '0x10' };
	});
	const h: HandOff = {
		rid: 'r1',
		method: 'eth_sendTransaction',
		address: SAFE,
		chainId: 100,
		calls: [CALL],
		origin: 'https://app.example',
		open: (request) => {
			events.push('review');
			sheet = request;
		},
		proposed: (_rid, hash) => events.push(`proposed ${hash.slice(0, 4)}`),
		closeSheet: () => events.push('sheet closed'),
		claim: async () => true,
		gone: () => events.push('gone'),
		propose,
		settle: (answer) => answers.push(answer),
		follow: () => events.push('following'),
		tellProposed: (hash) => events.push(`told ${hash.slice(0, 4)}`),
		answerBatch: (hash, nonce, fromBlock) =>
			events.push(`batch ${hash.slice(0, 4)} ${nonce} ${fromBlock}`),
		...overrides
	};
	const { closing } = openMultisigReview(h);
	return { answers, events, propose, closing, sheet: () => sheet! };
}

describe("a dApp's transaction from a multisig", () => {
	it('opens its review and signs nothing until the person proposes it', async () => {
		const { events, propose, sheet, answers } = harness();
		expect(events).toEqual(['review']);
		expect(propose).not.toHaveBeenCalled();
		expect(sheet().safeTxHash).toBeNull();
		expect(sheet().origin).toBe('https://app.example');
		await sheet().propose();
		expect(propose).toHaveBeenCalledWith(SAFE, 100, [CALL]);
		expect(events).toEqual(['review', 'passkey', 'proposed 0xab', 'following']);
		// Proposed is not answered: the site waits for the execution.
		expect(answers).toEqual([]);
	});

	it('Decline, or a close before proposing, is the person rejecting it (4001)', () => {
		const declined = harness();
		declined.sheet().decline();
		expect(declined.answers).toEqual([{ error: DECLINED }]);
		expect(declined.answers[0].error?.code).toBe(4001);
		expect(declined.propose).not.toHaveBeenCalled();

		const closed = harness();
		closed.sheet().close();
		expect(closed.answers).toEqual([{ error: DECLINED }]);
		expect(closed.events).toContain('sheet closed');
	});

	it('a close after proposing says it was proposed — never a rejection, never a blank wait', async () => {
		const { sheet, answers, closing } = harness();
		await sheet().propose();
		expect(closing()).toEqual({ error: leftProposed(HASH) });
		sheet().close();
		expect(answers).toHaveLength(1);
		expect(answers[0].error?.code).not.toBe(4001);
		expect(answers[0].error?.message).toContain(HASH);
		expect(answers[0].error?.message).toMatch(/Proposed/);
	});

	it('"proposed" is a JSON-RPC server error carrying the proposal — never 4001, 4900 or a hash', () => {
		const answer = leftProposed(HASH);
		expect(answer.code).toBe(PROPOSED_CODE);
		expect(answer.code).toBe(-32000);
		expect(answer.data).toEqual({ safeTxHash: HASH, status: 'proposed' });
	});

	it('executed from the sheet: the site gets the real transaction hash, and no "proposed" later', async () => {
		vi.useFakeTimers();
		try {
			const TX = `0x${'cd'.repeat(32)}`;
			const { sheet, answers, events } = harness({ limitMs: 300_000 });
			await sheet().propose();
			sheet().executed(TX);
			expect(answers).toEqual([{ result: TX }]);
			vi.advanceTimersByTime(300_000);
			expect(events.some((e) => e.startsWith('told'))).toBe(false);
		} finally {
			vi.useRealTimers();
		}
	});

	it('waiting for approvals, the site is told it was proposed before the request limit', async () => {
		vi.useFakeTimers();
		try {
			let clock = 1_000;
			const { sheet, answers, events } = harness({
				limitMs: 300_000,
				now: () => clock,
				// The passkey takes 30 s: the limit counts from the claim before it.
				propose: async () => {
					clock += 30_000;
					return { safeTxHash: HASH, nonce: '0x3' };
				}
			});
			await sheet().propose();
			// 270 s after the claim minus the margin, measured from now (30 s in).
			vi.advanceTimersByTime(300_000 - ANSWER_MARGIN_MS - 30_000 - 1);
			expect(events).not.toContain('told 0xab');
			vi.advanceTimersByTime(1);
			expect(events).toContain('told 0xab');
			// Told, not settled here: the host answers once (its own guard).
			expect(answers).toEqual([]);
		} finally {
			vi.useRealTimers();
		}
	});

	it('a close after proposing stops the limit: answered once', async () => {
		vi.useFakeTimers();
		try {
			const { sheet, answers, events } = harness({ limitMs: 300_000 });
			await sheet().propose();
			sheet().close();
			vi.advanceTimersByTime(300_000);
			expect(answers).toEqual([{ error: leftProposed(HASH) }]);
			expect(events.some((e) => e.startsWith('told'))).toBe(false);
		} finally {
			vi.useRealTimers();
		}
	});

	it('a page that left before the passkey gets nothing signed', async () => {
		const { sheet, propose, events } = harness({ claim: async () => false });
		await expect(sheet().propose()).rejects.toBeInstanceOf(DappGoneError);
		expect(propose).not.toHaveBeenCalled();
		expect(events).toContain('gone');
	});

	it('a dismissed passkey leaves the review up, with nothing answered', async () => {
		const dismissed = new DOMException('no', 'NotAllowedError');
		const { sheet, answers } = harness({
			propose: async () => {
				throw dismissed;
			}
		});
		await expect(sheet().propose()).rejects.toBe(dismissed);
		expect(answers).toEqual([]);
		expect(sheet().safeTxHash).toBeNull();
	});

	it('a batch (wallet_sendCalls) is one proposal of every call, its id answered once proposed', async () => {
		const SECOND = {
			to: '0x000000000000000000000000000000000000bEEF',
			value: '0x0',
			data: '0x1234'
		};
		const { events, propose, sheet, answers } = harness({
			method: 'wallet_sendCalls',
			calls: [CALL, SECOND]
		});
		expect(sheet().calls).toEqual([CALL, SECOND]);
		expect(sheet().method).toBe('wallet_sendCalls');
		expect(propose).not.toHaveBeenCalled();
		await sheet().propose();
		// Every call in the one proposal; the site told its id and the number
		// it took, never followed for an execution hash.
		expect(propose).toHaveBeenCalledWith(SAFE, 100, [CALL, SECOND]);
		expect(events).toEqual(['review', 'passkey', 'proposed 0xab', 'batch 0xab 0x3 0x10']);
		expect(answers).toEqual([]);
		// Its site already has the id: an execution here answers nothing more.
		sheet().executed(`0x${'cd'.repeat(32)}`);
		expect(answers).toEqual([]);
	});
});

describe('what a multisig refuses', () => {
	it('takes a transaction and a batch of them, and refuses a message as a message', () => {
		expect(multisigRefusal('eth_sendTransaction')).toBeNull();
		expect(multisigRefusal('wallet_sendCalls')).toBeNull();
		expect(multisigRefusal('personal_sign')?.message).toMatch(/signs no messages/);
		expect(multisigRefusal('eth_signTypedData_v4')?.code).toBe(4200);
	});
});
