/**
 * Spec 048: a wallet the retired client stored at this origin — camelCase
 * records — still opens; an unreadable list says so instead of hanging.
 * §18 item 2: what cannot be read is kept as stored, and one record that
 * cannot be read beside a good one opens the good one and is said under the
 * account switcher's list.
 */
import { expect, test } from '@playwright/test';

test.use({ viewport: { width: 390, height: 844 } });

const EXPO_ACCOUNT = {
	id: 'e2e-credential-id',
	name: 'Old Client Wallet',
	address: '0x14fb1f4e2b9c7a5d8e3f6a1b4c7d9e2f5a8b1d1e',
	publicKeyHex: '04' + 'ab'.repeat(64),
	createdAt: '2026-01-01T00:00:00.000Z',
	keys: [
		{
			credentialId: 'e2e-credential-id',
			publicKeyHex: '04' + 'ab'.repeat(64),
			name: 'Old Client Wallet'
		}
	]
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
	// §18 item 2: the shell's read writes nothing back; the session's own save
	// (the core migrating the record) rewrites it in the current spelling,
	// through the core's `stored_accounts` upsert, by id.
	await expect
		.poll(async () =>
			page.evaluate(() => {
				const list = JSON.parse(window.localStorage.getItem('vela.accounts') ?? '[]');
				return [list.length, 'public_key_hex' in list[0], 'publicKeyHex' in list[0]];
			})
		)
		.toEqual([1, true, false]);
});

test('an unreadable account list says so instead of hanging', async ({ page }) => {
	await page.addInitScript(() => {
		window.localStorage.setItem('vela.intro.seen', String(Date.now()));
		window.localStorage.setItem(
			'vela.accounts',
			'[{"id":"x","address":"0x1","keys":[{"credential_id":7}]}]'
		);
		window.localStorage.setItem('vela.activeAccountIndex', '0');
	});
	await page.goto('/en/wallet');
	await expect(page).toHaveURL(/\/en\/?$/, { timeout: 15_000 });
	await expect(page.getByRole('alertdialog').or(page.getByRole('dialog'))).toBeVisible({
		timeout: 5_000
	});
	// Kept as stored, byte for byte.
	expect(await page.evaluate(() => window.localStorage.getItem('vela.accounts'))).toBe(
		'[{"id":"x","address":"0x1","keys":[{"credential_id":7}]}]'
	);
});

test('one record it cannot read opens the others and is said under the switcher', async ({
	page
}) => {
	const bad = '{"id":"x","address":"0x1","keys":[{"credential_id":7}],"note":"a\\/b"}';
	await page.addInitScript(
		([account, bad]) => {
			window.localStorage.setItem('vela.intro.seen', String(Date.now()));
			if (window.localStorage.getItem('vela.accounts') === null) {
				window.localStorage.setItem('vela.accounts', `[${bad},${JSON.stringify(account)}]`);
				window.localStorage.setItem('vela.activeAccountIndex', '0');
			}
		},
		[EXPO_ACCOUNT, bad] as const
	);
	await page.goto('/en/wallet');
	await expect(page.getByText('Old Client Wallet').first()).toBeVisible({ timeout: 15_000 });
	await page
		.getByRole('button', { name: /Old Client Wallet/ })
		.first()
		.click();
	await expect(page.getByRole('dialog').getByTestId('accounts-unreadable')).toBeVisible();
	const stored = await page.evaluate(() => window.localStorage.getItem('vela.accounts'));
	expect(stored).toContain(bad);
});
