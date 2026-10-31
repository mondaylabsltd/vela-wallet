/**
 * r4-5: the home never says "Ready to execute" for a transaction that cannot
 * run — it says "Can't run now — <why>", the machine's standing worded (the
 * rule: app_multisig_proposals.rs `a_ready_one_that_would_fail_says_why_on_the_home`).
 */
import { describe, expect, it, vi } from 'vitest';

const SAFE = '0x00000000000000000000000000000000000000cc';
const ME = '0x00000000000000000000000000000000000000aa';
const said = vi.hoisted(() => ({ standing: null as unknown }));

vi.mock('./display', () => ({
	actionTitle: async () => 'Send 0.5 USDC',
	formatUnits: () => '0.5',
	tokenInfo: async () => ({ symbol: 'USDC', decimals: 18 }),
	localName: () => null,
	shortenAddress: (address: string) => address.slice(0, 8)
}));
vi.mock('$lib/core/kernels', () => ({ checksumAddress: (address: string) => address }));
vi.mock('$lib/flows/marks', () => ({ chainLogoURL: () => '' }));
vi.mock('$lib/services/networks', () => ({
	chainName: () => 'Arc Testnet',
	nativeSymbol: () => 'USDC'
}));
vi.mock('$lib/wallet/fixtures', () => ({ chainColor: () => 'var(--color-chain-arc)' }));
const { proposals } = await import('./proposals.svelte');
const { waitingRows } = await import('./activity.svelte');
vi.spyOn(proposals, 'view', 'get').mockImplementation(
	() => ({ standings: { '5042002:0xa': said.standing }, waiting_note: null }) as never
);

const m = {
	status: {
		ready: 'Ready to execute',
		yourTurn: 'Needs your approval',
		waiting: 'Waiting for others',
		queued: 'Queued',
		progress: '{{approved}}/{{threshold}} approved',
		cannotRun: 'Can’t run now — {{reason}}',
		nobodyPays: 'no account on this device can pay the network fee'
	},
	proposal: { whyShort: 'the multisig holds {{have}}, and this needs {{need}}' },
	panel: { pending: 'Waiting for approval', stale: 'stale' }
} as never;

function ready() {
	return {
		chainId: 5042002,
		safeTxHash: '0xa',
		tx: {},
		review: { action: { kind: 'changeThreshold', threshold: '2' } },
		nonce: 0n,
		approved: 2,
		threshold: 2,
		status: 'ready',
		approvals: [],
		canApprove: [],
		canExecute: [{ address: ME, name: 'Mine', account: { address: ME } }],
		sameNonce: [],
		executing: false
	};
}

function subtitle(): string {
	return waitingRows.group(m)!.rows[0].subtitle ?? '';
}

describe('a ready transaction on the home (r4-5)', () => {
	it('says why it cannot run, never "Ready to execute"', () => {
		proposals.chains = [
			{
				chainId: 5042002,
				members: { owners: [ME], threshold: 2, nonce: 0n, deployed: true },
				items: [ready()],
				mailbox: true
			}
		] as never;
		waitingRows.follow(m, null, SAFE);

		said.standing = { kind: 'ready' };
		expect(subtitle()).toContain('Ready to execute');

		said.standing = {
			kind: 'blocked',
			block: {
				kind: 'fails',
				why: { kind: 'short', token: null, have: '280000000000000000', need: '500000000000000000' }
			}
		};
		expect(subtitle()).toContain(
			'Can’t run now — the multisig holds 0.28 USDC, and this needs 0.5 USDC'
		);
		expect(subtitle()).not.toContain('Ready to execute');

		said.standing = { kind: 'blocked', block: { kind: 'nobodyPays' } };
		expect(subtitle()).toContain(
			'Can’t run now — no account on this device can pay the network fee'
		);
	});
});
