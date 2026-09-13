/**
 * Deleting a record from the activity detail (spec 028 Phase 8).
 *
 * Expo's home had swipe-to-delete with a tombstone; the web's detail sheet had
 * no way to say it. The delete goes through the same core (`activity_feed`'s
 * `DeleteRequested`): the row leaves at once, the record is removed from the
 * store, and a reload does not bring it back.
 */
import { expect, test, type Page } from '@playwright/test';
import { CHAINS } from '../src/lib/services/chains';
import { en } from './live-helpers';
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
/** The address the core DERIVES from this key (spec 019 invariant ②): a record
 *  is the account's only when its `from` is that address, so the key and the
 *  address must agree — these are relay-faults' pair. */
const SAFE = '0xD400866e00B055B20752a826CD5C89b811de130b';
const FIXTURE_PUBLIC_KEY =
	'04197db9030a1e166bec2cee05e0ddb94b26ee0b6d6f429f1748cda4eedac36f04fe546861a9c9dfaf75719b53c75e0b933d4aad6d325f18c75776a260d507647b';
const TX_HASH = '0x' + 'f6'.repeat(32);
const RECORD = {
	id: TX_HASH,
	userOpHash: '0x' + 'e5'.repeat(32),
	txHash: TX_HASH,
	from: SAFE,
	to: '0x' + 'ab'.repeat(20),
	value: '0.5',
	symbol: 'ETH',
	decimals: 18,
	chainId: 1,
	timestamp: Math.floor(Date.now() / 1000) - 60,
	status: 'confirmed',
	type: 'send'
};

async function seedWalletWithRecord(page: Page): Promise<void> {
	await page.addInitScript(
		([record, safe, key]) => {
			localStorage.setItem('vela.intro.seen', String(Date.now()));
			if (localStorage.getItem('vela.accounts') === null) {
				localStorage.setItem(
					'vela.accounts',
					JSON.stringify([
						{
							id: 'e2e-credential-id',
							name: 'E2E Wallet',
							address: safe,
							public_key_hex: key,
							created_at_iso: '2026-01-01T00:00:00.000Z',
							keys: []
						}
					])
				);
				localStorage.setItem('vela.activeAccountIndex', '0');
				const open = indexedDB.open('vela', 1);
				open.onupgradeneeded = () => open.result.createObjectStore('kv');
				open.onsuccess = () => {
					open.result
						.transaction('kv', 'readwrite')
						.objectStore('kv')
						.put(JSON.stringify([record]), 'vela.transactionHistory');
				};
			}
		},
		[RECORD, SAFE, FIXTURE_PUBLIC_KEY] as const
	);
}

test.beforeEach(async ({ page }) => {
	await seedWalletWithRecord(page);
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
		if (method === 'eth_blockNumber') return '0x10';
		if (method === 'eth_getLogs') return [];
		if (method === 'eth_call') {
			// Every chain answers zero holdings: the home is quiet and the one
			// activity row is the seeded record.
			const call = params[0] as { data?: string } | undefined;
			const n = call?.data ? aggregate3CallCount(call.data) : 0;
			const data = '0x' + abiWord(0) + abiWord(0) + abiWord(0) + abiWord(0) + abiWord(0);
			return encodeAggregate3Result(Array.from({ length: n }, () => ({ success: true, data })));
		}
		return undefined;
	});
});

async function openHome(page: Page): Promise<void> {
	await page.goto('/en/wallet');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await seedNetworkOverrides(
		page,
		CHAINS.map((c) => ({ chainId: c.chainId, rpcURL: `${STUB}/${c.chainId}` }))
	);
	await page.reload();
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
}

test('the detail’s delete removes the row, the record, and stays removed', async ({ page }) => {
	await openHome(page);

	// The seeded send is the one activity row: "Sent", −0.5 ETH.
	const row = page.getByRole('button', { name: new RegExp(en('history.labelSent')) }).first();
	await expect(row).toBeVisible({ timeout: 20_000 });
	await expect(page.getByText('-0.5', { exact: true })).toBeVisible();
	await row.click();

	const sheet = page.getByRole('dialog');
	await expect(sheet.getByText(en('history.deleteRecord'))).toBeVisible();
	await sheet.getByRole('button', { name: en('history.deleteRecord') }).click();

	// Gone from the list at once (the optimistic remove + tombstone)…
	await expect(page.getByText('-0.5', { exact: true })).toHaveCount(0);
	// …and from the store, so a reload does not repaint it.
	await expect
		.poll(async () => (await readKv(page, 'vela.transactionHistory')) ?? '[]')
		.not.toContain(TX_HASH);
	await page.reload();
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await expect(page.getByText(en('home.emptyNoActivity'))).toBeVisible({ timeout: 20_000 });
	await expect(page.getByText('-0.5', { exact: true })).toHaveCount(0);
});
