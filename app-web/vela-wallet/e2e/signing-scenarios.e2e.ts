/**
 * The signing sheet, on the real machines (spec 026 T245 — SC-203).
 *
 * Requests are fired from the page's own requester — the seam a transport will
 * plug into in 027 — so what is exercised here is the sheet, the four cores
 * behind it and the answer that goes back, without a transport in the way.
 *
 * The property that matters most: an unlimited approval cannot be signed by
 * sliding. The core detects it, the guard offers no cap, the slider stays shut
 * — and rejecting answers the requester with 4001, because dismissal IS the
 * refusal (the 022 contract draws no reject button).
 */
import { expect, test } from '@playwright/test';
import { en } from './live-helpers';
import { denyOffOrigin, happyRelay, stubJsonRpc, stubRelay } from './stub-chain';

test.use({ viewport: { width: 390, height: 844 } });
test.setTimeout(90_000);

// The app asks the Cloudflare relay (`vela-relay-cf`, endpoints.ts since spec 060);
// the older host is kept so a stub written against either still intercepts.
const RELAY = /vela-relay(-cf)?\.getvela\.app/;
const SPENDER = '0x1111111254EEB25477B68fb85Ed929f73A960582';
const USDC = '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48';
const MAX_UINT = 'f'.repeat(64);
const FIXTURE_PUBLIC_KEY =
	'04197db9030a1e166bec2cee05e0ddb94b26ee0b6d6f429f1748cda4eedac36f04fe546861a9c9dfaf75719b53c75e0b933d4aad6d325f18c75776a260d507647b';

/** `approve(spender, amount)` calldata — the shape the guard rules on. */
function approveCalldata(amountHex: string): string {
	return '0x095ea7b3' + SPENDER.slice(2).toLowerCase().padStart(64, '0') + amountHex;
}

/** A signed-in wallet with the dev gate on, so the requester exists. */
async function seedSigner(page: import('@playwright/test').Page): Promise<void> {
	await page.addInitScript((key) => {
		localStorage.setItem('vela.intro.seen', String(Date.now()));
		localStorage.setItem('vela.dev.console', '1');
		localStorage.setItem(
			'vela.accounts',
			JSON.stringify([
				{
					id: 'e2e-credential-id',
					name: 'E2E Wallet',
					address: '0xD400866e00B055B20752a826CD5C89b811de130b',
					public_key_hex: key,
					created_at_iso: '2026-01-01T00:00:00.000Z',
					keys: []
				}
			])
		);
		localStorage.setItem('vela.activeAccountIndex', '0');
	}, FIXTURE_PUBLIC_KEY);
}

async function openWallet(page: import('@playwright/test').Page): Promise<void> {
	await denyOffOrigin(page);
	await stubJsonRpc(page, /stub-rpc\.test\/rpc\/(\d+)/, (method) => {
		if (method === 'eth_chainId') return '0x1';
		if (method === 'eth_blockNumber') return '0x10';
		if (method === 'eth_getCode') return '0x6080';
		return undefined;
	});
	await stubRelay(page, RELAY, happyRelay('0x' + '11'.repeat(32), '0x' + '22'.repeat(32)));
	await seedSigner(page);
	await page.goto('/en/wallet');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	// The requester installs behind the dev gate, with the console.
	await page.waitForFunction(
		() => (window as unknown as { vela?: { requester?: unknown } }).vela?.requester !== undefined,
		null,
		{ timeout: 20_000 }
	);
}

/** Fire a request from the page and keep its promise for later. */
async function fire(
	page: import('@playwright/test').Page,
	method: string,
	params: unknown[]
): Promise<void> {
	await page.evaluate(
		([m, p]) => {
			const vela = window as unknown as {
				vela: { requester: { fire(method: string, params: unknown[]): Promise<unknown> } };
				__answer?: unknown;
				__error?: unknown;
			};
			vela.__answer = undefined;
			vela.__error = undefined;
			void vela.vela.requester
				.fire(m as string, p as unknown[])
				.then((result) => (vela.__answer = result))
				.catch((error) => (vela.__error = error));
		},
		[method, params] as const
	);
}

test('an unlimited approval cannot be slid: the chip is dead and the slider stays shut', async ({
	page
}) => {
	await openWallet(page);
	await fire(page, 'eth_sendTransaction', [
		{
			to: USDC,
			data: approveCalldata(MAX_UINT),
			value: '0x0',
			from: '0xD400866e00B055B20752a826CD5C89b811de130b'
		}
	]);

	// The sheet is the core's: it opens because `sign_request` says so.
	const slider = page.getByRole('button', { name: /^Slide to confirm/ });
	await expect(slider).toBeVisible({ timeout: 25_000 });

	// The guard's verdict, on screen: the requested cap reads "Unlimited" and
	// its own chip cannot be chosen.
	await expect(
		page.getByText(en('componentsUi.signingApprove.unlimitedValue')).first()
	).toBeVisible();
	const requestedChip = page.getByRole('button', {
		name: en('componentsUi.signingApprove.requested')
	});
	await expect(requestedChip).toBeDisabled();

	// And the slider is shut — the mandate is a gate, not a warning.
	await expect(slider).toBeDisabled();
	// Nothing invented in the copy: the decode warning names the real length.
	await expect(page.getByText(/\(\d+ bytes\)/)).toBeVisible();
});

test('rejecting answers the requester with 4001 — dismissal IS the refusal', async ({ page }) => {
	await openWallet(page);
	await fire(page, 'personal_sign', ['0x68656c6c6f', '0xD400866e00B055B20752a826CD5C89b811de130b']);

	const sheet = page.getByRole('button', { name: /^Slide to confirm/ });
	await expect(sheet).toBeVisible({ timeout: 25_000 });

	// The 022 contract: there is no reject button. Closing is the answer.
	await page.keyboard.press('Escape');
	await expect
		.poll(
			async () =>
				await page.evaluate(
					() => (window as unknown as { __error?: { code?: number } }).__error?.code ?? null
				),
			{ timeout: 20_000 }
		)
		.toBe(4001);
});
