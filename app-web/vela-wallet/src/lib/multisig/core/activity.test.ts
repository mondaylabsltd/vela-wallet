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
const seams = vi.hoisted(() => ({
	open: [] as unknown[],
	missing: [] as { chainId: number; count: number }[],
	stale: false,
	unreachable: false
}));

vi.mock('./proposals.svelte', () => ({
	proposals: {
		get open() {
			return seams.open;
		},
		get missing() {
			return seams.missing;
		},
		get stale() {
			return seams.stale;
		},
		get unreachable() {
			return seams.unreachable;
		}
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
// The chain index: it knows chain 77777 by name, and not 99999.
vi.mock('$lib/services/chain-doc', () => ({
	readChainDoc: async (chainId: number) =>
		chainId === 77777 ? { kind: 'doc', body: { name: 'Member Chain' } } : { kind: 'absent' }
}));

const { MULTISIG_RETRY, missingOfRow, waitingRows } = await import('./activity.svelte');

const m = {
	action: { call: 'Contract call' },
	status: {
		queued: 'Queued',
		ready: 'Ready',
		yourTurn: 'Your turn',
		waiting: 'Waiting',
		progress: '{{approved}}/{{threshold}}'
	},
	proposal: {
		missingNetworkNamed: 'On {{name}} (chain {{chainId}}) — not here',
		missingNetwork: 'On chain {{chainId}} — not here',
		missingCount: 'Waiting there: {{n}}',
		addNetwork: 'Add network'
	},
	panel: { pending: 'Waiting', stale: 'stale' },
	service: { unreachableNote: 'service away', retry: 'Retry' }
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
		expect(rows[0].subtitle).toBe('Read by Vela, from the transaction · 1/2 · Gnosis');
		// Nobody could read it: the multisig's own line, and no source.
		expect(rows[1].title).toBe('Contract call');
		expect(rows[1].subtitle).toBe('1/2 · Gnosis');
	});

	it('keeps the coin a call sends in its amount, not twice in its title', async () => {
		seams.open = [call('0x0badc0de', '5')];
		waitingRows.follow(m, signing, MULTISIG);
		await settle();
		const row = waitingRows.group(m)!.rows[0];
		expect(row.title).toBe('Contract call');
		expect(row.amount).toBe('\u22125');
		expect(row.unit).toBe('xDAI');
	});
});

describe('the waiting group while the multisig service is away', () => {
	it('says so once, over the last list, with a Retry', () => {
		seams.open = [call('0xdeadbeef')];
		seams.stale = true;
		seams.unreachable = true;
		const group = waitingRows.group(m)!;
		expect(group.note).toBe('service away');
		expect(group.action).toEqual({ id: MULTISIG_RETRY, label: 'Retry' });
		expect(group.rows).toHaveLength(1);
		// Nothing waiting is not "no activity": the group stays to say it.
		seams.open = [];
		expect(waitingRows.group(m)?.note).toBe('service away');
	});

	it('a list that is only stale says that, with no Retry of its own', () => {
		seams.open = [call('0xdeadbeef')];
		seams.stale = true;
		seams.unreachable = false;
		const group = waitingRows.group(m)!;
		expect(group.note).toBe('stale');
		expect(group.action).toBeUndefined();
		seams.stale = false;
		expect(waitingRows.group(m)?.note).toBeUndefined();
	});
});

/**
 * Proposals the mailbox holds on a network this device lacks: in the
 * waiting group, by the chain index's name when it has one, with Add network
 * — never judged, and never anyone's turn here.
 */
describe('proposals on a network this device does not have', () => {
	it('are a row of their own per network, with the way to add it', async () => {
		seams.open = [];
		seams.stale = false;
		seams.unreachable = false;
		seams.missing = [
			{ chainId: 99999, count: 2 },
			{ chainId: 77777, count: 1 }
		];
		waitingRows.follow(m, signing, MULTISIG);
		await settle();
		const group = waitingRows.group(m)!;
		expect(group.rows.map((r) => [r.id, r.title, r.subtitle])).toEqual([
			['multisig-network:99999', 'On chain 99999 — not here', 'Waiting there: 2 · Add network'],
			[
				'multisig-network:77777',
				'On Member Chain (chain 77777) — not here',
				'Waiting there: 1 · Add network'
			]
		]);
		// No amount, no progress, no "your turn": nothing about it is counted.
		expect(group.rows.every((r) => r.amount === '' && !/Your turn|\//.test(r.subtitle))).toBe(true);
		expect(missingOfRow(group.rows[0].id)).toBe(99999);
		expect(missingOfRow('multisig:100:0xab')).toBeNull();
		seams.missing = [];
		expect(waitingRows.group(m)).toBeNull();
	});
});
