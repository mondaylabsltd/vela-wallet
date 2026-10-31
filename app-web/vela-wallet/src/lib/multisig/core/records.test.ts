/**
 * An executed proposal's Activity rows are the core's plan (`execution_rows`:
 * app_multisig_presence.rs `an_execution_writes_one_row_per_transfer_else_one_for_its_proposal`);
 * here they are only worded: a token's symbol and decimals, the proposal's title.
 */
import { describe, expect, it, vi } from 'vitest';

vi.mock('./display', () => ({
	actionTitle: async () => 'Change approvals to 1',
	shortenAddress: (a: string) => a.slice(0, 8),
	UNREAD_TOKEN: { symbol: '', decimals: null },
	tokenInfo: async (_chain: number, token: string | null) =>
		token ? { symbol: 'USDC', decimals: 6 } : { symbol: 'xDAI', decimals: 18 }
}));
const stored = vi.hoisted(() => ({ rows: [] as { id: string; intent?: string }[] }));
vi.mock('$lib/services/records', () => ({
	loadTransactions: async () => stored.rows,
	updateTransaction: async (id: string, patch: { intent?: string }) => {
		const row = stored.rows.find((r) => r.id === id);
		if (row) Object.assign(row, patch);
	}
}));

const { executionRows, rewordCancelRows, useRecordWords } = await import('./records');

describe('r8-22: a cancellation row titled in English before the words came', () => {
	it("reads in the page's words, and nothing else is touched", async () => {
		stored.rows = [
			{ id: 'multisig:100:0xab:0', intent: 'Cancel transaction #4' },
			{ id: 'multisig:100:0xcd:0', intent: 'Send 1 USDC' },
			{ id: '0xother', intent: 'Cancel transaction #9' }
		];
		expect(await rewordCancelRows('作废第 {{nonce}} 号交易')).toBe(1);
		expect(stored.rows.map((r) => r.intent)).toEqual([
			'作废第 4 号交易',
			'Send 1 USDC',
			'Cancel transaction #9'
		]);
	});
});

const SAFE = '0x' + 'cd'.repeat(20);
const BOB = '0x' + 'b0'.repeat(20);
const executed = (rows: unknown[], action: unknown = null) =>
	({
		from: SAFE,
		chainId: 100,
		safeTxHash: '0xab',
		nonce: '7',
		action,
		txHash: '0xt',
		userOpHash: '0xu',
		timestamp: 1700000000,
		rows
	}) as never;

describe('an executed proposal in Activity', () => {
	it('words each send the core planned in its token', async () => {
		const rows = await executionRows(
			executed([
				{ index: 0, kind: 'send', to: BOB, token: '0x' + 'aa'.repeat(20), amount: '1500000' },
				{ index: 1, kind: 'send', to: BOB, token: null, amount: '1000000000000000000' }
			])
		);
		expect(rows.map((r) => [r.id, r.type, r.value, r.symbol])).toEqual([
			['multisig:100:0xab:0', 'send', '1.5', 'USDC'],
			['multisig:100:0xab:1', 'send', '1', 'xDAI']
		]);
	});

	it('words a proposal row by its title, moving only what a call sends', async () => {
		useRecordWords({ action: { cancel: '' } } as never);
		const [row] = await executionRows(
			executed([{ index: 0, kind: 'call', to: SAFE, sends: '0', calls: 1, contract: SAFE }], {
				kind: 'changeThreshold',
				threshold: '1'
			})
		);
		expect(row).toMatchObject({
			type: 'dapp_tx',
			value: '0',
			intent: 'Change approvals to 1',
			dappSummary: { action: 'call', calls: 1, contract: SAFE }
		});
	});
});
