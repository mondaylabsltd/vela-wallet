/**
 * The account switcher, live (spec 028 Phase 8).
 *
 * Until this phase the settings sheet listed fixture accounts and a tap did
 * nothing: `identity.ts` swapped only the active row's name. Now the rows are
 * the session's own, a tap is `SwitchAccount` in the session's domain, and
 * the choice survives a reload because the core persisted the index.
 *
 * Two accounts are seeded straight into the stored shape; the addresses the
 * app shows are DERIVED from the keys (spec 019 invariant ②), so the tests
 * assert by name.
 */
import { expect, test, type Page } from '@playwright/test';

test.use({ viewport: { width: 390, height: 844 } });

const FIRST = {
	id: 'e2e-credential-one',
	name: 'First Wallet',
	address: '0x14fb1f4e2b9c7a5d8e3f6a1b4c7d9e2f5a8b1d1e',
	public_key_hex: '04' + 'ab'.repeat(64),
	created_at_iso: '2026-01-01T00:00:00.000Z',
	keys: []
};
const SECOND = {
	id: 'e2e-credential-two',
	name: 'Second Wallet',
	address: '0x24fb1f4e2b9c7a5d8e3f6a1b4c7d9e2f5a8b1d2e',
	public_key_hex: '04' + 'cd'.repeat(64),
	created_at_iso: '2026-01-02T00:00:00.000Z',
	keys: []
};

async function seedTwoAccounts(page: Page): Promise<void> {
	await page.addInitScript(
		([first, second]) => {
			window.localStorage.setItem('vela.intro.seen', String(Date.now()));
			if (window.localStorage.getItem('vela.accounts') === null) {
				window.localStorage.setItem('vela.accounts', JSON.stringify([first, second]));
				window.localStorage.setItem('vela.activeAccountIndex', '0');
			}
		},
		[FIRST, SECOND] as const
	);
}

test.beforeEach(async ({ page }) => {
	await seedTwoAccounts(page);
});

test('the sheet lists every signed-in account, and a tap switches — durably', async ({ page }) => {
	await page.goto('/en/settings');
	await expect(page.getByText('First Wallet').first()).toBeVisible();

	// The account row opens the sheet; both rows are there, the active one marked.
	await page
		.getByRole('button', { name: /First Wallet/ })
		.first()
		.click();
	const sheet = page.getByRole('dialog');
	await expect(sheet.getByText('Second Wallet')).toBeVisible();
	await expect(sheet.getByRole('button', { name: /First Wallet/ })).toHaveAttribute(
		'aria-current',
		'true'
	);

	await sheet.getByRole('button', { name: /Second Wallet/ }).click();

	// The header now names the second account, and the sheet is gone.
	await expect(page.getByRole('dialog')).toHaveCount(0);
	await expect(page.getByText('Second Wallet').first()).toBeVisible();
	await expect(page.getByText('First Wallet')).toHaveCount(0);

	// The core persisted the index: a reload lands on the same account.
	await page.reload();
	await expect(page.getByText('Second Wallet').first()).toBeVisible();
	await expect(page.getByText('First Wallet')).toHaveCount(0);

	// And the wallet is that account's too — one session, every route.
	await page.goto('/en/wallet');
	await expect(page.getByText('Second Wallet').first()).toBeVisible();
});

test('the two buttons leave for the create and sign-in journeys', async ({ page }) => {
	await page.goto('/en/settings');
	await page
		.getByRole('button', { name: /First Wallet/ })
		.first()
		.click();
	const sheet = page.getByRole('dialog');
	await sheet.getByRole('button', { name: 'Create New Account' }).click();
	await expect(page).toHaveURL(/\/en\/create/);

	await page.goto('/en/settings');
	await page
		.getByRole('button', { name: /First Wallet/ })
		.first()
		.click();
	await page.getByRole('dialog').getByRole('button', { name: 'Sign In with Existing' }).click();
	await expect(page).toHaveURL(/\/en\/?$/);
});

