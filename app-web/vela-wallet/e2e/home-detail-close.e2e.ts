/**
 * A row tapped on the home closes back onto the home (PR 3 note 16).
 *
 * On the phone layout a transaction tapped in the home's Activity opened its
 * detail over History — the drawn state puts that sheet over the list it also
 * lives on — and closing it left the person IN History, a screen they never
 * opened. The same with a holding tapped in the home's Assets: its page closed
 * onto the Assets list.
 *
 * A sheet a home row opens now stands over the home, which stays mounted
 * under it, and every way of closing it — a tap outside, Escape, the
 * browser's Back — is back where it was opened. The list's own rows are
 * unchanged: opened from History, a detail still closes onto History.
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
	seedNetworkOverrides,
	stubChainRegistry,
	stubJsonRpc
} from './stub-chain';

test.use({ viewport: { width: 390, height: 844 } });

const STUB = 'https://stub-rpc.test/rpc';
const ONE_AND_A_HALF_ETH = 1_500_000_000_000_000_000n;
const PRICE_8DP = 3000n * 100_000_000n;
const ALICE = '0x9F3cA71b04E82f5C55d9B21aE00734F8Dd8021aE';
const BOB = '0x' + 'b0'.repeat(20);

/** Five transfers over two days, newest first — more than the home's three. */
function records(nowSec: number): LocalTransaction[] {
	const row = (id: string, at: number, type: 'send' | 'receive', value: string) => ({
		id,
		userOpHash: '0x' + id.padStart(64, '0'),
		txHash: '0x' + id.padStart(64, 'a'),
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
		row('1', nowSec - 600, 'receive', '0.5'),
		row('2', nowSec - 1800, 'send', '0.12'),
		row('3', nowSec - 3600, 'receive', '0.25'),
		row('4', nowSec - 86_400, 'send', '0.3'),
		row('5', nowSec - 87_000, 'receive', '1')
	];
}

async function openHome(page: Page): Promise<void> {
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
		if (method === 'eth_blockNumber') return '0x10';
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
	await page.goto('/en/wallet');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await seedNetworkOverrides(
		page,
		CHAINS.map((c) => ({ chainId: c.chainId, rpcURL: `${STUB}/${c.chainId}` }))
	);
	// The account's history, as the store keeps it.
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
		records(Math.floor(Date.now() / 1000))
	);
	await page.reload();
	await expect(page.getByText('$4,500', { exact: true })).toBeVisible({ timeout: 30_000 });
	await expect(page.getByText('+0.5', { exact: true })).toBeVisible();
}

/** The home is the screen showing: its total and its three doors, and no History heading. */
async function expectHome(page: Page): Promise<void> {
	await expect(page.getByRole('dialog')).toHaveCount(0);
	await expect(page.getByText('$4,500', { exact: true })).toBeVisible();
	await expect(
		page.getByRole('button', { name: en('componentsUi.dock.receive'), exact: true })
	).toBeVisible();
	await expect(page.getByRole('heading', { name: en('history.navTitle'), level: 1 })).toHaveCount(
		0
	);
}

const history = (page: Page) =>
	page.getByRole('heading', { name: en('history.navTitle'), level: 1 });
const firstHomeRow = (page: Page) => page.getByText('+0.5', { exact: true }).first();

test('a transaction tapped on the home opens over the home, and every close is back on the home', async ({
	page
}, testInfo) => {
	await openHome(page);

	// The sheet has no ✕ of its own: it closes by a tap outside it, by
	// Escape, by a drag, and by the browser's Back.
	const closes: [string, (page: Page) => Promise<void>][] = [
		['a tap outside', async (p) => p.locator('.scrim').click({ position: { x: 24, y: 24 } })],
		['Escape', async (p) => p.keyboard.press('Escape')],
		['the browser’s Back', async (p) => void (await p.goBack())]
	];
	for (const [how, close] of closes) {
		await firstHomeRow(page).click();
		const sheet = page.getByRole('dialog');
		await expect(sheet).toBeVisible();
		await expect(sheet.getByText(/0\.5/).first()).toBeVisible();
		// Over the home it was opened from — the home is still drawn under it,
		// and History is not.
		await expect(page.getByText('$4,500', { exact: true })).toBeVisible();
		await expect(history(page), how).toHaveCount(0);
		if (how === 'a tap outside')
			await page.screenshot({ path: testInfo.outputPath('note16-open.png') });

		await close(page);
		await expectHome(page);
	}
	await page.screenshot({ path: testInfo.outputPath('note16-closed.png') });

	// Closed, the same row opens again (the step was taken off the stack).
	await firstHomeRow(page).click();
	await expect(page.getByRole('dialog')).toBeVisible();
	await page.keyboard.press('Escape');
	await expectHome(page);
});

test('the home is where it was: a row tapped after scrolling closes back to the same place', async ({
	page
}) => {
	await openHome(page);
	// The home scrolls inside its own frame: bring the holding into view first.
	const asset = page.getByRole('button', { name: /^ETH Ethereum/ }).first();
	await asset.scrollIntoViewIfNeeded();
	const row = page.getByText('+0.25', { exact: true }).first();
	const before = await row.boundingBox();
	await row.click();
	await expect(page.getByRole('dialog')).toBeVisible();
	await page.keyboard.press('Escape');
	await expectHome(page);
	// Not remounted, not scrolled back to the top: the row is where it was.
	expect(await row.boundingBox()).toEqual(before);
});

test('a holding tapped on the home closes back onto the home too', async ({ page }) => {
	await openHome(page);
	await page
		.getByRole('button', { name: /^ETH Ethereum/ })
		.first()
		.click();
	const sheet = page.getByRole('dialog');
	await expect(sheet.getByText(/^1 ETH = /)).toBeVisible();
	// Over the home, not over the Assets list.
	await expect(page.getByText('$4,500', { exact: true })).toBeVisible();
	await page.keyboard.press('Escape');
	await expectHome(page);
});

test('History’s own rows are unchanged: a detail opened there closes onto History', async ({
	page
}) => {
	await openHome(page);
	await page
		.getByRole('button', { name: en('history.filterAll'), exact: true })
		.first()
		.click();
	await expect(history(page)).toBeVisible();
	// The fifth transfer is in History and not in the home's three.
	await page.getByText('+1', { exact: true }).first().click();
	await expect(page.getByRole('dialog')).toBeVisible();
	await page.keyboard.press('Escape');
	await expect(page.getByRole('dialog')).toHaveCount(0);
	await expect(history(page)).toBeVisible();
});
