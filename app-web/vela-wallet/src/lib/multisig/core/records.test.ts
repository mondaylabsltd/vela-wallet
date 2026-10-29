/**
 * A multisig's executed transactions in Activity (ms-9, r4a-3, r4a-4,
 * r4b-2): every kind writes a row, under the one id scheme, executed here or
 * found on chain — the history read in windows a capped node takes, and an
 * unreadable history said as that, never as "nothing happened".
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { LocalTransaction } from '$lib/services/transactions-model';

const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const CANCEL = `0x${'cc'.repeat(32)}`;
const VOIDED = `0x${'dd'.repeat(32)}`;
const CALL = `0x${'aa'.repeat(32)}`;
const TX = `0x${'ee'.repeat(32)}`;
const CALL_TX = `0x${'ef'.repeat(32)}`;

const store = vi.hoisted(() => ({ saved: [] as LocalTransaction[] }));
vi.mock('$lib/services/records', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/services/records')>()),
	loadTransactions: async () => store.saved,
	saveTransactions: async (rows: LocalTransaction[]) => {
		const ids = new Set(rows.map((r) => r.id));
		store.saved = [...rows, ...store.saved.filter((t) => !ids.has(t.id))];
	},
	mergeTransactions: async (rows: LocalTransaction[]) => {
		const ids = new Set(store.saved.map((t) => t.id));
		const fresh = rows.filter((r) => !ids.has(r.id));
		store.saved = [...fresh, ...store.saved];
		return fresh.length;
	},
	deleteTransactions: async (gone: string[]) => {
		store.saved = store.saved.filter((t) => !gone.includes(t.id));
	}
}));

/**
 * The chain: head 0x200000, a block every 2 s; an `ExecutionSuccess` for the
 * cancellation at block 0xff and for the call at 0x1f0000. The node takes no
 * window wider than `chain.cap` blocks ("requested range too large", Arc's
 * public node), and fails outright while `chain.down`.
 */
const chain = vi.hoisted(() => ({
	cap: Infinity,
	down: false,
	windows: [] as [bigint, bigint][],
	head: 0x200000n
}));
vi.mock('$lib/services/rpc-pool', async (importOriginal) => ({
	getLogsRangeCap: (await importOriginal<typeof import('$lib/services/rpc-pool-endpoints')>())
		.getLogsRangeCap,
	poolRpcCall: async (method: string, params: unknown[]) => {
		if (chain.down) throw new Error('every endpoint failed');
		if (method === 'eth_getBlockByNumber') {
			const tag = params[0] as string;
			const n = tag === 'latest' ? chain.head : BigInt(tag);
			return {
				result: { number: `0x${n.toString(16)}`, timestamp: `0x${(n * 2n).toString(16)}` }
			};
		}
		if (method === 'eth_call') return { result: '0x0' };
		const filter = params[0] as { topics: string[]; fromBlock: string; toBlock: string };
		const from = BigInt(filter.fromBlock);
		const to = BigInt(filter.toBlock);
		if (Number(to - from + 1n) > chain.cap) {
			return { error: { code: -32012, message: 'requested range too large' } };
		}
		chain.windows.push([from, to]);
		const at: Record<string, bigint> = { [CANCEL]: 0xffn, [CALL]: 0x1f0000n };
		const block = at[filter.topics[1]];
		return {
			result:
				block !== undefined && block >= from && block <= to
					? [
							{
								transactionHash: filter.topics[1] === CALL ? CALL_TX : TX,
								blockNumber: `0x${block.toString(16)}`
							}
						]
					: []
		};
	}
}));
vi.mock('./display', () => ({
	tokenInfo: async () => ({ symbol: 'xDAI', decimals: 18 }),
	actionTitle: async (_m: unknown, _c: number, action: { kind: string }, nonce: bigint) =>
		action.kind === 'cancel' ? `Cancel transaction #${nonce}` : `Title of ${action.kind}`
}));

const {
	executionRows,
	findExecution,
	HistoryUnreadableError,
	recordExecution,
	recordRemoteExecutions,
	resetHistoryForTests,
	searchLogs,
	useCancelTitle,
	useRecordWords
} = await import('./records');

