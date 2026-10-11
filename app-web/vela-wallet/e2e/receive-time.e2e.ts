/**
 * PR 3 — a receipt's time is its block's time, never "now".
 *
 * On the owner's iPhone three receipts of 0.001 xDAI, received on
 * 2026-09-29, stood under "Today" on 2026-10-10: the scan had not read their
 * block's time, they were stamped with the clock, and the store kept that
 * for good. The scan no longer does it. The records already stored that way
 * are repaired in the background: the feed core asks for each unmarked
 * receipt's block time (`read_receive_time`: the transaction's receipt, then
 * its block, through the RPC pool) and has the store rewritten
 * (`write_receive_time`).
 *
 * Here, on the real machines and the real store: two receipts stamped a few
 * minutes ago, whose block the chain says is eleven days old. While the
 * chain is silent they stand under Today, as stored; once it answers they
 * are under their own day — on the home and in the store — and the send
 * beside them, whose time IS the clock's, has not moved.
 */
import { expect, test, type Page } from '@playwright/test';
import { CHAINS } from '../src/lib/services/chains';
import type { LocalTransaction } from '../src/lib/services/transactions-model';
import { en, seedSignedIn, TEST_ACCOUNT_ADDRESS } from './live-helpers';
import {
	abiWord,
	aggregate3CallCount,
	denyOffOrigin,
	encodeAggregate3Result,
	readKv,
	seedNetworkOverrides,
	stubChainRegistry,
	stubJsonRpc
} from './stub-chain';

test.use({ viewport: { width: 390, height: 844 } });
test.setTimeout(90_000);

const STUB = 'https://stub-rpc.test/rpc';
const ONE_AND_A_HALF_ETH = 1_500_000_000_000_000_000n;
const PRICE_8DP = 3000n * 100_000_000n;
const ALICE = '0x9F3cA71b04E82f5C55d9B21aE00734F8Dd8021aE';
const BOB = '0x' + 'b0'.repeat(20);
/** The block both receipts are in. */
const BLOCK = '0x1092';
const ELEVEN_DAYS = 11 * 86_400;

const hash = (id: string) => '0x' + id.padStart(64, 'a');

/** Two receipts stamped a few minutes ago — the clock's time — and a send. */
function records(nowSec: number): LocalTransaction[] {
	const row = (id: string, at: number, type: 'send' | 'receive', value: string) => ({
		id,
		userOpHash: type === 'send' ? '0x' + id.padStart(64, '0') : '',
		txHash: hash(id),
		from: type === 'send' ? TEST_ACCOUNT_ADDRESS : ALICE,
		to: type === 'send' ? BOB : TEST_ACCOUNT_ADDRESS,
		value,
		symbol: 'ETH',
		decimals: 18,
		chainId: 1,
		timestamp: at,
		status: 'confirmed' as const,
		type
	});
	return [
		row('1', nowSec - 300, 'receive', '0.5'),
		row('2', nowSec - 600, 'send', '0.12'),
		row('3', nowSec - 900, 'receive', '0.25')
	];
}

const stored = async (page: Page): Promise<LocalTransaction[]> =>
	JSON.parse((await readKv(page, 'vela.transactionHistory')) ?? '[]') as LocalTransaction[];

