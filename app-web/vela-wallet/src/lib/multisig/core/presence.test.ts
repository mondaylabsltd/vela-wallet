/**
 * r4D-7: what waits on this device — needs an approval from a member here,
 * or is ready to execute — counted from what this device read, and said in
 * one line.
 */
import { describe, expect, it, vi } from 'vitest';
import type { MultisigMessages } from '../messages';
import type { ProposalItem } from './proposals.svelte';

vi.mock('./proposals.svelte', () => ({
	proposals: { followed: null, items: [] },
	readChain: async () => ({ items: [] }),
	splitListed: () => ({ lists: new Map(), used: [], missing: [] })
}));
// The core's CLDR rule, as English has it.
vi.mock('$lib/core/client', () => ({
	i18nPluralSuffix: (_l: string, n: number) => (n === 1 ? '_one' : '_other')
}));
vi.mock('./service', () => ({ fetchAllProposals: async () => null }));
vi.mock('./service-state.svelte', () => ({ serviceOutage: { blocked: () => false } }));
vi.mock('./store.svelte', () => ({ multisig: { accounts: [] } }));
vi.mock('./kernel', () => ({
	sameAddress: (a: string | null, b: string | null) =>
		!!a && !!b && a.toLowerCase() === b.toLowerCase()
}));

const { waitingOf, waitingWords } = await import('./presence.svelte');

const item = (over: Partial<ProposalItem>): ProposalItem =>
	({
		chainId: 100,
		safeTxHash: `0x${String(over.nonce ?? 0n).padStart(64, '0')}`,
		nonce: 0n,
		approved: 0,
		threshold: 2,
		status: 'pending',
		canApprove: [],
		canExecute: [],
		cancelledBy: null,
		...over
	}) as ProposalItem;

const ME = { address: '0xaa' } as ProposalItem['canApprove'][number];

describe('what waits here (r4D-7)', () => {
	it('counts one needing this device’s approval and one ready to execute', () => {
		const w = waitingOf([
			item({ nonce: 1n, canApprove: [ME], approved: 1 }),
			item({ nonce: 0n, status: 'ready', approved: 2, canApprove: [ME] }),
			// Someone else's turn: nothing for this device.
			item({ nonce: 2n, status: 'queued', approved: 0 }),
			// Done, or voided: not waiting.
			item({ nonce: 3n, status: 'done', canApprove: [ME] }),
			item({ nonce: 4n, canApprove: [ME], cancelledBy: '0x1' })
		]);
		expect(w.needsYou).toBe(1);
		expect(w.ready).toBe(1);
		expect(w.first?.safeTxHash).toBe(`0x${'1'.padStart(64, '0')}`);
	});

	it('says it in one line, plurals by count; nothing when nothing waits', () => {
		const forms = (one: string, other: string) => ({
			locale: 'en',
			forms: { _one: one, _other: other }
		});
		const m = {
			presence: {
				needsYou: forms('{{count}} needs your approval', '{{count}} need your approval'),
				ready: forms('{{count}} ready to execute', '{{count}} ready to execute')
			}
		} as unknown as MultisigMessages;
		expect(waitingWords({ needsYou: 1, ready: 1, first: null }, m)).toBe(
			'1 needs your approval · 1 ready to execute'
		);
		expect(waitingWords({ needsYou: 2, ready: 0, first: null }, m)).toBe('2 need your approval');
		expect(waitingWords({ needsYou: 0, ready: 0, first: null }, m)).toBe('');
	});
});
