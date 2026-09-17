/**
 * What the network fee COSTS, not just how much of a coin it takes (issue 201).
 *
 * The report: the fee row read "0.000091 BNB" and nothing else, while the
 * amount above it carried its own "≈ $…". A person who does not track the
 * coin's price could read the row and still not know what the transfer cost.
 * The drawn screens have had the fiat half since they were drawn
 * ("0.0021 ETH · ≈$0.55"); only the live overlay dropped it.
 *
 * Two prices reach the row, and both are exercised here: the relay's own
 * published quote row, and — when the relay publishes none — the balances feed
 * the amount's own "≈" line already reads.
 */
import { expect, test, type Page } from '@playwright/test';
import { CHAINS } from '../src/lib/services/chains';
import { en } from './live-helpers';
import {
	abiWord,
	aggregate3CallCount,
	denyOffOrigin,
	encodeAggregate3Result,
	happyRelay,
	seedNetworkOverrides,
	stubChainRegistry,
	stubJsonRpc,
	stubRelay,
	type RelayHandler
} from './stub-chain';

test.use({ viewport: { width: 390, height: 844 } });
test.setTimeout(120_000);

const STUB = 'https://stub-rpc.test/rpc';
const RELAY = /vela-relay\.getvela\.app/;
const ONE_AND_A_HALF_ETH = 1_500_000_000_000_000_000n;
const PRICE_8DP = 3000n * 100_000_000n;
const RECIPIENT = '0x' + 'ab'.repeat(20);

/** Ethereum, 1.5 ETH, priced at $3,000 by the chain's own feed. */
async function stubChain(page: Page): Promise<void> {
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
		if (method === 'eth_getCode') return '0x6080';
		if (method === 'eth_getTransactionCount') return '0x0';
		if (method === 'eth_gasPrice') return '0x3b9aca00';
		if (method === 'eth_maxPriorityFeePerGas') return '0x3b9aca00';
		if (method === 'eth_getBlockByNumber') return { baseFeePerGas: '0x3b9aca00' };
		if (method === 'eth_call') {
			const call = params[0] as { data?: string } | undefined;
			const n = call?.data ? aggregate3CallCount(call.data) : 0;
			const balance = chainId === 1 ? ONE_AND_A_HALF_ETH : 0n;
			const data =
				'0x' + abiWord(balance) + abiWord(PRICE_8DP) + abiWord(0) + abiWord(0) + abiWord(0);
			if (n === 0) return '0x' + abiWord(0);
			return encodeAggregate3Result(Array.from({ length: n }, () => ({ success: true, data })));
		}
		if (method === 'eth_getLogs') return [];
		return undefined;
	});
}

/** Into the send form with a recipient and an amount — the quoted state. */
async function composeSend(page: Page, relay: RelayHandler): Promise<void> {
	await stubChain(page);
	await stubRelay(page, RELAY, relay);

	await page.goto('/en/parallel');
	await page.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
	await page.waitForURL(/\/en\/wallet$/);
	await seedNetworkOverrides(
		page,
		CHAINS.map((c) => ({ chainId: c.chainId, rpcURL: `${STUB}/${c.chainId}` }))
	);
	await page.reload();

	await page
		.getByRole('button', { name: en('componentsUi.dock.send') })
		.first()
		.click();
	await expect(page.getByRole('heading', { name: en('send.selectTokenTitle') })).toBeVisible();
	await page.getByText('ETH', { exact: true }).first().click();
	await page.getByRole('textbox', { name: en('send.recipientLabel') }).fill(RECIPIENT);
	await page.getByRole('textbox', { name: 'ETH' }).fill('0.1');
}

test('the fee row says what the fee costs, in money as well as in the coin', async ({ page }) => {
	await composeSend(page, happyRelay('0x' + 'a1'.repeat(32), '0x' + 'b2'.repeat(32)));

	// The coin amount is the quote's; the money beside it is that coin's price.
	const feeRow = page.getByRole('button', { name: en('send.feeTokenLabel') });
	await expect(feeRow).toContainText(/[\d.]+ ETH · ≈\$[\d,]+\.\d\d/, { timeout: 30_000 });

	// …and the screen that is actually signed from says the same thing.
	await page.getByRole('button', { name: en('send.continueBtn') }).click();
	await expect(page.getByText(en('send.estFeeLabel'))).toBeVisible({ timeout: 30_000 });
	await expect(page.getByText(/[\d.]+ ETH · ≈\$[\d,]+\.\d\d/)).toBeVisible();
});

test('an unpriced relay row falls back to the price the amount line uses', async ({ page }) => {
	const happy = happyRelay('0x' + 'a1'.repeat(32), '0x' + 'b2'.repeat(32));
	await composeSend(page, (method, params) => {
		if (method !== 'vela_getInBandGasQuote') return happy(method, params);
		// The same row the relay publishes, with no price on it.
		return [
			{
				recipient: '0x' + 'fe'.repeat(20),
				asset: 'native',
				feeToken: null,
				balance: '0x14d1120d7b160000',
				decimals: 18,
				symbol: 'ETH',
				usdBalance: '4500',
				usdPrice: null
			}
		];
	});

	const feeRow = page.getByRole('button', { name: en('send.feeTokenLabel') });
	await expect(feeRow).toContainText(/[\d.]+ ETH · ≈\$[\d,]+\.\d\d/, { timeout: 30_000 });
});
