/**
 * A multisig's waiting transactions as Activity rows: a contract call says
 * what it is — the core's reading of it, with who read it — as its
 * proposal's sheet does, never only "Contract call"; one nobody could read
 * keeps the multisig's own line.
 */
import { describe, expect, it, vi } from 'vitest';
import type { MultisigMessages } from '../messages';
import type { SigningMessages } from '$lib/signing/messages';

const MULTISIG = '0x' + 'cd'.repeat(20);
const call = (data: string, value = '0') => ({
	chainId: 100,
	safeTxHash: `0x${data.slice(2, 10)}`,
	review: { action: { kind: 'call', to: '0x' + 'ee'.repeat(20), value, data } },
	nonce: 1n,
	approved: 1,
	threshold: 2,
	status: 'waiting',
	canApprove: []
});
const seams = vi.hoisted(() => ({ open: [] as unknown[] }));

vi.mock('./proposals.svelte', () => ({
	proposals: {
		get open() {
			return seams.open;
		},
		stale: false
	}
}));
vi.mock('./display', () => ({
	actionTitle: async () => 'Contract call',
	formatUnits: (raw: string) => raw,
	tokenInfo: async () => ({ symbol: 'xDAI', decimals: 0 })
}));
vi.mock('./call-review.svelte', () => ({
	readCall: async (_chain: number, multisig: string, action: { data: string }) =>
		multisig === MULTISIG && action.data === '0x095ea7b3'
			? { title: 'Approve', source: 'Read by Vela, from the transaction' }
			: null
}));
vi.mock('$lib/flows/marks', () => ({ chainLogoURL: () => null }));
vi.mock('$lib/services/networks', () => ({ chainName: () => 'Gnosis' }));
vi.mock('$lib/wallet/fixtures', () => ({ chainColor: () => 'var(--color-fg-muted)' }));

const { waitingRows } = await import('./activity.svelte');

const m = {
	action: { call: 'Contract call' },
	status: {
		queued: 'Queued',
		ready: 'Ready',
		yourTurn: 'Your turn',
		waiting: 'Waiting',
		progress: '{{approved}}/{{threshold}}'
	},
	panel: { pending: 'Waiting', stale: 'stale' }
} as unknown as MultisigMessages;
const signing = {} as SigningMessages;
const settle = () => new Promise((done) => setTimeout(done, 0));

describe('a waiting contract call in Activity', () => {
	it('is titled by what the call is, and says who read it', async () => {
		seams.open = [call('0x095ea7b3'), call('0xdeadbeef')];
		waitingRows.follow(m, signing, MULTISIG);
		await settle();
		const rows = waitingRows.group(m)!.rows;
		expect(rows[0].title).toBe('Approve');
		expect(rows[0].subtitle).toBe('Read by Vela, from the transaction · Waiting · 1/2 · Gnosis');
		// Nobody could read it: the multisig's own line, and no source.
		expect(rows[1].title).toBe('Contract call');
		expect(rows[1].subtitle).toBe('Waiting · 1/2 · Gnosis');
	});

	it('keeps the coin a call sends in its amount, not twice in its title', async () => {
		seams.open = [call('0x0badc0de', '5')];
		waitingRows.follow(m, signing, MULTISIG);
		await settle();
		const row = waitingRows.group(m)!.rows[0];
		expect(row.title).toBe('Contract call');
		expect(row.amount).toBe('-5');
		expect(row.unit).toBe('xDAI');
	});
});