/**
 * The header's name opens the same switcher (founder call, 2026-09-05): the
 * chevron beside the name drew a disclosure that led nowhere, and the only
 * switcher was three taps away on the settings screen.
 */
test('the wallet header opens the switcher, and a pick switches', async ({ page }) => {
	await page.goto('/en/wallet');
	await expect(page.getByText('First Wallet').first()).toBeVisible();

	await page.getByRole('button', { name: /First Wallet/ }).click();
	const sheet = page.getByRole('dialog');
	await expect(sheet.getByText('Second Wallet')).toBeVisible();
	await expect(sheet.getByRole('button', { name: /First Wallet/ })).toHaveAttribute(
		'aria-current',
		'true'
	);

	await sheet.getByRole('button', { name: /Second Wallet/ }).click();
	await expect(page.getByRole('dialog')).toHaveCount(0);
	await expect(page.getByText('Second Wallet').first()).toBeVisible();
	await expect(page.getByText('First Wallet')).toHaveCount(0);
});

test.describe('on the wide layout', () => {
	test.use({ viewport: { width: 1280, height: 900 } });

	test('the sidebar header opens the switcher as a dialog, with the two ways out', async ({
		page
	}) => {
		await page.goto('/en/wallet');
		await expect(page.getByText('First Wallet').first()).toBeVisible();

		await page.getByRole('button', { name: /First Wallet/ }).click();
		const dialog = page.getByRole('dialog');
		await expect(dialog.getByText('Second Wallet')).toBeVisible();
		await dialog.getByRole('button', { name: 'Sign In with Existing' }).click();
		await expect(page).toHaveURL(/\/en\/?$/);

		await page.goto('/en/wallet');
		await page.getByRole('button', { name: /First Wallet/ }).click();
		await page.getByRole('dialog').getByRole('button', { name: 'Create New Account' }).click();
		await expect(page).toHaveURL(/\/en\/create/);
	});
});

/**
 * Two records, one address — the switcher must survive it.
 *
 * Signing in with a passkey this device is ALREADY signed in with appends a
 * second account (the core's `AddAccount` appends without deduping), and both
 * records derive the same address. The switcher used to key its rows by the
 * truncated display address, so Svelte threw `each_key_duplicate` and the
 * settings screen died where a person had done nothing stranger than sign in
 * twice (founder-reported, 2026-09-16).
 *
 * Position is the row's identity here — it is what `onselect` already sends —
 * so the pair renders AND the second one is still selectable as its own row,
 * which an address key could not have managed even without throwing.
 */
test.describe('two records deriving one address', () => {
	test.use({ viewport: { width: 1280, height: 900 } });

	const TWIN = {
		id: 'e2e-same-credential',
		name: 'Twice Signed In',
		address: '0x34fb1f4e2b9c7a5d8e3f6a1b4c7d9e2f5a8b1d3e',
		public_key_hex: '04' + 'ef'.repeat(64),
		created_at_iso: '2026-01-03T00:00:00.000Z',
		keys: []
	};

	test('renders both rows, throws nothing, and switches by position', async ({ page }) => {
		const pageErrors: string[] = [];
		page.on('pageerror', (error) => pageErrors.push(error.message));

		await page.addInitScript((twin) => {
			window.localStorage.setItem('vela.accounts', JSON.stringify([twin, twin]));
			window.localStorage.setItem('vela.activeAccountIndex', '0');
		}, TWIN);

		await page.goto('/en/settings');
		await page.getByRole('button', { name: 'Account', exact: true }).click();

		// Scoped to the panel: the sidebar header names the active account too.
		const rows = page.getByRole('main').getByRole('button', { name: /Twice Signed In/ });
		await expect(rows).toHaveCount(2);
		expect(pageErrors.filter((message) => /each_key_duplicate/.test(message))).toEqual([]);

		// The second of the pair is its own row, and the core persists ITS index.
		await rows.nth(1).click();
		await expect
			.poll(() => page.evaluate(() => window.localStorage.getItem('vela.activeAccountIndex')))
			.toBe('1');
		expect(pageErrors).toEqual([]);
	});
});
