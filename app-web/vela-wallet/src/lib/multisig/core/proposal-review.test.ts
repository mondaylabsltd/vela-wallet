/**
 * The one review's reads are bounded (round 5, as the preview's): a node that
 * never answers is said as not answering — the core reads `{unreachable}` as
 * a state not read and a move not simulated — and never holds the sheet's
 * buttons, or the signing gate that reads through here, for good.
 */
import { describe, expect, it, vi } from 'vitest';

const never = <T>() => new Promise<T>(() => {});

const OWN = '0x00000000000000000000000000000000000000a1';
const SAFE_HERE = '0x00000000000000000000000000000000000000a2';
const MEMBER = '0x00000000000000000000000000000000000000a3';
const CONTACT = '0x00000000000000000000000000000000000000a4';

vi.mock('$lib/contacts/core/contacts-book.svelte', () => ({
	contactsBook: { view: { contacts: [{ address: CONTACT, name: 'Dana' }] } }
}));
vi.mock('$lib/session/core/session.svelte', () => ({
	session: { view: { accounts: [{ account: { address: OWN, name: 'R5 Bob' } }] } }
}));
vi.mock('./store.svelte', () => ({
	multisig: { accounts: [{ address: SAFE_HERE, genesis: { name: 'Team' } }] }
}));
vi.mock('./names.svelte', () => ({
	names: { label: (a: string) => (a === MEMBER ? 'Carol' : `${a.slice(0, 6)}…`) }
}));
vi.mock('$lib/services/rpc-pool', () => ({ poolRpcCall: () => never() }));
vi.mock('$lib/services/sim/sim-engine-rpc', () => ({ rpcSimulateReply: () => never() }));
vi.mock('$lib/services/recipient-risk', () => ({ resolveRecipientRisk: () => never() }));
vi.mock('$lib/services/sim/sim-trust', () => ({ judgeCheck: async () => null }));
vi.mock('./kernel', () => ({
	reviewReads: () => ({
		needCode: [],
		simCalls: [{ to: '0x00000000000000000000000000000000000000fe', value: '0x0', data: '0x' }],
		trace: { method: 'debug_traceCall', params: [] },
		probe: { method: 'eth_call', params: [] },
		unavailable: null,
		recipients: ['0x00000000000000000000000000000000000000fe'],
		tokens: []
	}),
	multisigReview: vi.fn()
}));

const { gatherReview, knownAddresses, REVIEW_READ_MS } = await import('./proposal-review.svelte');

/**
 * r4a-18, r5reg-8: whose each address the person knows is, for the core —
 * one's own accounts and multisigs here, the members by this device's name
 * for them, the contacts — so it never calls one's own a stranger.
 */
describe('the addresses the person knows', () => {
	it('says whose each is', () => {
		expect(knownAddresses([MEMBER])).toEqual([
			{ address: CONTACT, label: 'Dana' },
			{ address: OWN, label: 'R5 Bob', kind: 'own' },
			{ address: SAFE_HERE, label: 'Team', kind: 'own' },
			{ address: MEMBER, label: 'Carol', kind: 'member' }
		]);
	});
});

describe("the review's reads, when the node never answers", () => {
	it('end at the bound, each said as not answering', async () => {
		vi.useFakeTimers();
		try {
			const gathered = gatherReview({
				safe: '0x00000000000000000000000000000000000000cc',
				chainId: 100,
				tx: {} as never,
				members: { deployed: true, owners: [], threshold: 1 },
				genesis: null
			});
			await vi.advanceTimersByTimeAsync(REVIEW_READ_MS);
			const input = await gathered;
			expect(input.trace).toEqual({ unreachable: true });
			expect(input.probe).toEqual({ unreachable: true });
			expect(input.simReply).toBeNull();
			expect(input.recipientFacts).toEqual([
				{
					address: '0x00000000000000000000000000000000000000fe',
					isContract: null,
					firstTime: null
				}
			]);
		} finally {
			vi.useRealTimers();
		}
	});
});
