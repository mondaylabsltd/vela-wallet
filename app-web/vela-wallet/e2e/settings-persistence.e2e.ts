/**
 * Settings on the core, durably (spec 024 T040 — SC-001).
 *
 * Persistence is asserted the honest way: change something, `reload()`, and
 * read it back off the screen. Assertions speak the EN corpus.
 */
import { expect, test, type Page } from '@playwright/test';
import { en, seedSignedIn } from './live-helpers';

// The live wiring's phase-3/4 scope is the mobile layout (desktop
// interactivity is a recorded debt) — so the suites speak to it.
test.use({ viewport: { width: 390, height: 844 } });

test.beforeEach(async ({ page }) => {
	await seedSignedIn(page);
});

async function openSettings(page: Page): Promise<void> {
	await page.goto('/en/settings');
	// The account block renders once the session core has ruled.
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
}

/** The 高级 group ships collapsed; its rows need the disclosure open. */
async function openAdvanced(page: Page): Promise<void> {
	await page.getByText(en('settings.sections.advanced'), { exact: true }).click();
}

test('the display currency survives a reload (core-persisted, not staged)', async ({ page }) => {
	await openSettings(page);
	await page.getByText(en('settings.localization.currencyTitle'), { exact: true }).click();
	// The sheet lists codes; EUR is one of the canon rows.
	await page.getByText('EUR', { exact: true }).click();
	// The row now carries the committed code…
	await expect(page.getByText('EUR', { exact: true })).toBeVisible();
	// …and still does after the page forgets everything it did not persist.
	await page.reload();
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await expect(page.getByText('EUR', { exact: true })).toBeVisible();
});

test('a service-endpoint override survives a reload', async ({ page }) => {
	await openSettings(page);
	await openAdvanced(page);
	await page.getByText(en('settings.advanced.endpointsTitle'), { exact: true }).click();

	const field = page.getByLabel(en('settingsModals.endpoints.chainDataLabel'));
	await expect(field).toBeVisible();
	await field.fill('https://data.example.test');
	await field.blur();

	await page.reload();
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await openAdvanced(page);
	await page.getByText(en('settings.advanced.endpointsTitle'), { exact: true }).click();
	await expect(page.getByLabel(en('settingsModals.endpoints.chainDataLabel'))).toHaveValue(
		'https://data.example.test'
	);
});

test('the networks page renders the CORE registry, not the fixture list', async ({ page }) => {
	await openSettings(page);
	await openAdvanced(page);
	await page.getByText(en('settings.advanced.networksTitle'), { exact: true }).click();
	// The builtin registry's Ethereum row, worded from the corpus template.
	await expect(
		page.getByText(en('settingsModals.network.chainId').replace('{{chainId}}', '1'), {
			exact: true
		})
	).toBeVisible();
	// The fixture list's staged custom network must NOT appear: the fixture
	// canon has a custom row, and a custom row is the one with a remove
	// button; a fresh store has no customs. (Not by name: X Layer, the
	// fixture's custom, is a built-in network in the registry now.)
	await expect(
		page.getByRole('button', { name: en('settingsModals.network.removeTitle'), exact: true })
	).toHaveCount(0);
});

test('a fresh profile is sent back to Welcome (the guard holds)', async ({ page }) => {
	// No seeding beyond the intro: this profile has no wallet.
	await page.addInitScript(() => window.localStorage.clear());
	await page.addInitScript(() =>
		window.localStorage.setItem('vela.intro.seen', String(Date.now()))
	);
	await page.goto('/en/settings');
	await expect(page).toHaveURL('/en');
});

test('the live routes load the SAME fingerprinted core artifact as the rest of the app', async ({
	page
}) => {
	const wasmUrls = new Set<string>();
	page.on('request', (request) => {
		if (request.url().endsWith('.wasm')) wasmUrls.add(request.url());
	});
	await openSettings(page);
	await page.goto('/en/contacts');
	await expect(page.getByRole('heading', { name: en('contacts.title') }).first()).toBeVisible();
	// One artifact, not a per-feature zoo — wiring more machines costs 0 bytes.
	expect([...wasmUrls].length).toBe(1);
	expect([...wasmUrls][0]).toMatch(/vela_core_bg\.[0-9a-f]+\.wasm$/);
});
