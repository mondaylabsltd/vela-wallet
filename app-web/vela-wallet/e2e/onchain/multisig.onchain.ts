/**
 * Multisig accounts, end to end on a Gnosis fork: create one (nothing
 * deployed, the genesis recorded on the creator's passkey proof), fund it,
 * propose a transfer, collect the approvals, execute — the multisig and its
 * deployment in the executor's one user operation — and see the funds arrive.
 */
import { expect, test, type Page } from '@playwright/test';
import { en } from '../live-helpers';
import {
	FIXTURES,
	UNREAD_CHAIN,
	balanceOf,
	enterParallel,
	setBalance,
	useHarness
} from './harness';

const SHOTS = 'e2e/onchain/.shots';
const RECIPIENT = '0x000000000000000000000000000000000000bEEF';

/**
 * The send every account has — token, recipient, amount, Continue — whose
 * confirm, for a multisig, proposes and approves instead of sending.
 */
async function proposeSend(
	page: Page,
	amount: string,
	name: string,
	coin = { row: /^xDAI Gnosis/, symbol: 'xDAI' }
): Promise<void> {
	await page
		.getByRole('button', { name: en('componentsUi.dock.send') })
		.first()
		.click();
	await expect(page.getByRole('heading', { name: en('send.selectTokenTitle') })).toBeVisible();
	// The picker's row, not the home's: on the desktop both are on screen, and
	// the picker (the third column) is the last.
	await page.getByRole('button', { name: coin.row }).last().click({ timeout: 60_000 });
	await page.getByRole('textbox', { name: en('send.recipientLabel') }).fill(RECIPIENT);
	await page.getByRole('textbox', { name: coin.symbol }).fill(amount);
	await expect(page.getByText('2 of', { exact: false }).first()).toBeVisible();
	await shot(page, `${name}-form`);
	const advance = page.getByRole('button', { name: en('send.continueBtn') });
	await expect(advance).toBeEnabled({ timeout: 30_000 });
	await advance.click();
	await expect(page.getByRole('heading', { name: en('send.confirmTitle') })).toBeVisible();
	await expect(page.getByText(`${amount} ${coin.symbol}`, { exact: true }).first()).toBeVisible();
	await shot(page, `${name}-confirm`);
	await page.getByRole('button', { name: 'Propose and approve' }).click();
}

async function shot(page: Page, name: string): Promise<void> {
	await page.screenshot({ path: `${SHOTS}/${name}.png`, fullPage: false });
}

function errors(page: Page): string[] {
	const seen: string[] = [];
	page.on('pageerror', (e) => seen.push(e.message));
	// The wallet's own log, for whoever reads a failed run.
	page.on('console', (m) => {
		if (!/worldchain|Chainlink|Failed to load resource|favicon/.test(m.text())) {
			console.log(`[page ${m.type()}] ${m.text().slice(0, 400)}`);
		}
	});
	page.on('response', async (r) => {
		if (!/127\.0\.0\.1:(8787|8790)/.test(r.url())) return;
		const body = await r.text().catch(() => '');
		console.log(
			`[net ${r.status()}] ${r.request().method()} ${r.url().replace(/^http:\/\/127\.0\.0\.1:/, '')} ${(r.request().postData() ?? '').slice(0, 160)} → ${body.slice(0, 300)}`
		);
	});
	return seen;
}

