/**
 * §37 (review 1-13): the mailbox takes a post from anyone. A proposal no
 * member of the multisig approved is left out of the list and every count
 * (the core's `items`, `non_members`); the list ends with the way to see
 * them, and each one shown carries the warning.
 */
import { describe, expect, it, vi } from 'vitest';

vi.mock('./display', () => ({
	actionTitle: async () => 'Send 1.5 xDAI',
	memberActionTitle: () => null,
	memberWho: (address: string) => address,
	formatUnits: () => '1.5',
	tokenInfo: async () => ({ symbol: 'xDAI', decimals: 18 }),
	localName: () => null,
	shortenAddress: (address: string) => address
}));
vi.mock('$lib/core/kernels', () => ({ checksumAddress: (address: string) => address }));
vi.mock('$lib/flows/marks', () => ({ chainLogoURL: (id: number) => `/logo/${id}.svg` }));
vi.mock('$lib/services/networks', () => ({ chainName: () => 'Gnosis' }));
vi.mock('$lib/wallet/fixtures', () => ({ chainColor: () => 'var(--color-chain-gnosis)' }));
const sent = vi.hoisted(() => [] as unknown[]);
vi.mock('../shell/proposals-host.svelte', () => ({
	proposalsHost: { view: null, send: (event: unknown) => sent.push(event) },
	ProposeFailedError: class extends Error {}
}));

const { proposals } = await import('./proposals.svelte');
const { MULTISIG_NON_MEMBERS, waitingRows } = await import('./activity.svelte');

const m = {
	status: {
		ready: 'Ready to execute',
		yourTurn: 'Needs your approval',
		waiting: 'Waiting for others',
		queued: 'Queued',
		progress: '{{approved}}/{{threshold}} approved'
	},
	panel: {
		pending: 'Waiting for approval',
		stale: 'stale',
		nonMembersShow: 'Show proposals from non-members ({{total}})',
		nonMembersHide: 'Hide proposals from non-members',
		nonMembersWarning: 'No member of this multisig has approved this.'
	},
	change: { rowCaution: 'Changes who controls this multisig' }
} as never;

const to = { kind: 'transfer', token: null, to: '0x' + '11'.repeat(20), amount: '1' };
function item(hash: string, fromNonMember: boolean) {
	return {
		chainId: 100,
		safeTxHash: hash,
		review: { action: to },
		nonce: 0n,
		approved: 0,
		threshold: 2,
		status: 'open',
		canApprove: [],
		canExecute: [],
		sameNonce: [],
		executing: false,
		fromNonMember
	};
}

function view(nonMembers: number, showing: boolean) {
	vi.spyOn(proposals, 'view', 'get').mockReturnValue({
		waiting_note: null,
		standings: {},
		non_members: nonMembers,
		showing_non_members: showing
	} as never);
}

describe('proposals no member approved', () => {
	it('left out: the list ends with "Show proposals from non-members (2)"', () => {
		proposals.chains = [{ chainId: 100, items: [item('0xa', false)] }] as never;
		view(2, false);
		const rows = waitingRows.group(m)!.rows;
		expect(rows).toHaveLength(2);
		expect(rows[0].caution).toBeUndefined();
		expect(rows[1]).toMatchObject({
			id: MULTISIG_NON_MEMBERS,
			disclosure: true,
			title: 'Show proposals from non-members (2)'
		});
	});

	it('nothing else waiting: the group is still there, with only the way to see them', () => {
		proposals.chains = [{ chainId: 100, items: [] }] as never;
		view(1, false);
		const group = waitingRows.group(m);
		expect(group?.rows.map((r) => r.id)).toEqual([MULTISIG_NON_MEMBERS]);
	});

	it('shown: each carries the warning in the warning tone, and the last row hides them', () => {
		proposals.chains = [{ chainId: 100, items: [item('0xa', false), item('0xb', true)] }] as never;
		view(1, true);
		const rows = waitingRows.group(m)!.rows;
		expect(rows[0].caution).toBeUndefined();
		expect(rows[1]).toMatchObject({
			caution: 'No member of this multisig has approved this.',
			cautionTone: 'warning'
		});
		expect(rows[2]).toMatchObject({
			id: MULTISIG_NON_MEMBERS,
			title: 'Hide proposals from non-members'
		});
	});

	it('none left out: no disclosure row, and no group for nothing', () => {
		proposals.chains = [{ chainId: 100, items: [] }] as never;
		view(0, false);
		expect(waitingRows.group(m)).toBeNull();
	});

	it('the row tells the machine (`show_non_members`)', () => {
		proposals.showNonMembers(true);
		proposals.showNonMembers(false);
		expect(sent).toEqual([
			{ type: 'show_non_members', show: true },
			{ type: 'show_non_members', show: false }
		]);
	});
});
