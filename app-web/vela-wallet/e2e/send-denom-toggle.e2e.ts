/**
 * The ⇄ under the amount, pressed (issue 197), hermetically.
 *
 * The report: on the Send amount screen the up/down chevron beside the "≈ $…"
 * line "does nothing when clicked. There is no response, no toggle, no visible
 * effect." It was an intended control — `send.rs` has carried
 * `Event::ToggleFiatInput` and three `denom_toggle_*` view fields the whole
 * time — that this shell drew and never wired.
 *
 * Same stubs as `send-fee-over-balance.e2e.ts`, with a balance worth sending:
 * a priced ETH, a figure typed in tokens, one press, and the entry field is
 * counted in the display currency with the tokens on the line beneath.
 */
import { expect, test } from '@playwright/test';
import { CHAINS } from '../src/lib/services/chains';
import { en } from './live-helpers';
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
test.setTimeout(120_000);

const STUB = 'https://stub-rpc.test/rpc';
/** 1 ETH held, priced at $3,000 — enough that the money is the point. */
const BALANCE = 1_000_000_000_000_000_000n;
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
			const balance = chainId === 1 ? BALANCE : 0n;
			const data =
				'0x' + abiWord(balance) + abiWord(PRICE_8DP) + abiWord(0) + abiWord(0) + abiWord(0);
			if (n === 0) return '0x' + abiWord(0);
			return encodeAggregate3Result(Array.from({ length: n }, () => ({ success: true, data })));
		}
		if (method === 'eth_getLogs') return [];
		return undefined;
	});
}

test('the ⇄ beside the fiat estimate swaps which unit the amount is typed in', async ({ page }) => {
	await stubChain(page);

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
	await page.getByRole('textbox', { name: 'ETH' }).fill('0.5');

	// Found the way the reporter finds it: the only button on the form whose
	// text carries the "≈ $…" estimate. By TEXT and not by accessible name on
	// purpose — the control exists before this fix too, so the assertions below
	// are about what pressing it does, not about how it is labelled.
	const toggle = page.getByRole('button').filter({ hasText: /≈/ });
	await expect(toggle).toHaveText(/\$1,500\.00/, { timeout: 30_000 });
	await expect(toggle).toBeEnabled();

	await toggle.click();

	// Pressed: the field is now counted in the display currency — which is the
	// whole response the report said never came — and the tokens moved to the
	// line beneath.
	const inUsd = page.getByRole('textbox', { name: 'USD' });
	await expect(inUsd).toBeVisible({ timeout: 30_000 });
	await expect(inUsd).toHaveValue(/^1500/);
	await expect(toggle).toHaveText(/0\.5 ETH/);

	// And back, because a door in must have a door out.
	await toggle.click();
	await expect(page.getByRole('textbox', { name: 'ETH' })).toHaveValue('0.5');
	await expect(toggle).toHaveText(/\$1,500\.00/);
});
