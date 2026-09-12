/**
 * Spec 048: a wallet the retired client stored at this origin — camelCase
 * records — still opens; an unreadable list says so instead of hanging.
 */
import { expect, test } from '@playwright/test';

test.use({ viewport: { width: 390, height: 844 } });

const EXPO_ACCOUNT = {
	id: 'e2e-credential-id',
	name: 'Old Client Wallet',
	address: '0x14fb1f4e2b9c7a5d8e3f6a1b4c7d9e2f5a8b1d1e',
	publicKeyHex: '04' + 'ab'.repeat(64),
	createdAt: '2026-01-01T00:00:00.000Z',
	keys: [{ credentialId: 'e2e-credential-id', publicKeyHex: '04' + 'ab'.repeat(64), name: 'Old Client Wallet' }]
};

test('a wallet stored by the retired client opens', async ({ page }) => {
	await page.addInitScript((account) => {
		window.localStorage.setItem('vela.intro.seen', String(Date.now()));
		if (window.localStorage.getItem('vela.accounts') === null) {
			window.localStorage.setItem('vela.accounts', JSON.stringify([account]));
			window.localStorage.setItem('vela.activeAccountIndex', '0');
		}
	}, EXPO_ACCOUNT);
	await page.goto('/en/wallet');
	await expect(page.getByText('Old Client Wallet').first()).toBeVisible({ timeout: 15_000 });
	await expect(page).toHaveURL(/\/en\/wallet$/);
	const rewritten = await page.evaluate(() => JSON.parse(window.localStorage.getItem('vela.accounts') ?? '[]'));
	expect(rewritten[0].public_key_hex).toBeDefined();
	expect(rewritten[0].publicKeyHex).toBeUndefined();
});

test('an unreadable account list says so instead of hanging', async ({ page }) => {
	await page.addInitScript(() => {
		window.localStorage.setItem('vela.intro.seen', String(Date.now()));
		window.localStorage.setItem('vela.accounts', '[{"id":"x","address":"0x1","keys":[{"credential_id":7}]}]');
		window.localStorage.setItem('vela.activeAccountIndex', '0');
	});
	await page.goto('/en/wallet');
	await expect(page).toHaveURL(/\/en\/?$/, { timeout: 15_000 });
	await expect(page.getByRole('alertdialog').or(page.getByRole('dialog'))).toBeVisible({ timeout: 5_000 });
});
