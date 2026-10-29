/**
 * ms-9: an executed cancellation is a row of its own in Activity — executed
 * here or on another member's device — titled as its proposal was, moving
 * nothing; a voided transaction (no `ExecutionSuccess`) writes nothing.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { LocalTransaction } from '$lib/services/transactions-model';

const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const CANCEL = `0x${'cc'.repeat(32)}`;
const VOIDED = `0x${'dd'.repeat(32)}`;
const TX = `0x${'ee'.repeat(32)}`;

const store = vi.hoisted(() => ({ saved: [] as LocalTransaction[] }));
vi.mock('$lib/services/records', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/services/records')>()),
	loadTransactions: async () => store.saved,
	saveTransactions: async (rows: LocalTransaction[]) => void store.saved.push(...rows)
}));
/** The chain: an `ExecutionSuccess` for the cancellation only. */
vi.mock('$lib/services/rpc-pool', () => ({
	poolRpcCall: async (method: string, params: unknown[]) => {
		if (method === 'eth_blockNumber') return { result: '0x100' };
		if (method === 'eth_getBlockByNumber') return { result: { timestamp: '0x65000000' } };
		const topics = (params[0] as { topics: string[] }).topics;
		return {
			result: topics[1] === CANCEL ? [{ transactionHash: TX, blockNumber: '0xff' }] : []
		};
	}
}));
vi.mock('./display', () => ({ tokenInfo: async () => ({ symbol: 'xDAI', decimals: 18 }) }));

const { recordExecution, recordRemoteExecutions, useCancelTitle } = await import('./records');

function item(safeTxHash: string, action: unknown, nonce = 3n) {
	return {
		chainId: 100,
		safeTxHash,
		review: { action, refusal: null },
		nonce,
		status: 'done'
	} as never;
}

// A loaded full run (both projects at once, `pnpm test:unit`) overran the
// default 5 s here: a first test pays for compiling its module graph (the
// real core, where one loads), not its own time (the house pattern,
// bridge.test.ts).
vi.setConfig({ testTimeout: 30_000 });

beforeEach(() => {
	store.saved = [];
});

describe('an executed cancellation in Activity (ms-9)', () => {
	it('executed here: one row titled as its proposal, from the multisig, moving nothing', async () => {
		useCancelTitle('Annuler la transaction n° {{nonce}}');
		await recordExecution(SAFE, item(CANCEL, { kind: 'cancel' }), TX, '0xop', 1_700_000_000);
		expect(store.saved).toHaveLength(1);
		expect(store.saved[0]).toMatchObject({
			id: `multisig:${CANCEL}:0`,
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
		expect(await recordRemoteExecutions(SAFE, items)).toBe(true);
		expect(store.saved.map((r) => r.id)).toEqual([`multisig:${CANCEL}:0`]);
		expect(store.saved[0].intent).toBe('Cancel transaction #3');
		// Asked again on the next refresh: already there, nothing written.
		expect(await recordRemoteExecutions(SAFE, items)).toBe(false);
		expect(store.saved).toHaveLength(1);
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