test('a 2-of-3 multisig on one device: create, fund, propose, approve and execute', async ({
	page,
	context
}) => {
	const thrown = errors(page);
	await useHarness(context);
	await enterParallel(page);
	await shot(page, '01-wallet');

	await test.step('create', async () => {
		await page
			.getByRole('button', { name: /Parallel One/ })
			.first()
			.click();
		await shot(page, '02-switcher');
		await page.getByRole('button', { name: 'Create multisig' }).click();
		await expect(page.getByText('New multisig')).toBeVisible();
		await page.getByText('Parallel Two', { exact: true }).click();
		await page.getByText('Parallel Three', { exact: true }).click();
		await shot(page, '03-members');
		await page.getByRole('button', { name: 'Next' }).click();
		await expect(page.getByText('2 of 3')).toBeVisible();
		await shot(page, '04-threshold');
		await page.getByRole('button', { name: 'Next' }).click();
		await shot(page, '05-review');
		await page.getByRole('textbox', { name: 'Name' }).fill('Team fund');
		await page.getByRole('button', { name: 'Create', exact: true }).click();
		await expect(page.getByText('Multisig created')).toBeVisible();
		await shot(page, '06-created');
		await page.getByRole('button', { name: 'Done' }).click();
	});

	const multisig = await page.evaluate(() => localStorage.getItem('vela.activeMultisig'));
	expect(multisig).toMatch(/^0x[0-9a-fA-F]{40}$/);
	await expect(page.getByText('2 of 3 members approve').first()).toBeVisible();
	// The name is the multisig's own, from its genesis.
	await expect(page.getByText('Team fund', { exact: true }).first()).toBeVisible();
	await shot(page, '07-home');

	await test.step('fund and propose', async () => {
		await setBalance(multisig!, 5n * 10n ** 18n);
		await page.reload();
		await proposeSend(page, '1.5', '08-propose');
		await expect(page.getByText('1/2 approved').first()).toBeVisible();
		await shot(page, '09-proposal');
	});

	await test.step('approve and execute as the second member', async () => {
		const before = await balanceOf(RECIPIENT);
		await page.getByRole('button', { name: 'Approve and execute' }).click();
		await expect(page.getByText('Executed', { exact: true }).first()).toBeVisible({
			timeout: 120_000
		});
		await shot(page, '10-executed');
		const after = await balanceOf(RECIPIENT);
		expect(after - before).toBe(15n * 10n ** 17n);
	});

	expect(thrown).toEqual([]);
	void FIXTURES;
});

test('two devices: one proposes, the other opens the link, approves and executes', async ({
	browser
}) => {
	const a = await browser.newContext({ viewport: { width: 430, height: 900 } });
	const b = await browser.newContext({ viewport: { width: 430, height: 900 } });
	await useHarness(a);
	await useHarness(b);
	const pageA = await a.newPage();
	const pageB = await b.newPage();
	const thrownA = errors(pageA);
	const thrownB = errors(pageB);
	await enterParallel(pageA, [FIXTURES.one]);
	await enterParallel(pageB, [FIXTURES.two]);

	await test.step('device A creates a 2-of-2 with a member added by address', async () => {
		await pageA
			.getByRole('button', { name: /Parallel One/ })
			.first()
			.click();
		await pageA.getByRole('button', { name: 'Create multisig' }).click();
		await pageA.getByPlaceholder("Member's address (0x…)").fill(FIXTURES.two);
		await pageA.getByRole('button', { name: 'Add', exact: true }).click();
		await expect(pageA.getByText('Members · 2')).toBeVisible();
		await pageA.getByRole('button', { name: 'Next' }).click();
		await expect(pageA.getByText('2 of 2')).toBeVisible();
		await expect(pageA.getByText(/Every member must approve/)).toBeVisible();
		await shot(pageA, 'x1-threshold-all');
		await pageA.getByRole('button', { name: 'Next' }).click();
		await pageA.getByRole('textbox', { name: 'Name' }).fill('Shared 2/2');
		await pageA.getByRole('button', { name: 'Create', exact: true }).click();
		await pageA.getByRole('button', { name: 'Done' }).click();
		await expect(pageA.getByText(/This device/i).first()).toBeVisible();
		await shot(pageA, 'x2-home-a');
	});

	const multisig = (await pageA.evaluate(() => localStorage.getItem('vela.activeMultisig')))!;
	await setBalance(multisig, 3n * 10n ** 18n);

	let link = '';
	await test.step('device A proposes and shares the link', async () => {
		await pageA.reload();
		await proposeSend(pageA, '0.25', 'x2-propose-a');
		await expect(pageA.getByText('1/2 approved').first()).toBeVisible();
		// Device A cannot finish it: the other member must approve.
		await expect(pageA.getByRole('button', { name: 'Approve and execute' })).toHaveCount(0);
		link = (await pageA.locator('code.link').textContent())!.trim();
		await shot(pageA, 'x3-proposal-a');
	});
	expect(link).toContain('#ms=100.');

	await test.step('device B opens the link, approves and executes', async () => {
		const before = await balanceOf(RECIPIENT);
		await pageB.goto(link);
		await expect(pageB.getByText('Send 0.25 xDAI').first()).toBeVisible({ timeout: 90_000 });
		await shot(pageB, 'x4-proposal-b');
		// The name came with the genesis the service holds, checked by the core.
		await expect(pageB.getByText('Shared 2/2', { exact: true }).first()).toBeVisible();
		await pageB.getByRole('button', { name: 'Approve and execute' }).click();
		await expect(pageB.getByText('Executed', { exact: true }).first()).toBeVisible({
			timeout: 120_000
		});
		await shot(pageB, 'x5-executed-b');
		expect((await balanceOf(RECIPIENT)) - before).toBe(25n * 10n ** 16n);
	});

	await test.step('device A sees it done', async () => {
		await pageA.reload();
		await expect(pageA.getByText('2 of 2 members approve').first()).toBeVisible();
		// Nothing left waiting: the executed transaction is gone from the panel.
		await expect(pageA.getByText('Waiting for approval')).toHaveCount(0, { timeout: 30_000 });
		// Executed on device B, found on chain here, and in this device's activity.
		await expect(pageA.getByRole('main').getByText(/0\.25/).first()).toBeVisible({
			timeout: 60_000
		});
		await expect(pageA.getByText(/Not shared yet/)).toHaveCount(0);
		await shot(pageA, 'x6-home-a-after');
	});

	expect(thrownA).toEqual([]);
	expect(thrownB).toEqual([]);
	await a.close();
	await b.close();
});

