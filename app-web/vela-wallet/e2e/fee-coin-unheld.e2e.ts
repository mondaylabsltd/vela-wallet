/**
 * A send cannot be paid for in a coin the account does not hold (issue 211).
 *
 * The report: an account holding pUSD on Polygon and 0 POL was allowed to send
 * with POL as the gas coin, no warning anywhere — and the transaction was then
 * listed as "Confirmed" while nothing had landed on chain and no balance had
 * moved. The wallet had signed an operation whose fee leg moves a coin that is
 * not there, which can only revert.
 *
 * This is that account, hermetically: one stablecoin held, zero native, the
 * relay quoting its fee in the native coin. What the screen must do is say so
 * and refuse — before the passkey, not after the money is reported as sent.
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
import { answerAggregate3, isAggregate3, MULTICALL_SEL as SEL, word } from './stub-multicall';

test.use({ viewport: { width: 390, height: 844 } });
test.setTimeout(120_000);

const STUB = 'https://stub-rpc.test/rpc';
// The app asks the Cloudflare relay (`vela-relay-cf`, endpoints.ts since spec 060);
// the older host is kept so a stub written against either still intercepts.
const RELAY = /vela-relay(-cf)?\.getvela\.app/;
const USDC = '0x' + 'cc'.repeat(20);
const HUNDRED_USDC = 100_000_000n;
const RECIPIENT = '0x' + 'ab'.repeat(20);
const USER_OP_HASH = '0x' + 'a1'.repeat(32);
const TX_HASH = '0x' + 'b2'.repeat(32);

/** Ethereum with 100 USDC held and not one wei of the native coin. */
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
					// The whole premise: no native coin, so no row for it — a zero
					// balance never reaches the wallet's holdings at all.
					case SEL.getEthBalance:
						return { success: true, data: '0x' + word(0) };
					case SEL.balanceOf:
						return inner.target === USDC
							? { success: true, data: '0x' + word(HUNDRED_USDC) }
							: { success: true, data: '0x' + word(0) };
					case SEL.decimals:
						return { success: true, data: '0x' + word(6) };
					default:
						return undefined;
				}
			});
		}
		return undefined;
	});
}

test('a gas coin the account does not hold is refused, not signed', async ({ page }) => {
	const submits: string[] = [];
	await stubChain(page);
	const relay = happyRelay(USER_OP_HASH, TX_HASH, () => 'pending');
	await stubRelay(page, RELAY, (method, params) => {
		if (method === 'eth_sendUserOperation') submits.push(method);
		if (method === 'vela_getInBandGasQuote') {
			// The relay prices this transfer in the native coin, and reports the
			// Safe's own balance of it: nothing.
			return [
				{
					recipient: '0x' + 'fe'.repeat(20),
					asset: 'native',
					feeToken: null,
					balance: '0x0',
					decimals: 18,
					symbol: 'ETH',
					usdBalance: '0',
					usdPrice: '3000'
				}
			];
		}
		return relay(method, params);
	});

	await page.goto('/en/parallel');
	await page.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
	await page.waitForURL(/\/en\/wallet$/);
	await seedNetworkOverrides(
		page,
		CHAINS.map((c) => ({ chainId: c.chainId, rpcURL: `${STUB}/${c.chainId}` }))
	);
	await page.reload();
	await expect(page.getByText('$100', { exact: true })).toBeVisible({ timeout: 25_000 });

	// The stablecoin is all there is to send.
	await page
		.getByRole('button', { name: en('componentsUi.dock.send') })
		.first()
		.click();
	await expect(page.getByRole('heading', { name: en('send.selectTokenTitle') })).toBeVisible();
	await page.getByText('USDC', { exact: true }).first().click();
	await page.getByRole('textbox', { name: en('send.recipientLabel') }).fill(RECIPIENT);
	await page.getByRole('textbox', { name: 'USDC' }).fill('10');

	// The quote lands in the native coin — and the screen says, without being
	// asked, that this account cannot pay in it.
	const needsGas = en('send.warnNeedGas').replace('{{sym}}', 'ETH');
	await expect(page.getByText(needsGas)).toBeVisible({ timeout: 30_000 });

	// The fee sheet shows the row for context and refuses it: the tap that used
	// to do nothing at all now has a reason attached to it.
	await page.getByRole('button', { name: en('send.feeTokenLabel') }).click();
	const ethRow = page.getByRole('button', { name: /ETH/ }).first();
	await expect(ethRow).toBeDisabled();
	await expect(ethRow).toContainText(en('send.warnInsufficientGas').replace('{{sym}}', 'ETH'));
	// The sheet's own scrim closes it — the same tap a person makes.
	await page.locator('.scrim').first().click();
	await expect(ethRow).toBeHidden();

	// Continue refuses out loud and stays on the form. Nothing is signed, and
	// nothing reaches the relay to be reported as sent.
	await page.getByRole('button', { name: en('send.continueBtn') }).click();
	await expect(
		page.getByText(en('send.alertInsufficientBalanceTitle'), { exact: false })
	).toBeVisible();
	await expect(page.getByRole('heading', { name: en('send.confirmTitle') })).toBeHidden();
	expect(submits).toEqual([]);
});
