/**
 * The signing sheet, on the real machines (spec 026 T245 — SC-203).
 *
 * Requests are fired from the page's own requester — the seam a transport will
 * plug into in 027 — so what is exercised here is the sheet, the four cores
 * behind it and the answer that goes back, without a transport in the way.
 *
 * The property that matters most: an unlimited approval is never signed
 * unseen. The core detects it, the guard keeps the site's ask on its own chip
 * and says the danger (Permit2 bundles revert when the wallet re-encodes the
 * approve — the 2026-09-26 ruling), a cap is one chip away — and rejecting
 * answers the requester with 4001, because dismissal IS the refusal (the 022
 * contract draws no reject button).
 */
import { expect, test } from '@playwright/test';
import { en } from './live-helpers';
import {
	denyOffOrigin,
	happyRelay,
	seedNetworkOverrides,
	stubJsonRpc,
	stubRelay
} from './stub-chain';

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

test('an unlimited approval is kept as asked and said in danger; a cap is one chip away', async ({
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
	const confirm = page.getByTestId('signing-confirm');
	await expect(confirm).toBeVisible({ timeout: 25_000 });

	// The guard's verdict, on screen (2026-09-26): the cap reads "Unlimited",
	// the site's own chip is the one chosen — Permit2 bundles revert when the
	// wallet re-encodes the approve — and the danger is said in words.
	await expect(
		page.getByText(en('componentsUi.signingApprove.unlimitedValue')).first()
	).toBeVisible();
	const requestedChip = page.getByRole('button', {
		name: en('componentsUi.signingApprove.requested')
	});
	await expect(requestedChip).toBeEnabled();
	await expect(requestedChip).toHaveAttribute('aria-pressed', 'true');
	const warning = page.getByText(en('componentsUi.signing.unlimitedWarning'));
	await expect(warning).toBeVisible();

	// A typed cap takes over: the site's chip lets go and the warning goes.
	await page.getByRole('button', { name: en('componentsUi.signingApprove.custom') }).click();
	await page.getByRole('textbox').last().fill('5');
	await expect(requestedChip).toHaveAttribute('aria-pressed', 'false');
	await expect(warning).toHaveCount(0);
	// The cap reads in tokens, never in base units ("5000000" is not 5 USDC).
	await expect(page.getByText(/^5 /).first()).toBeVisible();
});

/**
 * PR 3 device round, items 1 and 3. The sheet's own simulation — the read it
 * already makes for the fee — is read by the core; when the node checked the
 * request and nothing of the person's moves, the sheet says the core's line,
 * "No asset changes", in a balance-changes card after the request's own
 * blocks. It lands late (the node answers after the sheet has opened), and
 * the confirm, in the sheet's foot outside the scroll, is where it was.
 */
test('a request that moves nothing says "No asset changes" — and its landing moves no confirm', async ({
	page
}, testInfo) => {
	await openWallet(page);
	// Mainnet's node is the stub, and it simulates — when the test lets it
	// answer: an approval runs, and no asset of the account moves.
	let asked = 0;
	let answer: () => void = () => {};
	const held = new Promise<void>((resolve) => (answer = resolve));
	await page.route(/stub-rpc\.test\/rpc\/1$/, async (route) => {
		const body = JSON.parse(route.request().postData() ?? '{}') as { method?: string; id?: number };
		if (body.method !== 'eth_simulateV1') return route.fallback();
		asked += 1;
		await held;
		return route.fulfill({
			contentType: 'application/json',
			body: JSON.stringify({
				jsonrpc: '2.0',
				id: body.id ?? 1,
				result: [{ calls: [{ status: '0x1', logs: [] }] }]
			})
		});
	});
	await seedNetworkOverrides(page, [{ chainId: 1, rpcURL: 'https://stub-rpc.test/rpc/1' }]);
	await page.reload();
	await page.waitForFunction(
		() => (window as unknown as { vela?: { requester?: unknown } }).vela?.requester !== undefined,
		null,
		{ timeout: 20_000 }
	);

	await fire(page, 'eth_sendTransaction', [
		{
			to: USDC,
			// approve(spender, 100 USDC): a cap, so the sheet is an ordinary one.
			data: approveCalldata((100_000_000).toString(16).padStart(64, '0')),
			value: '0x0',
			from: '0xD400866e00B055B20752a826CD5C89b811de130b'
		}
	]);
	const confirm = page.getByTestId('signing-confirm');
	await expect(confirm).toBeVisible({ timeout: 25_000 });
	const card = page.locator('[role="dialog"] section.balances');
	const line = en('componentsUi.signing.simResultNoChange');
	expect(line).toBe('No asset changes');

	// The node was asked once, and has not answered: nothing is said yet.
	// (PR 3: four seconds on, the core's deadline puts the caution that
	// nothing could be checked in the card's place — the one card that may be
	// there before the answer; `fee-speed.e2e.ts` walks that.)
	await expect.poll(() => asked, { timeout: 20_000 }).toBe(1);
	await expect(
		card.filter({ hasNotText: en('componentsUi.signing.simUnavailableWarning') })
	).toHaveCount(0);
	await expect(page.getByText(line)).toHaveCount(0);
	// The sheet has opened and its fee has had its say: where the confirm rests.
	const restingY = async () => {
		let last = -1;
		await expect
			.poll(
				async () => {
					const y = (await confirm.boundingBox())?.y ?? -1;
					const still = y === last;
					last = y;
					return still;
				},
				{ timeout: 20_000, intervals: [400] }
			)
			.toBe(true);
		return last;
	};
	const was = await restingY();

	answer();
	await expect(card).toBeVisible({ timeout: 20_000 });
	await expect(card).toContainText(en('componentsUi.signing.balanceChangesTitle'));
	await expect(card).toContainText(line);
	// The card and its one line: no balance rows (spec 082 RG6 stands).
	await expect(card.locator('.row')).toHaveCount(0);
	await expect(card).toHaveAttribute('data-verdict', '');
	// Whole, in sight, and the confirm has not moved.
	await expect(card).toBeInViewport({ ratio: 1 });
	expect(await restingY()).toBe(was);
	await expect(confirm).toBeInViewport({ ratio: 1 });
	// One read served the fee and the sheet.
	expect(asked).toBe(1);
	// Nothing else was drawn from the simulation.
	await expect(page.getByText(en('componentsUi.signing.simUnavailableWarning'))).toHaveCount(0);
	await page.screenshot({ path: testInfo.outputPath('no-asset-changes-en.png') });
});

test('rejecting answers the requester with 4001 — the ✕ IS the refusal', async ({ page }) => {
	await openWallet(page);
	await fire(page, 'personal_sign', ['0x68656c6c6f', '0xD400866e00B055B20752a826CD5C89b811de130b']);

	const sheet = page.getByTestId('signing-confirm');
	await expect(sheet).toBeVisible({ timeout: 25_000 });

	// Spec 079: a stray Escape or a tap on the scrim no longer throws the
	// request away — the sheet stays, and the page has no answer.
	await page.keyboard.press('Escape');
	await page
		.locator('.scrim')
		.first()
		.click({ position: { x: 10, y: 10 } });
	await page.waitForTimeout(800);
	await expect(sheet).toBeVisible();
	expect(
		await page.evaluate(
			() => (window as unknown as { __error?: { code?: number } }).__error?.code ?? null
		)
	).toBeNull();

	// The 022 contract: there is no reject button. The quiet ✕ is the answer.
	await page.getByRole('button', { name: en('componentsUi.signing.close'), exact: true }).click();
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

/**
 * Spec 079 ("可信签名器签完后，回到签名提示框，似乎没有任何提示"): the core clears
 * the sheet the moment it answers, and a message signature used to simply
 * vanish. It ends on the signed tick now, which goes by itself. Signed for real
 * in the parallel space (its fixture keys sign headlessly).
 */
test('a signed message ends on the tick, and the tick goes by itself', async ({ page }) => {
	await denyOffOrigin(page);
	await page.addInitScript(() => {
		localStorage.setItem('vela.intro.seen', String(Date.now()));
		localStorage.setItem('vela.dev.console', '1');
	});
	await page.goto('/en/parallel');
	// A click that lands before the page has hydrated does nothing (seen under
	// a loaded runner): ask again until the entry navigates.
	await expect(async () => {
		await page.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
		await page.waitForURL(/\/en\/wallet$/, { timeout: 3_000 });
	}).toPass({ timeout: 30_000 });
	await page.waitForFunction(
		() => (window as unknown as { vela?: { requester?: unknown } }).vela?.requester !== undefined,
		null,
		{ timeout: 20_000 }
	);
	await fire(page, 'personal_sign', ['0x68656c6c6f', '0xD400866e00B055B20752a826CD5C89b811de130b']);

	const confirm = page.getByTestId('signing-confirm');
	await expect(confirm).toBeVisible({ timeout: 25_000 });
	const tick = page.getByText(en('clearSigning.alertSignedTitle'), { exact: true });
	// Watched from the tap on: the tick is a beat, not a state.
	const seen = expect(tick).toBeVisible({ timeout: 20_000 });
	await confirm.click();
	await seen;

	await expect
		.poll(
			async () =>
				await page.evaluate(() => (window as unknown as { __answer?: unknown }).__answer ?? null),
			{ timeout: 20_000 }
		)
		.toMatch(/^0x[0-9a-fA-F]{100,}$/);
	await expect(tick).toBeHidden({ timeout: 5_000 });
});
