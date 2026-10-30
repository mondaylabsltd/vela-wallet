/**
 * r4D-7: what waits on this device — needs an approval from a member here,
 * or is ready to execute — counted from what this device read, and said in
 * one line.
 */
import { describe, expect, it, vi } from 'vitest';
import type { MultisigMessages } from '../messages';
import type { ProposalItem } from './proposals.svelte';

const seams = vi.hoisted(() => ({
	/** The proposals machine's word on the multisigs not shown, by lowercase address. */
	machine: {} as Record<string, unknown>,
	accounts: [] as { address: string }[],
	/** r5reg-6: each ready one's executor look, by its hash. */
	looks: {} as Record<string, { kind: string; canPay?: boolean }>,
	followed: { address: null as string | null, items: [] as ProposalItem[] }
}));
vi.mock('./proposals.svelte', () => ({
	proposals: {
		get followed() {
			return seams.followed.address;
		},
		get items() {
			return seams.followed.items;
		},
		chain: () => ({ members: { owners: [], threshold: 1, deployed: true } })
	},
	executionInput: (_a: unknown, item: { safeTxHash: string }) => item.safeTxHash
}));
vi.mock('../shell/proposals-host.svelte', () => ({
	proposalsHost: {
		get view() {
			return { presence: seams.machine };
		},
		othersTick: async () => {},
		followOthers: () => () => {}
	}
}));
// The core's CLDR rule, as English has it.
vi.mock('$lib/core/client', () => ({
	i18nPluralSuffix: (_l: string, n: number) => (n === 1 ? '_one' : '_other')
}));
vi.mock('./preflight.svelte', () => ({
	preflight: { get: (hash: string) => seams.looks[hash] ?? null },
	blockOf: (looks: ({ kind: string; canPay?: boolean } | null)[]) =>
		looks.length > 0 && looks.every((l) => l?.kind === 'priced' && !l.canPay)
			? { kind: 'nobodyPays' }
			: null
}));
vi.mock('./store.svelte', () => ({
	multisig: {
		get accounts() {
			return seams.accounts;
		},
		find: (a: string) => seams.accounts.find((x) => x.address.toLowerCase() === a.toLowerCase())
	}
}));
vi.mock('./kernel', () => ({
	sameAddress: (a: string | null, b: string | null) =>
		!!a && !!b && a.toLowerCase() === b.toLowerCase()
}));

const { presence, waitingOf, waitingWords } = await import('./presence.svelte');

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
			item({ nonce: 0n, status: 'ready', approved: 2, canApprove: [ME], canExecute: [ME] }),
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
				ready: forms('{{count}} ready to execute', '{{count}} ready to execute'),
				waitingPayer: forms(
					'{{count}} waiting for a member who can pay',
					'{{count}} waiting for a member who can pay'
				)
			}
		} as unknown as MultisigMessages;
		const w = (needsYou: number, ready: number, waitingPayer = 0) => ({
			needsYou,
			ready,
			waitingPayer,
			first: null
		});
		expect(waitingWords(w(1, 1), m)).toBe('1 needs your approval · 1 ready to execute');
		expect(waitingWords(w(2, 0), m)).toBe('2 need your approval');
		expect(waitingWords(w(0, 0), m)).toBe('');
		expect(waitingWords(w(0, 0, 1), m)).toBe('1 waiting for a member who can pay');
	});
});

/**
 * r5reg-6: "ready to execute" counts only what someone here can run now; one
 * nobody here can pay for (or with no member here) waits for a member who
 * can — said as such, as its row says "Can't run now".
 */
describe('ready, as far as this device can run it (r5reg-6)', () => {
	it('a ready one nobody here can pay for is not "ready to execute"', () => {
		seams.accounts = [{ address: '0xAA' }];
		const ready = (n: bigint, canExecute = [ME]) =>
			item({ nonce: n, status: 'ready', approved: 2, canExecute });
		const payable = ready(0n);
		const unpaid = ready(1n);
		const nobodyHere = ready(2n, []);
		const unread = ready(3n);
		seams.looks = {
			[payable.safeTxHash]: { kind: 'priced', canPay: true },
			[unpaid.safeTxHash]: { kind: 'priced', canPay: false }
		};
		seams.followed = { address: '0xAA', items: [payable, unpaid, nobodyHere, unread] };
		const w = presence.of('0xaa');
		// The one that can be paid, and the one whose look is still out.
		expect(w?.ready).toBe(2);
		expect(w?.waitingPayer).toBe(2);
		seams.followed = { address: null, items: [] };
	});

	it('without looks, a ready one with a member here counts as ready', () => {
		const w = waitingOf([item({ nonce: 0n, status: 'ready', approved: 2, canExecute: [ME] })]);
		expect(w).toMatchObject({ ready: 1, waitingPayer: 0 });
	});
});

/**
 * r4b-11 × r4D-7: a multisig not shown counts what the proposals machine
 * says of it — read now and then, else its kept waiting list (the core's
 * rule: `the_others_start_from_the_kept_list_then_are_read`) — and nothing at
 * all, never a zero, before it says anything.
 */
describe('a multisig not shown', () => {
	it('counts what the machine read, in this module’s words', () => {
		seams.accounts = [{ address: '0xBB' }, { address: '0xCC' }];
		seams.machine = {
			'0xbb': {
				needs_you: 1,
				ready: 0,
				waiting_payer: 0,
				first: { chain_id: 100, safe_tx_hash: '0x01' }
			},
			'0xcc': null
		};
		expect(presence.of('0xBB')).toEqual({
			needsYou: 1,
			ready: 0,
			waitingPayer: 0,
			first: { chainId: 100, safeTxHash: '0x01' }
		});
		expect(presence.of('0xCC')).toBeNull();
		expect(presence.othersWaiting(null)).toBe(true);
		expect(presence.othersWaiting('0xbb')).toBe(false);
	});
});