function item(safeTxHash: string, action: unknown, nonce = 3n, createdMs = 0) {
	return {
		chainId: 100,
		safeTxHash,
		tx: { to: SAFE },
		review: { action, refusal: null },
		nonce,
		status: 'done',
		createdMs
	} as never;
}

// A loaded full run (both projects at once, `pnpm test:unit`) overran the
// default 5 s here: a first test pays for compiling its module graph (the
// real core, where one loads), not its own time (the house pattern,
// bridge.test.ts).
vi.setConfig({ testTimeout: 30_000 });

beforeEach(() => {
	store.saved = [];
	chain.cap = Infinity;
	chain.down = false;
	chain.windows = [];
	resetHistoryForTests();
});

describe('an executed cancellation in Activity (ms-9)', () => {
	it('executed here: one row titled as its proposal, from the multisig, moving nothing', async () => {
		useCancelTitle('Annuler la transaction n° {{nonce}}');
		await recordExecution(SAFE, item(CANCEL, { kind: 'cancel' }), TX, '0xop', 1_700_000_000);
		expect(store.saved).toHaveLength(1);
		expect(store.saved[0]).toMatchObject({
			id: `multisig:100:${CANCEL}:0`,
			txHash: TX,
			from: SAFE,
			to: SAFE,
			value: '0',
			type: 'dapp_tx',
			status: 'confirmed',
			intent: 'Annuler la transaction n° 3',
			dappSummary: { action: 'call', calls: 1, contract: SAFE.toLowerCase() }
		});
	});

	it('executed elsewhere: found on chain and written once; the voided one writes nothing', async () => {
		useCancelTitle('Cancel transaction #{{nonce}}');
		const items = [item(CANCEL, { kind: 'cancel' }), item(VOIDED, { kind: 'cancel' })];
		expect(await recordRemoteExecutions(SAFE, items)).toEqual({ wrote: true, unreadable: [] });
		expect(store.saved.map((r) => r.id)).toEqual([`multisig:100:${CANCEL}:0`]);
		expect(store.saved[0].intent).toBe('Cancel transaction #3');
		// Asked again on the next refresh: already there, nothing written —
		// and the voided one, final, is not searched again (r4D-18).
		chain.windows = [];
		expect(await recordRemoteExecutions(SAFE, items)).toEqual({ wrote: false, unreadable: [] });
		expect(store.saved).toHaveLength(1);
		expect(chain.windows).toEqual([]);
	});

	it('reads in the feed as its own row: the title, no figure', async () => {
		useCancelTitle('Cancel transaction #{{nonce}}');
		await recordExecution(SAFE, item(CANCEL, { kind: 'cancel' }), TX, '', 1_700_000_000);
		const { feedItemsThroughCore } = await import('$lib/wallet/core/feed-through-core');
		const { liveActivityRow } = await import('$lib/wallet/live');
		const { resolveWalletMessages } = await import('$lib/i18n/engine.server');
		const items = await feedItemsThroughCore(store.saved, SAFE, 1_700_000_100_000);
		expect(items).toHaveLength(1);
		const row = liveActivityRow(items[0], resolveWalletMessages('en'), false);
		expect(row.title).toBe('Cancel transaction #3');
		expect(row.amount).toBe('');
	});
});

