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
 * One wallet, one row (the session core's rule ⑨).
 *
 * A wallet is its ADDRESS. Signing in with a passkey this device is already
 * signed in with used to append a second record deriving the same address,
 * and the switcher — keyed by the truncated display address — threw
 * `each_key_duplicate` and took the settings screen down (founder-reported,
 * 2026-09-16). This test then held that the PAIR rendered and each of the two
 * was selectable by position.
 *
 * The core no longer keeps the pair: an establishment whose address the list
 * already holds activates that row instead of appending one, and a restore
 * collapses a pair already on disk — with the saved index following its
 * wallet, not its old slot. So this holds the core's rule: a profile that
 * still has the pair on disk shows the wallet ONCE, nothing throws, and the
 * wallet that was active is still the active one.
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
	const OTHER = {
		id: 'e2e-other-credential',
		name: 'Another Wallet',
		address: '0x54fb1f4e2b9c7a5d8e3f6a1b4c7d9e2f5a8b1d5e',
		public_key_hex: '04' + 'cd'.repeat(64),
		created_at_iso: '2026-01-02T00:00:00.000Z',
		keys: []
	};

	test('a pair on disk is one row, and nothing throws', async ({ page }) => {
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
		await expect(rows).toHaveCount(1);
		// One wallet is counted as one: "1 account · ", in the singular.
		await expect(page.getByRole('main').getByText(/^1 account · /)).toBeVisible();
		expect(pageErrors).toEqual([]);
	});

	test('the saved index follows its wallet when the pair collapses', async ({ page }) => {
		const pageErrors: string[] = [];
		page.on('pageerror', (error) => pageErrors.push(error.message));

		// The second of the pair was the active one: slot 2 of three.
		await page.addInitScript(
			([other, twin]) => {
				window.localStorage.setItem('vela.accounts', JSON.stringify([other, twin, twin]));
				window.localStorage.setItem('vela.activeAccountIndex', '2');
			},
			[OTHER, TWIN] as const
		);

		await page.goto('/en/settings');
		// Still that wallet — not whatever now sits in its old slot, and not
		// the first of the list.
		await expect(page.getByText('Twice Signed In').first()).toBeVisible();
		await page.getByRole('button', { name: 'Account', exact: true }).click();
		const main = page.getByRole('main');
		await expect(main.getByRole('button', { name: /Twice Signed In/ })).toHaveCount(1);
		await expect(main.getByRole('button', { name: /Another Wallet/ })).toHaveCount(1);
		await expect(main.getByText(/^2 accounts · /)).toBeVisible();

		// The other wallet is its own row, and choosing it is persisted.
		await main.getByRole('button', { name: /Another Wallet/ }).click();
		await expect
			.poll(() => page.evaluate(() => window.localStorage.getItem('vela.activeAccountIndex')))
			.toBe('0');
		expect(pageErrors).toEqual([]);
	});
});
