/**
 * r4D-7: what waits on this device — needs an approval from a member here,
 * or is ready to execute — as the proposals machine counts it (its rule:
 * rust/crates/vela-core/tests/app_multisig_presence.rs,
 * `counts_one_needing_an_approval_here_and_one_ready`,
 * `a_ready_one_nobody_here_can_pay_for_is_not_ready`), said in one line.
 */
import { describe, expect, it, vi } from 'vitest';
import type { MultisigMessages } from '../messages';
import type { ProposalItem } from './proposals.svelte';

const seams = vi.hoisted(() => ({
	/** The proposals machine's word on the multisigs not shown, by lowercase address. */
	machine: {} as Record<string, unknown>,
	accounts: [] as { address: string }[],
	/** The machine's word on whether a multisig not shown has something waiting. */
	othersWaiting: false,
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
			return { presence: seams.machine, others_waiting: seams.othersWaiting };
		},
		othersTick: async () => {},
		followOthers: () => () => {}
	}
}));
// The core's CLDR rule, as English has it.
vi.mock('$lib/core/client', () => ({
	i18nPluralSuffix: (_l: string, n: number) => (n === 1 ? '_one' : '_other')
}));
const { presence, waitingWords } = await import('./presence.svelte');

describe('what waits here, said (r4D-7)', () => {
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
		// Whether another has something waiting is the machine's word.
		seams.othersWaiting = true;
		expect(presence.othersWaiting).toBe(true);
		seams.othersWaiting = false;
		expect(presence.othersWaiting).toBe(false);
	});
});
