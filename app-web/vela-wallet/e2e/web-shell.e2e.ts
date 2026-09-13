/**
 * The web shell at both widths — what the founder saw broken on 2026-09-05,
 * pinned so it stays fixed:
 *
 * - 通讯录 drawn as a phone at 1440px, while /wallet and /settings had their
 *   three columns (spec 018's DC1 exists; the route never reached for it).
 * - 探索 back in the sidebar and the tab bar on 通讯录 and 设置. The web has
 *   no dApp browser (spec 022): one list, `WEB_DESTINATIONS`, every route.
 * - The wallet a strip down the left of the screen: a flex item given no
 *   `flex` is as wide as its content, and the gallery stage — a block — had
 *   hidden that.
 *
 * Chromium only: nothing here is about storage.
 */
import { expect, test } from '@playwright/test';
import { en, seedSignedIn } from './live-helpers';

const ROUTES = ['/en/wallet', '/en/contacts', '/en/settings'];

test.beforeEach(async ({ page }) => {
	await seedSignedIn(page);
});

test.describe('at desktop width', () => {
	test.use({ viewport: { width: 1440, height: 900 } });

	test('contacts is three columns, not a phone', async ({ page }) => {
		await page.goto('/en/contacts');
		await expect(page.getByText('E2E Wallet').first()).toBeVisible();
		// The app sidebar, full height, and the group rail's 全部 row: DC1.
		const sidebar = await page.locator('aside.sidebar').boundingBox();
		expect(sidebar?.height).toBe(900);
		await expect(page.getByText(en('contacts.allContacts'), { exact: true })).toBeVisible();
		// The phone's tab bar is not here.
		await expect(page.locator('nav.tabbar')).toHaveCount(0);
	});

	test('adding a contact opens the third column, not a phone sheet', async ({ page }) => {
		await page.goto('/en/contacts');
		await expect(page.getByText('E2E Wallet').first()).toBeVisible();
		await page
			.getByRole('button', { name: en('contacts.addContact') })
			.first()
			.click();
		// The form is the column beside the list — the desktop's rule for
		// every phone sheet — and the sheet itself is nowhere.
		const column = page.locator('aside.panel');
		await expect(column.getByRole('heading', { name: en('contacts.addTitle') })).toBeVisible();
		await expect(column.getByRole('button', { name: en('contacts.save') })).toBeDisabled();
		await expect(page.locator('.sheet')).toHaveCount(0);
	});

	test('the sidebar names three destinations on every route, never 探索', async ({ page }) => {
		const expected = [
			en('componentsUi.mainNav.wallet'),
			en('componentsUi.mainNav.contacts'),
			en('componentsUi.mainNav.settings')
		];
		for (const route of ROUTES) {
			await page.goto(route);
			await expect(page.getByText('E2E Wallet').first()).toBeVisible();
			await expect(page.locator('aside.sidebar nav button')).toHaveText(expected);
		}
	});

	test('the wallet takes the whole width beside its sidebar', async ({ page }) => {
		await page.goto('/en/wallet');
		await expect(page.getByText('E2E Wallet').first()).toBeVisible();
		// The wallet's own root spans the viewport; its main column is what is
		// left after the sidebar — not the width of a skeleton and two headings.
		const root = await page.locator('aside.sidebar').locator('..').boundingBox();
		expect(root?.width).toBe(1440);
		const main = await page.locator('aside.sidebar + main').boundingBox();
		expect(main?.width ?? 0).toBeGreaterThan(1000);
	});
});

test.describe('at phone width', () => {
	test.use({ viewport: { width: 390, height: 844 } });

	test('the tab bar has three tabs on every route, never 探索', async ({ page }) => {
		for (const route of ROUTES) {
			await page.goto(route);
			// The phone's contacts page has no identity header; the tab bar is
			// what every route draws once it has ruled.
			await expect(page.locator('nav.tabbar')).toBeVisible();
			await expect(page.locator('nav.tabbar button')).toHaveCount(3);
			await expect(page.locator('nav.tabbar')).not.toContainText(
				en('componentsUi.mainNav.explore')
			);
		}
	});
});
