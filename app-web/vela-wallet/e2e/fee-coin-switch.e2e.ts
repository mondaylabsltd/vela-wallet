/**
 * The fee coin the person chose is the fee coin the form shows — and the one
 * `Max` reserves for.
 *
 * Reported against the ETH form with USDC ticked in the fee sheet: the sheet
 * showed the tick and the stablecoin figure, while the row behind it went on
 * reading "0.000392 ETH" and `Max` went on subtracting that ETH from the
 * amount. The send machine only ever learned a fee from quotes IT asked for,
 * and a fee-coin pick on an incomplete form asks for none — so it kept the
 * previous one. The desktop and Android shells have mirrored the fee session
 * into it since they were wired; this shell never did.
 */
import { expect, test, type Page } from '@playwright/test';
import { CHAINS } from '../src/lib/services/chains';
import { en } from './live-helpers';
import {
	denyOffOrigin,
	happyRelay,
	seedNetworkOverrides,
	stubChainRegistry,
	stubJsonRpc,
	stubRelay
} from './stub-chain';
import {
	answerAggregate3,
	isAggregate3,
	MULTICALL_SEL as SEL,
	roundData,
	word
} from './stub-multicall';

test.use({ viewport: { width: 390, height: 844 } });
test.setTimeout(120_000);

const STUB = 'https://stub-rpc.test/rpc';
// The app asks the Cloudflare relay (`vela-relay-cf`, endpoints.ts since spec 060);
// the older host is kept so a stub written against either still intercepts.
const RELAY = /vela-relay(-cf)?\.getvela\.app/;
const USDC = '0x' + 'cc'.repeat(20);
const ONE_AND_A_HALF_ETH = 1_500_000_000_000_000_000n;
const HUNDRED_USDC = 100_000_000n;
const ETH_USD_8DP = 3000n * 100_000_000n;
const RECIPIENT = '0x' + 'ab'.repeat(20);
const USER_OP_HASH = '0x' + 'a1'.repeat(32);
const TX_HASH = '0x' + 'b2'.repeat(32);

/** Ethereum, 1.5 ETH and 100 USDC — two coins that can both pay a fee. */
async function stubChain(page: Page): Promise<void> {
	await denyOffOrigin(page);
	await stubChainRegistry(page, {
		1: {
			chainId: 1,
			name: 'Ethereum',
			nativeCurrency: { name: 'Ether', symbol: 'ETH', decimals: 18 },
			stables: [{ symbol: 'USDC', type: 'native', contract: USDC }],
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
		if (method === 'eth_getLogs') return [];
		if (method === 'eth_call') {
			const call = params[0] as { data?: string } | undefined;
			if (!call?.data || !isAggregate3(call.data)) return '0x' + word(0);
			if (chainId !== 1) return answerAggregate3(call.data, () => undefined);
			return answerAggregate3(call.data, (inner) => {
				switch (inner.selector) {
					case SEL.getEthBalance:
						return { success: true, data: '0x' + word(ONE_AND_A_HALF_ETH) };
					case SEL.balanceOf:
						return inner.target === USDC
							? { success: true, data: '0x' + word(HUNDRED_USDC) }
							: { success: true, data: '0x' + word(0) };
					case SEL.decimals:
						return { success: true, data: '0x' + word(6) };
					case SEL.latestRoundData:
						return { success: true, data: roundData(ETH_USD_8DP) };
					default:
						return undefined;
				}
			});
		}
		return undefined;
	});
}

test('picking a fee coin changes the fee the form shows, and what Max reserves', async ({
	page
}) => {
	await stubChain(page);
	const relay = happyRelay(USER_OP_HASH, TX_HASH, () => 'pending');
	await stubRelay(page, RELAY, (method, params) => {
		if (method === 'vela_getInBandGasQuote') {
			// Two rows: the native coin, and a stablecoin the account holds.
			return [
				{
					recipient: '0x' + 'fe'.repeat(20),
					asset: 'native',
					feeToken: null,
					balance: '0x14d1120d7b160000',
					decimals: 18,
					symbol: 'ETH',
					usdBalance: '4500',
					usdPrice: '3000'
				},
				{
					recipient: '0x' + 'fe'.repeat(20),
					asset: 'erc20',
					feeToken: USDC,
					balance: '0x' + HUNDRED_USDC.toString(16),
					decimals: 6,
					symbol: 'USDC',
					usdBalance: '100',
					usdPrice: '1'
				}
			];
		}
		return relay(method, params);
	});

	await page.goto('/en/parallel');
	// The button is in the server's HTML before the page hydrates; pressed then,
	// it does nothing and the URL wait runs out the whole test (batch.e2e.ts).
	// The fixture list fills on mount — a row in it means the handlers are on.
	await page.locator('li code').first().waitFor({ timeout: 60_000 });
	await page.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
	await page.waitForURL(/\/en\/wallet$/);
	await seedNetworkOverrides(
		page,
		CHAINS.map((c) => ({ chainId: c.chainId, rpcURL: `${STUB}/${c.chainId}` }))
	);
	await page.reload();
	await expect(page.getByText('$4,600', { exact: true })).toBeVisible({ timeout: 25_000 });

	await page
		.getByRole('button', { name: en('componentsUi.dock.send') })
		.first()
		.click();
	await expect(page.getByRole('heading', { name: en('send.selectTokenTitle') })).toBeVisible();
	await page.getByText('ETH', { exact: true }).first().click();
	await page.getByRole('textbox', { name: en('send.recipientLabel') }).fill(RECIPIENT);
	await page.getByRole('textbox', { name: 'ETH' }).fill('0.1');

	// The native coin pays by default, and the row says so.
	const feeRow = page.getByRole('button', { name: en('send.feeTokenLabel') });
	await expect(feeRow).toContainText('ETH', { timeout: 30_000 });

	// Pick the stablecoin.
	await feeRow.click();
	await page.getByRole('button', { name: /USDC/ }).first().click();

	// The row behind the sheet follows the pick — no round trip needed, and no
	// waiting for a form the person may not have finished.
	await expect(feeRow).toContainText('USDC', { timeout: 15_000 });
	await expect(feeRow).not.toContainText('ETH');

	// …and Max now offers the WHOLE balance: the gas is not coming out of it.
	await page.getByRole('button', { name: en('send.maxBtn') }).click();
	await expect(page.getByRole('textbox', { name: 'ETH' })).toHaveValue('1.5', { timeout: 30_000 });
});
