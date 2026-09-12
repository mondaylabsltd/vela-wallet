/**
 * The identicon viewer, from every artwork (founder call, 2026-09-05).
 *
 * Until now only the header's avatar opened it. Now the artwork is a button
 * wherever it is drawn — the header, the settings account row, the rows of
 * the account switcher — and every one of them opens the same viewer on the
 * address that drew it, in full.
 */
import { expect, test } from '@playwright/test';
import { en, seedSignedIn, TEST_ACCOUNT_ADDRESS } from './live-helpers';

test.use({ viewport: { width: 390, height: 844 } });

const OPEN = en('componentsUi.identiconViewer.a11yOpen');
const TITLE = en('componentsUi.identiconViewer.title');
const CLOSE = en('componentsUi.identiconViewer.close');

test.beforeEach(async ({ page }) => {
	await seedSignedIn(page);
});

test('the header artwork opens the viewer on the whole address', async ({ page }) => {
	await page.goto('/en/wallet');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();

	await page.getByRole('button', { name: OPEN }).first().click();
	const viewer = page.getByRole('dialog', { name: TITLE });
	await expect(viewer.getByText(TEST_ACCOUNT_ADDRESS)).toBeVisible();

	await viewer.getByRole('button', { name: CLOSE }).click();
	await expect(page.getByRole('dialog', { name: TITLE })).toHaveCount(0);
});

test('the settings account row has its own artwork button', async ({ page }) => {
	await page.goto('/en/settings');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();

	await page.getByRole('button', { name: OPEN }).first().click();
	await expect(
		page.getByRole('dialog', { name: TITLE }).getByText(TEST_ACCOUNT_ADDRESS)
	).toBeVisible();
});

test('the switcher rows open the viewer over the switcher', async ({ page }) => {
	await page.goto('/en/wallet');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();

	await page.getByRole('button', { name: /E2E Wallet/ }).click();
	const switcher = page.getByRole('dialog').first();
	await switcher.getByRole('button', { name: OPEN }).first().click();

	const viewer = page.getByRole('dialog', { name: TITLE });
	await expect(viewer.getByText(TEST_ACCOUNT_ADDRESS)).toBeVisible();
});

test.describe('on the wide layout', () => {
	test.use({ viewport: { width: 1280, height: 900 } });

	test('the viewer is a card, not a page', async ({ page }) => {
		await page.goto('/en/wallet');
		await expect(page.getByText('E2E Wallet').first()).toBeVisible();

		await page.getByRole('button', { name: OPEN }).first().click();
		const viewer = page.getByRole('dialog', { name: TITLE });
		await expect(viewer.getByText(TEST_ACCOUNT_ADDRESS)).toBeVisible();

		// `--layout-promptCard` was never declared until 2026-09-05, so the card
		// read its max-width as invalid and spanned the whole window — the
		// founder's "太长了". A card is narrower than half of this viewport.
		const card = viewer.locator('.card');
		const box = await card.boundingBox();
		expect(box).not.toBeNull();
		expect(box!.width).toBeLessThanOrEqual(440);
		expect(box!.width).toBeGreaterThan(300);
	});
});
