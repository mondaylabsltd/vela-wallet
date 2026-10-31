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
	canApprove: [],
	canExecute: [],
	sameNonce: [] as string[],
	executing: false
});
const seams = vi.hoisted(() => ({
	open: [] as unknown[],
	missing: [] as { chainId: number; count: number }[],
	stale: false,
	unreachable: false,
	offline: false,
	unreadable: [] as number[],
	historyUnreadable: [] as number[],
	cached: null as { at: number } | null,
	loading: false,
	readAt: null as number | null,
	/** The machine's note on its list, and each row's standing (`view`). */
	note: null as unknown,
	standings: {} as Record<string, unknown>
}));

vi.mock('./proposals.svelte', () => ({
	proposals: {
		get view() {
			return { waiting_note: seams.note, standings: seams.standings };
		},
		chain: () => undefined,
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
		},
		get offline() {
			return seams.offline;
		},
		get unreadable() {
			return seams.unreadable;
		},
		get historyUnreadable() {
			return seams.historyUnreadable;
		},
		get cached() {
			return seams.cached;
		},
		get loading() {
			return seams.loading;
		},
		get readAt() {
			return seams.readAt;
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

const { MULTISIG_RETRY, missingOfRow, proposalOfRow, waitingRows } =
	await import('./activity.svelte');

const m = {
	action: { call: 'Contract call' },
	status: {
		queued: 'Queued',
		ready: 'Ready',
		yourTurn: 'Your turn',
		waiting: 'Waiting',
		progress: '{{approved}}/{{threshold}}',
		executing: 'Executing',
		conflict: 'Only one can run'
	},
	proposal: {
		missingNetworkNamed: 'On {{name}} (chain {{chainId}}) — not here',
		missingNetwork: 'On chain {{chainId}} — not here',
		missingCount: 'Waiting there: {{n}}',
		addNetwork: 'Add network'
	},
	panel: {
		pending: 'Waiting',
		stale: 'stale',
		lastKnown: 'last known {{time}}',
		checking: 'checking',
		historyUnreadable: 'history unread on {{network}}'
	},
	service: {
		unreachableNote: 'service away',
		retry: 'Retry',
		offline: 'offline',
		offlineSince: 'offline since {{time}}',
		chainUnreachable: "can't reach {{network}}"
	}
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
		seams.note = { kind: 'unreachable' };
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
		seams.note = { kind: 'stale' };
		const group = waitingRows.group(m)!;
		expect(group.note).toBe('stale');
		expect(group.action).toBeUndefined();
		seams.note = null;
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

/**
 * What the group says of its list is the machine's note (its order:
 * app_multisig_presence.rs `the_waiting_group_says_where_its_list_stands`),
 * worded here, with a Retry where one helps.
 */
describe('what the waiting group says of where it stands (r4b-3, r4b-11, r4b-15, r4b-2)', () => {
	const reset = () => {
		seams.open = [call('0xdeadbeef')];
		seams.missing = [];
		seams.note = null;
	};

	it('offline is said as offline, with when the list was read', () => {
		reset();
		seams.note = { kind: 'offline', since: null };
		expect(waitingRows.group(m)?.note).toBe('offline');
		seams.note = { kind: 'offline', since: Date.UTC(2026, 9, 29, 16, 36) };
		expect(waitingRows.group(m)?.note).toMatch(/^offline since \S/);
		expect(waitingRows.group(m)?.action).toBeUndefined();
	});

	it('a network that did not answer is named, with Retry — never "nothing waits"', () => {
		reset();
		seams.note = { kind: 'chainUnreachable', chain_ids: [100] };
		const group = waitingRows.group(m)!;
		expect(group.note).toBe("can't reach Gnosis");
		expect(group.action).toEqual({ id: MULTISIG_RETRY, label: 'Retry' });
		seams.open = [];
		expect(waitingRows.group(m)?.note).toBe("can't reach Gnosis");
	});

	it('the list kept from an earlier page is shown at once, said as the last known', () => {
		reset();
		seams.note = { kind: 'lastKnown', at: Date.UTC(2026, 9, 29, 16, 36) };
		const group = waitingRows.group(m)!;
		expect(group.rows).toHaveLength(1);
		expect(group.note).toMatch(/^last known \S/);
	});

	it('still reading with nothing to show: a cue from the first frame', () => {
		reset();
		seams.open = [];
		seams.note = { kind: 'checking' };
		expect(waitingRows.group(m)).toMatchObject({ note: 'checking', rows: [] });
		seams.note = null;
		expect(waitingRows.group(m)).toBeNull();
	});

	it('a history that could not be read is said, with Retry', () => {
		reset();
		seams.note = { kind: 'historyUnreadable', chain_ids: [100] };
		expect(waitingRows.group(m)).toMatchObject({
			note: 'history unread on Gnosis',
			action: { id: MULTISIG_RETRY }
		});
	});
});

describe('a waiting row (r4b-6, r4b-7, r4b-17, r4a-4)', () => {
	it('never more approved than needed; proposals at one number say only one can run', async () => {
		seams.offline = false;
		seams.unreadable = [];
		seams.historyUnreadable = [];
		seams.cached = null;
		seams.stale = false;
		const a = {
			...call('0x11111111'),
			approved: 3,
			threshold: 2,
			status: 'ready',
			sameNonce: ['0xb']
		};
		const b = { ...call('0x22222222'), sameNonce: ['0xa'], executing: true };
		seams.open = [a, b];
		seams.note = null;
		seams.standings = {
			[`100:${a.safeTxHash}`]: { kind: 'ready' },
			[`100:${b.safeTxHash}`]: { kind: 'executing' }
		};
		waitingRows.follow(m, signing, MULTISIG);
		await settle();
		const [first, second] = waitingRows.group(m)!.rows;
		// r4D-13: one word on where it stands — a ready or executing row says no
		// count at all, so none can read "3/2" (a plain wait's goes through
		// progressLine, capped).
		expect(first.subtitle).toBe('Ready · Only one can run · Gnosis');
		expect(second.subtitle).toBe('Executing · Only one can run · Gnosis');
	});

	it('its id opens the transaction — an executed row’s too', () => {
		const hash = `0x${'ab'.repeat(32)}`;
		expect(proposalOfRow(`multisig:100:${hash}`)).toEqual({ chainId: 100, safeTxHash: hash });
		expect(proposalOfRow(`multisig:100:${hash}:0`)).toEqual({ chainId: 100, safeTxHash: hash });
		expect(proposalOfRow(`multisig:${hash}:0`)).toBeNull();
	});
});
