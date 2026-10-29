/**
 * r4a-8: an executed transaction reads the facts frozen when it ran — from
 * its receipt (the core's `executionFacts`), else what this device recorded
 * when it executed it — never today's owners re-counted; and never more
 * approvals than its threshold (the "4/3" of the round-4 walk).
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const A = '0x1111111111111111111111111111111111111111';
const B = '0x2222222222222222222222222222222222222222';
const C = '0x3333333333333333333333333333333333333333';
const TX = `0x${'ee'.repeat(32)}`;

const chain = vi.hoisted(() => ({
	receipt: null as unknown,
	facts: null as unknown,
	asked: [] as unknown[]
}));
vi.mock('$lib/services/rpc-pool', () => ({
	poolRpcCall: async (method: string) => {
		if (method !== 'eth_getTransactionReceipt') throw new Error(method);
		return { result: chain.receipt };
	}
}));
vi.mock('./kernel', () => ({
	sameAddress: (a?: string | null, b?: string | null) =>
		!!a && !!b && a.toLowerCase() === b.toLowerCase(),
	executionFacts: (...args: unknown[]) => {
		chain.asked.push(args);
		return chain.facts;
	}
}));

const store = new Map<string, string>();
vi.stubGlobal('localStorage', {
	getItem: (k: string) => store.get(k) ?? null,
	setItem: (k: string, v: string) => void store.set(k, v),
	removeItem: (k: string) => void store.delete(k)
});

const { factsOfExecution, frozenFacts, rememberExecution } = await import('./executed');

let hash = 0;
const nextHash = () => `0x${(++hash).toString(16).padStart(64, '0')}`;

beforeEach(() => {
	chain.receipt = null;
	chain.facts = null;
	chain.asked = [];
	store.clear();
});

describe('the facts an executed transaction keeps', () => {
	it('reads them from its receipt, by the core', async () => {
		chain.receipt = { logs: [{ address: SAFE, topics: ['0x01'], data: '0x', extra: 1 }] };
		chain.facts = {
			executor: C,
			threshold: 2,
			nonce: '7',
			approvers: [A, C],
			executorApproved: true
		};
		const safeTxHash = nextHash();
		expect(await frozenFacts(100, SAFE, safeTxHash, TX)).toEqual({
			executor: C,
			threshold: 2,
			approvers: [A, C],
			executorApproved: true,
			source: 'chain'
		});
		// The receipt's logs as the node gave them, and nothing else of it.
		expect(chain.asked[0]).toEqual([
			100,
			SAFE,
			safeTxHash,
			[{ address: SAFE, topics: ['0x01'], data: '0x' }]
		]);
	});

	it('falls back to what this device recorded when it executed it', async () => {
		chain.receipt = { logs: [] };
		const safeTxHash = nextHash();
		rememberExecution(100, SAFE, safeTxHash, factsOfExecution([A], B, true, 2));
		expect(await frozenFacts(100, SAFE, safeTxHash, TX)).toEqual({
			executor: B,
			threshold: 2,
			approvers: [A, B],
			executorApproved: true,
			source: 'device'
		});
	});

	it('claims nothing when neither says', async () => {
		expect(await frozenFacts(100, SAFE, nextHash(), TX)).toBeNull();
	});
});

describe('what an execution from this device counts', () => {
	it('counts an executor who approved already once', () => {
		const facts = factsOfExecution([A, C], C, true, 2);
		expect(facts.approvers).toEqual([A, C]);
		expect(facts.executorApproved).toBe(true);
	});

	it('never more than the threshold', () => {
		// Three approvals and an owner executing, two needed: two count —
		// the first two in address order, as Safe reads them.
		const facts = factsOfExecution([C, B, A], B, true, 2);
		expect(facts.approvers).toEqual([A, B]);
		expect(facts.approvers.length).toBe(facts.threshold);
	});

	it('an executor that is no owner adds no approval', () => {
		const facts = factsOfExecution([A, B], '0x9999999999999999999999999999999999999999', false, 2);
		expect(facts.approvers).toEqual([A, B]);
		expect(facts.executorApproved).toBe(false);
	});
});
