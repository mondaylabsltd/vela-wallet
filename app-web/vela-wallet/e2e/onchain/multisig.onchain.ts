/**
 * Multisig accounts, end to end on a Gnosis fork: create one (nothing
 * deployed, the genesis recorded on the creator's passkey proof), fund it,
 * propose a transfer, collect the approvals, execute — the multisig and its
 * deployment in the executor's one user operation — and see the funds arrive.
 */
import { expect, test, type Page } from '@playwright/test';
import { FIXTURES, balanceOf, enterParallel, setBalance, useHarness } from './harness';

const SHOTS = 'e2e/onchain/.shots';
const RECIPIENT = '0x000000000000000000000000000000000000bEEF';

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
		console.log(`[net ${r.status()}] ${r.request().method()} ${r.url().replace(/^http:\/\/127\.0\.0\.1:/, '')} ${(r.request().postData() ?? '').slice(0, 160)} → ${body.slice(0, 300)}`);
	});
	return seen;
}

test('a 2-of-3 multisig on one device: create, fund, propose, approve and execute', async ({ page, context }) => {
	const thrown = errors(page);
	await useHarness(context);
	await enterParallel(page);
	await shot(page, '01-wallet');

	await test.step('create', async () => {
		await page.getByRole('button', { name: /Parallel One/ }).first().click();
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
		await page.getByRole('button', { name: 'Create', exact: true }).click();
		await expect(page.getByText('Multisig created')).toBeVisible();
		await shot(page, '06-created');
		await page.getByRole('button', { name: 'Done' }).click();
	});

	const multisig = await page.evaluate(() => localStorage.getItem('vela.activeMultisig'));
	expect(multisig).toMatch(/^0x[0-9a-fA-F]{40}$/);
	await expect(page.getByText('2 of 3 members approve').first()).toBeVisible();
	await shot(page, '07-home');

	await test.step('fund and propose', async () => {
		await setBalance(multisig!, 5n * 10n ** 18n);
		await page.reload();
		await page.getByRole('button', { name: /Send/ }).first().click();
		await expect(page.getByText('Propose a transfer')).toBeVisible();
		await expect(page.getByText('xDAI').first()).toBeVisible({ timeout: 60_000 });
		await page.getByPlaceholder('0', { exact: true }).fill('1.5');
		await page.getByPlaceholder('0x…').fill(RECIPIENT);
		await shot(page, '08-propose');
		await page.getByRole('button', { name: 'Propose and approve' }).click();
		await expect(page.getByText('1/2 approved').first()).toBeVisible();
		await shot(page, '09-proposal');
	});

	await test.step('approve and execute as the second member', async () => {
		const before = await balanceOf(RECIPIENT);
		await page.getByRole('button', { name: 'Approve and execute' }).click();
		await expect(page.getByText('Executed', { exact: true })).toBeVisible({ timeout: 120_000 });
		await shot(page, '10-executed');
		const after = await balanceOf(RECIPIENT);
		expect(after - before).toBe(15n * 10n ** 17n);
	});

	expect(thrown).toEqual([]);
	void FIXTURES;
});
