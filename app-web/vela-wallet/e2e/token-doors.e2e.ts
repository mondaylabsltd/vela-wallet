/**
 * The token's two doors, and the filter's shortcut (spec 028 Phase 9,
 * RULINGS 1 and 3), hermetically on the stubbed chain: one Ethereum holding
 * of 1.5 ETH at $3,000, no live traffic.
 *
 * - 转账 from a held token opens the form WITH that token — no picker.
 * - 收款 from a held token opens the token's own code — no network list.
 * - With a network chosen in the sidebar, 收款 opens that network's code.
 */
import { expect, test, type Page } from '@playwright/test';
import { CHAINS } from '../src/lib/services/chains';
import { en, seedSignedIn } from './live-helpers';
import {
	abiWord,
	aggregate3CallCount,
	denyOffOrigin,
	encodeAggregate3Result,
	seedNetworkOverrides,
	stubChainRegistry,
	stubJsonRpc
} from './stub-chain';

const STUB = 'https://stub-rpc.test/rpc';
const ONE_AND_A_HALF_ETH = 1_500_000_000_000_000_000n;
const THREE_THOUSAND_USD_8DP = 3000n * 100_000_000n;

test.beforeEach(async ({ page }) => {
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
		if (method === 'eth_call') {
			const call = params[0] as { data?: string } | undefined;
			const n = call?.data ? aggregate3CallCount(call.data) : 0;
			const balance = chainId === 1 ? ONE_AND_A_HALF_ETH : 0n;
			const data =
				'0x' +
				abiWord(balance) +
				abiWord(THREE_THOUSAND_USD_8DP) +
				abiWord(0) +
				abiWord(0) +
				abiWord(0);
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
	await expect(page.getByText('$4,500', { exact: true })).toBeVisible({ timeout: 25_000 });
}

test.describe('on the wide layout', () => {
	test.use({ viewport: { width: 1280, height: 900 } });

	test('转账 from the asset column opens the form with the token chosen', async ({ page }) => {
		await openHome(page);
		await page.getByText('ETH', { exact: true }).first().click();
		const column = page.getByRole('complementary').last();
		await column.getByRole('button', { name: en('tokenDetail.send') }).click();
		// The form, not the picker: the recipient field is on screen at once.
		await expect(page.getByRole('textbox', { name: en('send.recipientLabel') })).toBeVisible({
			timeout: 20_000
		});
		await expect(page.getByRole('heading', { name: en('send.selectTokenTitle') })).toHaveCount(0);
	});

	test('收款 from the asset column opens the token’s own code', async ({ page }) => {
		await openHome(page);
		await page.getByText('ETH', { exact: true }).first().click();
		const column = page.getByRole('complementary').last();
		await column.getByRole('button', { name: en('tokenDetail.receive') }).click();
		// The asset code: titled for the token on its chain, no list first.
		await expect(page.getByText(/ETH.*Ethereum|Ethereum.*ETH/).first()).toBeVisible({
			timeout: 20_000
		});
		await expect(page.getByPlaceholder(en('receive.searchNetworkPlaceholder'))).toHaveCount(0);
	});

	test('with a network chosen in the sidebar, 收款 goes straight to its code', async ({ page }) => {
		await openHome(page);
		await page
			.getByRole('button', { name: /^Ethereum/ })
			.first()
			.click();
		await page
			.getByRole('button', { name: en('componentsUi.dock.receive') })
			.first()
			.click();
		await expect(page.locator('svg[role="img"] path').first()).toBeVisible({ timeout: 20_000 });
		await expect(page.getByPlaceholder(en('receive.searchNetworkPlaceholder'))).toHaveCount(0);
	});
});

test('on the phone, the token sheet’s 转账 opens the form with the token chosen', async ({
	page
}) => {
	await page.setViewportSize({ width: 390, height: 844 });
	await openHome(page);
	await page.getByText('ETH', { exact: true }).first().click();
	const sheet = page.getByRole('dialog');
	await expect(sheet.getByText('1.5 ETH', { exact: true })).toBeVisible();
	await sheet.getByRole('button', { name: en('tokenDetail.send') }).click();
	await expect(page.getByRole('textbox', { name: en('send.recipientLabel') })).toBeVisible({
		timeout: 20_000
	});
});
