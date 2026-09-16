/**
 * Leaving a signed-in wallet, on the route the button is actually on.
 *
 * Issue 214 (web, 2026-09-15): the settings screen's red "Sign Out" confirm
 * did nothing. `session.signOut()` reached the core and the core answered —
 * but the sheet that carries the answer was rendered only inside the WALLET
 * route, so on settings there was nothing to draw it. The dialog sat there,
 * the session stayed, and the sheet turned up orphaned the next time somebody
 * opened the wallet.
 *
 * So the test is about the ROUTE, not the machine: start on settings, never
 * leave it, and end signed out on Welcome with the two session keys gone.
 * Both layouts, because each has its own confirm surface (the phone's bottom
 * sheet, the desktop's dialog) and the bug was invisible to whichever one you
 * did not open.
 */
import { expect, test, type Page } from '@playwright/test';

const ACCOUNT = {
	id: 'e2e-sign-out',
	name: 'Leaving Wallet',
	address: '0x14fb1f4e2b9c7a5d8e3f6a1b4c7d9e2f5a8b1d1e',
	public_key_hex: '04' + 'ab'.repeat(64),
	created_at_iso: '2026-01-01T00:00:00.000Z',
	keys: []
};

test.beforeEach(async ({ page }) => {
	await page.addInitScript((account) => {
		window.localStorage.setItem('vela.intro.seen', String(Date.now()));
		window.localStorage.setItem('vela.accounts', JSON.stringify([account]));
		window.localStorage.setItem('vela.activeAccountIndex', '0');
	}, ACCOUNT);
});

/** What "signed out" means on disk: the two keys that constitute being signed in. */
async function storedSession(
	page: Page
): Promise<{ accounts: string | null; index: string | null }> {
	return page.evaluate(() => ({
		accounts: window.localStorage.getItem('vela.accounts'),
		index: window.localStorage.getItem('vela.activeAccountIndex')
	}));
}

/** The core's sheet: the one with a way back out beside the red button. */
function coreSheet(page: Page) {
	return page.getByRole('dialog').filter({ has: page.getByRole('button', { name: 'Cancel' }) });
}

test.describe('on the phone layout', () => {
	test.use({ viewport: { width: 390, height: 844 } });

	test('the confirm signs out without leaving settings first', async ({ page }) => {
		await page.goto('/en/settings');
		await expect(page.getByText('Leaving Wallet').first()).toBeVisible();

		await page.getByRole('button', { name: 'Sign Out', exact: true }).click();
		await page.getByRole('dialog').getByRole('button', { name: 'Sign Out', exact: true }).click();

		// The core's sheet is what answers that button — and it is the ONLY thing
		// on screen: the settings sheet closed rather than burying it.
		await expect(coreSheet(page)).toBeVisible();
		await expect(page.getByRole('dialog')).toHaveCount(1);

		await coreSheet(page).getByRole('button', { name: 'Sign Out', exact: true }).click();

		await expect(page).toHaveURL(/\/en\/?$/);
		expect(await storedSession(page)).toEqual({ accounts: null, index: null });
	});

	test('cancelling keeps the session, and the row still works afterwards', async ({ page }) => {
		await page.goto('/en/settings');
		await page.getByRole('button', { name: 'Sign Out', exact: true }).click();
		await page.getByRole('dialog').getByRole('button', { name: 'Sign Out', exact: true }).click();
		await coreSheet(page).getByRole('button', { name: 'Cancel' }).click();

		await expect(page.getByRole('dialog')).toHaveCount(0);
		await expect(page).toHaveURL(/\/en\/settings/);
		expect((await storedSession(page)).accounts).not.toBeNull();

		// A refusal must not latch the machine's single-flight check shut.
		await page.getByRole('button', { name: 'Sign Out', exact: true }).click();
		await page.getByRole('dialog').getByRole('button', { name: 'Sign Out', exact: true }).click();
		await expect(coreSheet(page)).toBeVisible();
	});
});

test.describe('on the wide layout', () => {
	test.use({ viewport: { width: 1280, height: 900 } });

	test('the account panel’s confirm signs out', async ({ page }) => {
		await page.goto('/en/settings');
		await page.getByRole('button', { name: 'Account', exact: true }).click();
		await page.getByRole('button', { name: 'Sign Out', exact: true }).click();
		await page.getByRole('dialog').getByRole('button', { name: 'Sign Out', exact: true }).click();

		await expect(coreSheet(page)).toBeVisible();
		await expect(page.getByRole('dialog')).toHaveCount(1);

		await coreSheet(page).getByRole('button', { name: 'Sign Out', exact: true }).click();

		await expect(page).toHaveURL(/\/en\/?$/);
		expect(await storedSession(page)).toEqual({ accounts: null, index: null });
	});
});