describe('every executed kind writes a row (r4a-3)', () => {
	beforeEach(() => useRecordWords({ action: { cancel: 'Cancel #{{nonce}}' } } as never));

	it.each([
		[
			'a contract call',
			{ kind: 'call', to: '0x00000000000000000000000000000000000000c1', value: '0', data: '0x' }
		],
		[
			'a member change',
			{ kind: 'addOwner', owner: '0x00000000000000000000000000000000000000c2', threshold: '2' }
		],
		['a rename', { kind: 'rename', name: 'Team' }],
		['a threshold change', { kind: 'changeThreshold', threshold: '3' }]
	])('%s: one titled row, under the one id scheme', async (_what, action) => {
		const rows = await executionRows(SAFE, item(CALL, action), TX, '', 1);
		expect(rows).toHaveLength(1);
		expect(rows[0]).toMatchObject({
			id: `multisig:100:${CALL}:0`,
			type: 'dapp_tx',
			intent: `Title of ${action.kind}`,
			from: SAFE
		});
	});

	it('a batch of transfers and a call: a row per transfer, and one for the rest', async () => {
		const to = '0x00000000000000000000000000000000000000d1';
		const rows = await executionRows(
			SAFE,
			item(CALL, {
				kind: 'batch',
				calls: [
					{ action: { kind: 'transfer', token: null, to, amount: '1000000000000000000' } },
					{ action: { kind: 'call', to, value: '0', data: '0x12' } }
				]
			}),
			TX,
			'',
			1
		);
		expect(rows.map((r) => [r.id, r.type])).toEqual([
			[`multisig:100:${CALL}:0`, 'send'],
			[`multisig:100:${CALL}:1`, 'dapp_tx']
		]);
		expect(rows[0]).toMatchObject({ to, value: '1', symbol: 'xDAI' });
	});

	it('a call executed on another member’s device is found and written here', async () => {
		const call = item(
			CALL,
			{ kind: 'call', to: '0x00000000000000000000000000000000000000c1', value: '0', data: '0x' },
			4n
		);
		expect(await recordRemoteExecutions(SAFE, [call])).toEqual({ wrote: true, unreadable: [] });
		expect(store.saved[0]).toMatchObject({ id: `multisig:100:${CALL}:0`, txHash: CALL_TX });
	});

	it('a row written before the one scheme is renamed to it, once', async () => {
		store.saved = [
			{ id: `multisig:${CANCEL}:0`, chainId: 100, txHash: TX, timestamp: 1 } as LocalTransaction
		];
		await recordRemoteExecutions(SAFE, [item(CANCEL, { kind: 'cancel' })]);
		expect(store.saved.map((r) => r.id)).toEqual([`multisig:100:${CANCEL}:0`]);
	});
});

describe('the history, read in windows (r4b-2)', () => {
	it('a node capping the window: asked again with half, newest first, until found', async () => {
		chain.cap = 3_000;
		const log = await searchLogs(
			100,
			{ address: SAFE, topics: [null, CALL] },
			0x1e0000n,
			chain.head
		);
		expect(log?.transactionHash).toBe(CALL_TX);
		// 10 000 refused, 5 000 refused, 2 500 taken — and never wider again.
		expect(chain.windows.every(([a, b]) => b - a + 1n <= 2_500n)).toBe(true);
		expect(chain.windows[0][1]).toBe(chain.head);
	});

	it('bounded by when it was proposed: the windows stop there', async () => {
		// Proposed at block 0x1f0000's time (2 s a block); an hour's margin before.
		const since = Number(0x1f0000n * 2n) * 1000;
		expect(await findExecution(SAFE, 100, CALL, { sinceMs: since, final: true, nonce: 4n })).toBe(
			CALL_TX
		);
		const lowest = chain.windows.reduce((low, [a]) => (a < low ? a : low), chain.head);
		expect(lowest).toBeGreaterThan(0x1f0000n - 5_000n);
	});

	it('a node that is down is an unreadable history — said, never "none"', async () => {
		chain.down = true;
		await expect(
			searchLogs(100, { address: SAFE, topics: [null, CALL] }, 0n, chain.head)
		).rejects.toBeInstanceOf(HistoryUnreadableError);
		expect(await findExecution(SAFE, 100, CALL)).toBeUndefined();
		const items = [item(CALL, { kind: 'call', to: SAFE, value: '0', data: '0x' })];
		expect(await recordRemoteExecutions(SAFE, items)).toEqual({ wrote: false, unreadable: [100] });
		expect(store.saved).toEqual([]);
	});

	it('a node that refuses even one block: unreadable, not endless', async () => {
		chain.cap = 0;
		await expect(
			searchLogs(100, { address: SAFE, topics: [null, CALL] }, 0n, chain.head)
		).rejects.toBeInstanceOf(HistoryUnreadableError);
	});
});
