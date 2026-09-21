/**
 * Max on a balance the network fee outruns (issue 210), hermetically.
 *
 * The report: 0.00005 BNB in the account, a 0.000332 BNB fee, and "Max" fills
 * `0` with nothing beside it — leaving the person to guess between "the fee is
 * bigger than everything I have" and "the button is broken". The zero is
 * right; the silence was the defect.
 *
 * The same shape here on Ethereum, because that is the chain this suite's
 * stubs already speak: dust in the account, a quote that costs more than the
 * dust, and the core's own sentence on screen — on 211's live-warning line,
 * which is where every `amount_warning` reaches this shell.
 */
import { expect, test } from '@playwright/test';
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
	stubRelay
} from './stub-chain';

test.use({ viewport: { width: 390, height: 844 } });
test.setTimeout(120_000);

const STUB = 'https://stub-rpc.test/rpc';
// The app asks the Cloudflare relay (`vela-relay-cf`, endpoints.ts since spec 060);
// the older host is kept so a stub written against either still intercepts.
const RELAY = /vela-relay(-cf)?\.getvela\.app/;
/** 0.00005 ETH — the reporter's dust, in the asset this suite stubs. */
const DUST = 50_000_000_000_000n;
const PRICE_8DP = 3000n * 100_000_000n;
const RECIPIENT = '0x' + 'ab'.repeat(20);

async function stubChain(page: import('@playwright/test').Page): Promise<void> {
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
			const balance = chainId === 1 ? DUST : 0n;
			const data =
				'0x' + abiWord(balance) + abiWord(PRICE_8DP) + abiWord(0) + abiWord(0) + abiWord(0);
			if (n === 0) return '0x' + abiWord(0);
			return encodeAggregate3Result(Array.from({ length: n }, () => ({ success: true, data })));
		}
		if (method === 'eth_getLogs') return [];
		return undefined;
	});
}

test('Max says why it filled nothing when the fee outruns the balance', async ({ page }) => {
	await stubChain(page);
	await stubRelay(page, RELAY, happyRelay('0x' + 'a1'.repeat(32), '0x' + 'b2'.repeat(32)));

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

	await page
		.getByRole('button', { name: en('componentsUi.dock.send') })
		.first()
		.click();
	await expect(page.getByRole('heading', { name: en('send.selectTokenTitle') })).toBeVisible();
	await page.getByText('ETH', { exact: true }).first().click();

	await page.getByRole('textbox', { name: en('send.recipientLabel') }).fill(RECIPIENT);
	await page.getByRole('button', { name: en('send.maxBtn') }).click();

	// The fill is `0` — correct: the reserve is larger than everything there is.
	await expect(page.getByRole('textbox', { name: 'ETH' })).toHaveValue('0', { timeout: 30_000 });
	// …and this is the part that did not exist: the core's sentence, on screen.
	await expect(
		page.getByText(en('send.warnInsufficientGas').replace('{{sym}}', 'ETH'))
	).toBeVisible({ timeout: 30_000 });
	// The gate stays honestly shut behind it.
	await expect(page.getByRole('button', { name: en('send.continueBtn') })).toBeDisabled();
});
