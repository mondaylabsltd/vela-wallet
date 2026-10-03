/**
 * A relay with no gas on the chain being sent on (reported 2026-10-03).
 *
 * The relay says so itself: `GET /v1/treasury/{chainId}` answers
 * `bootstrapNeeded: true` with a zero balance — on Sepolia today, live. The core
 * is built to stop there: `precheck_settle` opens the treasury sheet and never
 * enters confirm. The report was that the web wallet goes on to send anyway,
 * without a word. This drives that exact path, so the answer is observed, not
 * argued.
 *
 * (The rest of this header is the stub's origin.)
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
/** 1 ETH — enough that nothing but the relay can stop this send. */
const DUST = 10n ** 18n;
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

const EMPTY_TREASURY = {
	chainId: 1,
	address: '0x3e59292e18417f814112f731e7163534c6d2fe3c',
	asset: 'native',
	balance: '0x0',
	floor: '0x5af3107a4000',
	bootstrapNeeded: true
};

async function toTheAmount(page: import('@playwright/test').Page): Promise<void> {
	await page.goto('/en/parallel');
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
	await page.getByRole('textbox', { name: 'ETH' }).fill('0.01');
}

test('a relay with no gas on this chain stops the send at Continue, and says what to do', async ({
	page
}) => {
	await stubChain(page);
	await stubRelay(page, RELAY, happyRelay('0x' + 'a1'.repeat(32), '0x' + 'b2'.repeat(32)), {
		treasury: EMPTY_TREASURY
	});
	await toTheAmount(page);

	const proceed = page.getByRole('button', { name: en('send.continueBtn') });
	await expect(proceed).toBeEnabled({ timeout: 30_000 });
	await proceed.click();

	// The sheet the core opens for a depleted relayer.
	await expect(page.getByText(en('componentsUi.treasuryBootstrap.title'))).toBeVisible({
		timeout: 30_000
	});
	// …and the step after it never arrives: nothing to sign, nothing submitted.
	await expect(page.getByRole('button', { name: en('send.confirmSendBtn') })).toHaveCount(0);
});

/**
 * Spec 098 §2: the relay says it cannot serve this chain (404). Funding would
 * not help, so this is not the funding sheet — and before 098 it went on to
 * confirm and the passkey, and failed after the person had signed.
 */
test('a relay that cannot serve this chain stops the send at Continue, and says so', async ({
	page
}) => {
	await stubChain(page);
	await stubRelay(page, RELAY, happyRelay('0x' + 'a1'.repeat(32), '0x' + 'b2'.repeat(32)), {
		treasury: null
	});
	await toTheAmount(page);

	const proceed = page.getByRole('button', { name: en('send.continueBtn') });
	await expect(proceed).toBeEnabled({ timeout: 30_000 });
	await proceed.click();

	await expect(page.getByText(en('componentsUi.relayUnreachable.title'))).toBeVisible({
		timeout: 30_000
	});
	// Chain 1 ships, so it is the operator's to fix: the report row is there.
	await expect(page.getByText(en('componentsUi.relayUnreachable.operatorLead'))).toBeVisible();
	await expect(page.getByRole('button', { name: en('send.confirmSendBtn') })).toHaveCount(0);
	// No funding sheet: gas cannot help a relay that cannot reach the network.
	await expect(page.getByText(en('componentsUi.treasuryBootstrap.title'))).toHaveCount(0);
});

/** Spec 098 §4: the funding sheet says what it has against what it needs, and that it is watching. */
test('the funding sheet shows the balance against the floor, and says it closes by itself', async ({
	page
}) => {
	await stubChain(page);
	await stubRelay(page, RELAY, happyRelay('0x' + 'a1'.repeat(32), '0x' + 'b2'.repeat(32)), {
		treasury: EMPTY_TREASURY
	});
	await toTheAmount(page);
	await page.getByRole('button', { name: en('send.continueBtn') }).click();
	await expect(page.getByText(en('componentsUi.treasuryBootstrap.title'))).toBeVisible({
		timeout: 30_000
	});
	// 0 against a floor of 0x5af3107a4000 wei = 0.0001 ETH.
	await expect(
		page.getByText(
			en('componentsUi.treasuryBootstrap.balanceLine')
				.replace('{{balance}}', '0')
				.replace('{{floor}}', '0.0001')
				.replace('{{symbol}}', 'ETH')
		)
	).toBeVisible();
	await expect(page.getByText(en('componentsUi.treasuryBootstrap.watching'))).toBeVisible();
});
