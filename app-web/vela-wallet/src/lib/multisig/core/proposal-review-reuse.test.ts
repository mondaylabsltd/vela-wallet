/**
 * r5reg-14: the review a sheet gathered is found again by the signing gate
 * — for exactly the same transaction, members and nonce, while it is fresh
 * — so approving does not gather the whole review a second time. Anything
 * else (a nonce moved, another member, too old) is gathered anew.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const reads = vi.hoisted(() => ({ simulations: 0, state: 'unchanged' }));
vi.mock('$lib/contacts/core/contacts-book.svelte', () => ({ contactsBook: { view: null } }));
vi.mock('$lib/session/core/session.svelte', () => ({ session: { view: { accounts: [] } } }));
vi.mock('./store.svelte', () => ({ multisig: { accounts: [] } }));
vi.mock('./names.svelte', () => ({ names: { label: () => null } }));
vi.mock('$lib/services/rpc-pool', () => ({ poolRpcCall: async () => ({ result: '0x' }) }));
vi.mock('$lib/services/recipient-risk', () => ({ resolveRecipientRisk: async () => null }));
vi.mock('$lib/services/sim/sim-trust', () => ({ judgeCheck: async () => null }));
vi.mock('$lib/services/sim/sim-engine-rpc', () => ({
	rpcSimulateReply: async () => {
		reads.simulations += 1;
		return null;
	}
}));
vi.mock('./kernel', () => ({
	reviewReads: () => ({
		needCode: [],
		simCalls: [{ to: '0x1', value: '0x0', data: '0x' }],
		trace: null,
		probe: null,
		recipients: []
	}),
	multisigReview: () => ({
		state: { source: 'trace', verdict: { kind: reads.state } },
		simulation: { kind: 'not_offered' }
	})
}));

const {
	forgetRecentReviews,
	gatherReview,
	recentReview,
	ProposalReview,
	REVIEW_FRESH_MS,
	REVIEW_RETRY_MS
} = await import('./proposal-review.svelte');

const subject = (nonce: bigint, owners = ['0xAA', '0xBB']) => ({
	safe: '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83',
	chainId: 5042002,
	tx: { to: '0x1', value: '0x0', data: '0x', nonce: '3' } as never,
	members: { deployed: true, owners, threshold: 2, nonce },
	genesis: null
});

beforeEach(() => {
	vi.useFakeTimers();
	forgetRecentReviews();
	reads.simulations = 0;
	reads.state = 'unchanged';
});
afterEach(() => vi.useRealTimers());

describe('a review gathered moments ago (r5reg-14)', () => {
	it('is found for the same transaction, members and nonce — any letter case', async () => {
		const gathered = await gatherReview(subject(3n));
		expect(reads.simulations).toBe(1);
		await vi.advanceTimersByTimeAsync(5_000);
		expect(recentReview(subject(3n, ['0xaa', '0xbb']))).toBe(gathered);
	});

	it('is not found once the nonce moved, the members changed, or it is old', async () => {
		await gatherReview(subject(3n));
		expect(recentReview(subject(4n))).toBeNull();
		expect(recentReview(subject(3n, ['0xAA', '0xCC']))).toBeNull();
		await vi.advanceTimersByTimeAsync(REVIEW_FRESH_MS + 1);
		expect(recentReview(subject(3n))).toBeNull();
	});
});

/**
 * r5reg-14: after an RPC outage the sheet said "couldn't check" for 25–35 s
 * — its review waited out the 30 s round. A review that could not read the
 * chain is read again every REVIEW_RETRY_MS: back within 5 s of the network.
 */
describe('a review read during an outage', () => {
	it('is read again within 5 s of the network coming back, not at the 30 s round', async () => {
		reads.state = 'unavailable';
		const review = new ProposalReview(subject(3n));
		await review.refresh();
		expect(review.unread).toBe(true);
		const stop = review.keepFresh();
		// Still down for a while: asked again every 5 s, never left for good.
		await vi.advanceTimersByTimeAsync(12_000);
		expect(reads.simulations).toBe(3);
		// The network is back now.
		reads.state = 'unchanged';
		const back = Date.now();
		while (review.unread && Date.now() - back < REVIEW_FRESH_MS * 2)
			await vi.advanceTimersByTimeAsync(250);
		expect(Date.now() - back).toBeLessThanOrEqual(REVIEW_RETRY_MS);
		// Read: back to the 30 s round — no 5 s reads while it reads fine.
		const before = reads.simulations;
		await vi.advanceTimersByTimeAsync(REVIEW_RETRY_MS * 2);
		expect(reads.simulations).toBe(before);
		stop();
		review.dispose();
	});
});
