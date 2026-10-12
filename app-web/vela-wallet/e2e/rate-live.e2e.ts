/**
 * The chosen currency converts the total once a rate source is reachable
 * (spec 025 T143 — SC-104), hermetically.
 *
 * The home stub from home-truth (1.5 ETH at $3,000 = $4,500), plus the
 * configured fiat-rates endpoint stubbed to price ONE currency: VND at 25,000
 * per USD. VND has no Chainlink feed, so the waterfall's second source is the
 * one that answers — and the 024 rule is visible in the same run: before the
 * choice the total is the honest USD figure, after it the converted one.
 */
import { expect, test } from '@playwright/test';
import { CHAINS } from '../src/lib/services/chains';
import { en, seedSignedIn } from './live-helpers';
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

const STUB = 'https://stub-rpc.test/rpc';
const ETH = 1_000_000_000_000_000_000n;
const PRICE_8DP = 3000n * 100_000_000n;

test('a chosen currency converts the total with a reachable rate source (SC-104)', async ({
	page
}) => {
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
			const balance = chainId === 1 ? (ETH * 3n) / 2n : 0n;
			const data =
				'0x' + abiWord(balance) + abiWord(PRICE_8DP) + abiWord(0) + abiWord(0) + abiWord(0);
			return encodeAggregate3Result(Array.from({ length: n }, () => ({ success: true, data })));
		}
		if (method === 'eth_getLogs') return [];
		return undefined;
	});
	// The configured fiat-rates endpoint (Frankfurter v2 shape): only VND.
	await page.route(/vela-currency\.getvela\.app\/v2\/rates/, (route) =>
		route.fulfill({
			contentType: 'application/json',
			body: JSON.stringify([{ base: 'USD', quote: 'VND', rate: 25000 }])
		})
	);

	await page.goto('/en/wallet');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await seedNetworkOverrides(
		page,
		CHAINS.map((c) => ({ chainId: c.chainId, rpcURL: `${STUB}/${c.chainId}` }))
	);
	await page.reload();
	await expect(page.getByText('$4,500', { exact: true })).toBeVisible({ timeout: 20_000 });

	// Choose VND in Settings — the core persists the code, the shell fetches the rate.
	await page.goto('/en/settings');
	await page.getByText(en('settings.localization.currencyTitle'), { exact: true }).click();
	await page.getByText('VND', { exact: true }).click();
	// Persisted before the page is left: the core writes behind the choice,
	// and the navigation below, issued on the heels of the tap, beat the write
	// two runs running on an idle machine — the home then still read $4,500.
	await expect.poll(() => readKv(page, 'vela.displayCurrency')).toBe('VND');

	// Back on the home: 4,500 × 25,000, in the chosen currency's glyph.
	await page.goto('/en/wallet');
	await expect(page.getByText('₫112,500,000', { exact: true })).toBeVisible({ timeout: 20_000 });
	await expect(page.getByText('$4,500', { exact: true })).toHaveCount(0);
});