test('receipts stamped with the clock are put under the day their block was mined — on the home and in the store', async ({
	page
}, testInfo) => {
	const nowSec = Math.floor(Date.now() / 1000);
	const blockTime = nowSec - ELEVEN_DAYS;
	const seeded = records(nowSec);

	await seedSignedIn(page);
	await denyOffOrigin(page);
	await stubChainRegistry(page, {
		1: {
			chainId: 1,
			name: 'Ethereum',
			nativeCurrency: { name: 'Ether', symbol: 'ETH', decimals: 18 },
			stables: [],
			wrappedNativeToken: null,
			dex: null,
			rpc: [`${STUB}/1`]
		}
	});
	await stubJsonRpc(page, /stub-rpc\.test\/rpc\/(\d+)/, (method, params, url) => {
		const chainId = Number(/\/rpc\/(\d+)/.exec(url)?.[1]);
		if (method === 'eth_chainId') return '0x' + chainId.toString(16);
		if (method === 'eth_blockNumber') return '0x2000';
		if (method === 'eth_getLogs') return [];
		if (method === 'eth_call') {
			const call = params[0] as { data?: string } | undefined;
			const n = call?.data ? aggregate3CallCount(call.data) : 0;
			const balance = chainId === 1 ? ONE_AND_A_HALF_ETH : 0n;
			const data =
				'0x' + abiWord(balance) + abiWord(PRICE_8DP) + abiWord(0) + abiWord(0) + abiWord(0);
			return encodeAggregate3Result(Array.from({ length: n }, () => ({ success: true, data })));
		}
		return undefined;
	});
	// Registered after the stub, so it runs first: what the chain says of the
	// receipts' transactions and of their block — held until the test has seen
	// the home as the store has it.
	const asked: { method: string; params: unknown[] }[] = [];
	let answer: () => void = () => {};
	const held = new Promise<void>((resolve) => (answer = resolve));
	await page.route(/stub-rpc\.test\/rpc\/1$/, async (route) => {
		const body = route.request().postDataJSON() as {
			method?: string;
			params?: unknown[];
			id?: number;
		} | null;
		const method = body?.method ?? '';
		const params = body?.params ?? [];
		const reply = (result: unknown) =>
			route.fulfill({
				contentType: 'application/json',
				body: JSON.stringify({ jsonrpc: '2.0', id: body?.id ?? 1, result })
			});
		if (method === 'eth_getTransactionReceipt') {
			asked.push({ method, params });
			await held;
			return reply({ transactionHash: params[0], blockNumber: BLOCK, status: '0x1', logs: [] });
		}
		if (method === 'eth_getBlockByNumber' && params[0] === BLOCK) {
			asked.push({ method, params });
			return reply({ number: BLOCK, timestamp: '0x' + blockTime.toString(16) });
		}
		return route.fallback();
	});

	await page.goto('/en/wallet');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await seedNetworkOverrides(
		page,
		CHAINS.map((c) => ({ chainId: c.chainId, rpcURL: `${STUB}/${c.chainId}` }))
	);
	await page.evaluate(
		(rows) =>
			new Promise<void>((resolve, reject) => {
				const open = indexedDB.open('vela', 1);
				open.onupgradeneeded = () => open.result.createObjectStore('kv');
				open.onerror = () => reject(open.error);
				open.onsuccess = () => {
					const tx = open.result.transaction('kv', 'readwrite');
					tx.objectStore('kv').put(JSON.stringify(rows), 'vela.transactionHistory');
					tx.oncomplete = () => resolve();
					tx.onerror = () => reject(tx.error);
				};
			}),
		seeded
	);
	await page.reload();
	await expect(page.getByText('$4,500', { exact: true })).toBeVisible({ timeout: 30_000 });

	// As stored: all three under Today, and the chain has been asked about the
	// two receipts — and only them.
	const days = page.locator('p.day');
	const today = en('componentsUi.dayGroup.today');
	await expect(page.getByText('+0.5', { exact: true })).toBeVisible();
	await expect(days).toHaveText([today]);
	await expect
		.poll(() => [...new Set(asked.map((read) => read.params[0]))].sort(), { timeout: 30_000 })
		.toEqual([hash('1'), hash('3')].sort());
	expect(asked.every((read) => read.method === 'eth_getTransactionReceipt')).toBe(true);
	await page.screenshot({ path: testInfo.outputPath('before.png') });

	// The chain answers: the block is eleven days old.
	answer();
	await expect
		.poll(
			async () =>
				(await stored(page)).map((tx) => [tx.id, tx.type, tx.timestamp, tx.timeVerified ?? null]),
			{ timeout: 30_000 }
		)
		.toEqual([
			['1', 'receive', blockTime, true],
			['2', 'send', nowSec - 600, null],
			['3', 'receive', blockTime, true]
		]);
	// Nothing else of any record moved: every other field as it was stored.
	const after = (await stored(page)) as unknown as Record<string, unknown>[];
	for (const [i, before] of (seeded as unknown as Record<string, unknown>[]).entries()) {
		for (const field of Object.keys(before).filter((key) => key !== 'timestamp')) {
			expect(JSON.stringify(after[i][field]), `${before.id}.${field}`).toBe(
				JSON.stringify(before[field])
			);
		}
		expect(Object.keys(after[i]).filter((key) => !(key in before))).toEqual(
			before.type === 'receive' ? ['timeVerified'] : []
		);
	}

	// And the home says so: the send under Today, the receipts under a day of
	// their own — a date, eleven days back, not "Today" and not "Yesterday".
	await expect(days).toHaveCount(2, { timeout: 30_000 });
	await expect(days.first()).toHaveText(today);
	const older = (await days.nth(1).textContent())?.trim() ?? '';
	expect(older).not.toBe('');
	expect(older).not.toBe(today);
	expect(older).not.toBe(en('componentsUi.dayGroup.yesterday'));
	// The rows under each: the send alone today, both receipts after the date.
	const order = await page.evaluate(() =>
		[...document.querySelectorAll('p.day, p.day ~ ul li')].map((el) =>
			el.matches('p.day') ? `# ${el.textContent?.trim()}` : (el.textContent ?? '')
		)
	);
	const at = order.indexOf(`# ${older}`);
	expect(at).toBeGreaterThan(0);
	expect(order.slice(1, at).some((row) => row.includes('0.12'))).toBe(true);
	expect(order.slice(1, at).some((row) => row.includes('+0.5') || row.includes('+0.25'))).toBe(
		false
	);
	expect(order.slice(at + 1).filter((row) => /\+0\.(5|25)/.test(row))).toHaveLength(2);
	// The chain was asked about the receipts' transactions and their block,
	// and never about the send's.
	expect(asked.some((read) => read.method === 'eth_getBlockByNumber')).toBe(true);
	expect(asked.some((read) => read.params[0] === hash('2'))).toBe(false);
	await page.screenshot({ path: testInfo.outputPath('after.png') });
});
