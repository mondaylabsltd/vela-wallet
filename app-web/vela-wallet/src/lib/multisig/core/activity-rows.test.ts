/**
 * vis-18: a multisig's waiting transfer reads as the other Activity rows do —
 * the chain logo on the glyph, the typographic minus, and whom it goes to.
 */
import { describe, expect, it, vi } from 'vitest';

vi.mock('./display', () => ({
	actionTitle: async () => 'Send 1.5 xDAI',
	formatUnits: () => '1.5',
	tokenInfo: async () => ({ symbol: 'xDAI', decimals: 18 }),
	localName: (address: string) =>
		address === '0x1111111111111111111111111111111111111111' ? 'Alice' : null,
	shortenAddress: (address: string) => `${address.slice(0, 8)}…${address.slice(-6)}`
}));
vi.mock('$lib/core/kernels', () => ({
	checksumAddress: (address: string) => address.replace('abcdef', 'AbCdEf')
}));
vi.mock('$lib/flows/marks', () => ({ chainLogoURL: (id: number) => `/logo/${id}.svg` }));
vi.mock('$lib/services/networks', () => ({ chainName: () => 'Gnosis' }));
vi.mock('$lib/wallet/fixtures', () => ({ chainColor: () => 'var(--color-chain-gnosis)' }));

const { proposals } = await import('./proposals.svelte');
const { waitingRows } = await import('./activity.svelte');

const m = {
	status: {
		ready: 'Ready to execute',
		yourTurn: 'Needs your approval',
		waiting: 'Waiting for others',
		queued: 'Queued',
		progress: '{{approved}}/{{threshold}} approved'
	},
	panel: { pending: 'Waiting for approval', stale: 'stale' }
} as never;

function item(action: unknown, hash: string) {
	return {
		chainId: 100,
		safeTxHash: hash,
		review: { action },
		nonce: 0n,
		approved: 1,
		threshold: 2,
		status: 'open',
		canApprove: [],
		canExecute: [],
		sameNonce: [],
		executing: false
	};
}

describe('a waiting multisig transaction in Activity', () => {
	it('carries the chain logo, the typographic minus and "To …"', async () => {
		proposals.chains = [
			{
				chainId: 100,
				items: [
					item(
						{
							kind: 'transfer',
							token: null,
							to: '0x1111111111111111111111111111111111111111',
							amount: '1'
						},
						'0xa'
					),
					item(
						{
							kind: 'transfer',
							token: null,
							to: '0x22222222222222222222222222222222abcdef00',
							amount: '1'
						},
						'0xb'
					),
					item({ kind: 'changeThreshold', threshold: '2' }, '0xc')
				]
			}
		] as never;
		// Where each stands is the machine's word (`standings`), worded here.
		vi.spyOn(proposals, 'view', 'get').mockReturnValue({
			waiting_note: null,
			standings: {
				'100:0xd': { kind: 'ready' },
				'100:0xe': { kind: 'yourTurn' },
				'100:0xf': { kind: 'queued' }
			}
		} as never);
		waitingRows.follow(m);
		await vi.waitFor(() =>
			expect(waitingRows.group(m, 'To {{name}}')?.rows[0].amount).not.toBe('')
		);
		const rows = waitingRows.group(m, 'To {{name}}')!.rows;
		expect(rows[0].badgeLogoUrl).toBe('/logo/100.svg');
		expect(rows[0].amount).toBe('−1.5');
		expect(rows[0].subtitle).toBe('1/2 approved · To Alice');
		// No name on this device: the checksummed short form.
		expect(rows[1].subtitle).toBe('1/2 approved · To 0x222222…CdEf00');
		// Not a transfer: the network, as before.
		expect(rows[2].subtitle).toBe('1/2 approved · Gnosis');
	});

	it('says one thing about where it stands, so it fits two lines at 320px (r4D-13)', async () => {
		const to = {
			kind: 'transfer',
			token: null,
			to: '0x1111111111111111111111111111111111111111',
			amount: '1'
		};
		proposals.chains = [
			{
				chainId: 100,
				items: [
					{ ...item(to, '0xd'), approved: 2, status: 'ready' },
					{ ...item(to, '0xe'), canApprove: ['0x1111111111111111111111111111111111111111'] },
					{ ...item(to, '0xf'), status: 'queued' }
				]
			}
		] as never;
		// Where each stands is the machine's word (`standings`), worded here.
		vi.spyOn(proposals, 'view', 'get').mockReturnValue({
			waiting_note: null,
			standings: {
				'100:0xd': { kind: 'ready' },
				'100:0xe': { kind: 'yourTurn' },
				'100:0xf': { kind: 'queued' }
			}
		} as never);
		waitingRows.follow(m);
		await vi.waitFor(() =>
			expect(waitingRows.group(m, 'To {{name}}')?.rows[0].amount).not.toBe('')
		);
		const rows = waitingRows.group(m, 'To {{name}}')!.rows;
		// Never a status AND its count: "Ready to execute" already says 2/2.
		expect(rows.map((r) => r.subtitle)).toEqual([
			'Ready to execute · To Alice',
			'Needs your approval · To Alice',
			'Queued · To Alice'
		]);
	});
});