test('the desktop: create, propose and the transaction dialog', async ({ browser }) => {
	const ctx = await browser.newContext({ viewport: { width: 1360, height: 900 } });
	await useHarness(ctx);
	const page = await ctx.newPage();
	const thrown = errors(page);
	await enterParallel(page);
	await page
		.getByRole('button', { name: /Parallel One/ })
		.first()
		.click();
	await shot(page, 'd1-switcher');
	await page.getByRole('button', { name: 'Create multisig' }).click();
	await page.getByText('Parallel Two', { exact: true }).click();
	await shot(page, 'd2-create');
	await page.getByRole('button', { name: 'Next' }).click();
	await page.getByRole('button', { name: 'Next' }).click();
	await page.getByRole('textbox', { name: 'Name' }).fill('Desk fund');
	await page.getByRole('button', { name: 'Create', exact: true }).click();
	await page.getByRole('button', { name: 'Done' }).click();
	const multisig = (await page.evaluate(() => localStorage.getItem('vela.activeMultisig')))!;
	await setBalance(multisig, 2n * 10n ** 18n);
	await page.reload();
	await expect(page.getByText('2 of 2 members approve').first()).toBeVisible();
	await shot(page, 'd3-home');
	await proposeSend(page, '0.5', 'd4-propose');
	await expect(page.getByText('1/2 approved').first()).toBeVisible();
	await shot(page, 'd5-proposal');
	expect(thrown).toEqual([]);
	await ctx.close();
});

test('a network the genesis service does not read: propose, approve and execute there', async ({
	browser
}) => {
	const ctx = await browser.newContext({ viewport: { width: 430, height: 900 } });
	await useHarness(ctx);
	const page = await ctx.newPage();
	const thrown = errors(page);
	await enterParallel(page);
	await page
		.getByRole('button', { name: /Parallel One/ })
		.first()
		.click();
	await page.getByRole('button', { name: 'Create multisig' }).click();
	await page.getByText('Parallel Two', { exact: true }).click();
	await page.getByRole('button', { name: 'Next' }).click();
	await page.getByRole('button', { name: 'Next' }).click();
	await page.getByRole('textbox', { name: 'Name' }).fill('Base fund');
	await page.getByRole('button', { name: 'Create', exact: true }).click();
	await page.getByRole('button', { name: 'Done' }).click();
	const multisig = (await page.evaluate(() => localStorage.getItem('vela.activeMultisig')))!;
	// Funds on "Base" only: the second fork, which the service has no RPC for.
	await setBalance(multisig, 2n * 10n ** 18n, UNREAD_CHAIN);
	await page.reload();
	await expect(page.getByText('2 of 2 members approve').first()).toBeVisible();

	await proposeSend(page, '0.5', 'u1-propose', { row: /^ETH Base/, symbol: 'ETH' });
	await expect(page.getByText('1/2 approved').first()).toBeVisible();
	await shot(page, 'u2-proposal');

	const before = await balanceOf(RECIPIENT, UNREAD_CHAIN);
	await page.getByRole('button', { name: 'Approve and execute' }).click();
	await expect(page.getByText('Executed', { exact: true }).first()).toBeVisible({
		timeout: 120_000
	});
	await shot(page, 'u3-executed');
	expect((await balanceOf(RECIPIENT, UNREAD_CHAIN)) - before).toBe(5n * 10n ** 17n);
	expect(thrown).toEqual([]);
	await ctx.close();
});
