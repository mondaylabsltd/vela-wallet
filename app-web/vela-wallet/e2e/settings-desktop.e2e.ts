/**
 * The wide settings layout, live (spec 072).
 *
 * The phone layout was the one the settings suites spoke to, and the wide one
 * shipped with two data-loss bugs nobody exercised: its providers panel never
 * told the core it had opened, so tabbing out of a key field saved an empty
 * key over the saved one; and a storage row's Clear removed the whole address
 * book on one click. Its erase was drawn and did nothing.
 *
 * Asserted the honest way — through the stores, and across a reload.
 */
import { expect, test, type Page } from '@playwright/test';
import { en, seedSignedIn } from './live-helpers';
import { denyOffOrigin } from './stub-chain';

// Past the desktop breakpoint (1280): the three-column settings.
test.use({ viewport: { width: 1440, height: 900 } });

test.beforeEach(async ({ page }) => {
	// A key test probes the provider's endpoints; none of them is reached here.
	await denyOffOrigin(page);
	await seedSignedIn(page);
});

async function openSettings(page: Page): Promise<void> {
	await page.goto('/en/settings');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
}

async function openPanel(page: Page, label: string): Promise<void> {
	await page.locator('nav.settings-nav').getByRole('button', { name: label }).click();
}

/** One record of the IndexedDB KV, opened the way `services/storage.ts` opens it. */
function kvGet(page: Page, key: string): Promise<string | null> {
	return page.evaluate(
		(k) =>
			new Promise<string | null>((resolve, reject) => {
				const open = indexedDB.open('vela', 1);
				open.onupgradeneeded = () => open.result.createObjectStore('kv');
				open.onerror = () => reject(open.error);
				open.onsuccess = () => {
					const request = open.result.transaction('kv', 'readonly').objectStore('kv').get(k);
					request.onsuccess = () => resolve((request.result as string | undefined) ?? null);
					request.onerror = () => reject(request.error);
				};
			}),
		key
	);
}

function kvPut(page: Page, key: string, value: string): Promise<void> {
	return page.evaluate(
		([k, v]) =>
			new Promise<void>((resolve, reject) => {
				const open = indexedDB.open('vela', 1);
				open.onupgradeneeded = () => open.result.createObjectStore('kv');
				open.onerror = () => reject(open.error);
				open.onsuccess = () => {
					const tx = open.result.transaction('kv', 'readwrite');
					tx.objectStore('kv').put(v, k);
					tx.oncomplete = () => resolve();
					tx.onerror = () => reject(tx.error);
				};
			}),
		[key, value] as const
	);
}

test('a saved provider key survives tabbing through the key fields (P0)', async ({ page }) => {
	await openSettings(page);
	await openPanel(page, en('settings.advanced.rpcProvidersTitle'));
	const alchemy = page.locator('section.provider', { hasText: 'Alchemy' }).locator('input');
	await alchemy.fill('alch-e2e-key');
	await alchemy.blur();
	await expect.poll(() => kvGet(page, 'vela.rpcProviders')).toContain('alch-e2e-key');

	// A new visit: the panel shows the saved key. Walk through every field on
	// it without typing — which is all it took to lose the key.
	await page.reload();
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await openPanel(page, en('settings.advanced.rpcProvidersTitle'));
	await expect(alchemy).toHaveValue('alch-e2e-key');
	await alchemy.focus();
	for (let i = 0; i < 6; i += 1) await page.keyboard.press('Tab');

	await page.reload();
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await openPanel(page, en('settings.advanced.rpcProvidersTitle'));
	await expect(alchemy).toHaveValue('alch-e2e-key');
	expect(JSON.parse((await kvGet(page, 'vela.rpcProviders')) ?? '{}')).toEqual({
		alchemy: 'alch-e2e-key'
	});
});

test('"Get key" is a link to that provider’s key page', async ({ page }) => {
	await openSettings(page);
	await openPanel(page, en('settings.advanced.rpcProvidersTitle'));
	const drpc = page.locator('section.provider', { hasText: 'dRPC' });
	await expect(drpc.locator('a.action')).toHaveAttribute('href', 'https://drpc.org/');
	await expect(drpc.locator('a.action')).toHaveText(en('settingsModals.rpcProviders.getKey'));
});

test('a storage row asks first, names itself, and clears only on confirm', async ({ page }) => {
	await openSettings(page);
	const book = JSON.stringify([
		{ address: '0x0000000000000000000000000000000000000001', name: 'A' }
	]);
	await kvPut(page, 'vela.contacts', book);
	await openPanel(page, en('settings.storage.title'));

	const row = page.locator('li', { hasText: en('settings.storage.itemContacts') });
	const question = page.getByRole('dialog', { name: en('settings.storage.itemContacts') });

	// Cancelled: nothing goes.
	await row.getByRole('button').click();
	await expect(question).toBeVisible();
	await question.getByRole('button', { name: en('common.cancel') }).click();
	await expect(question).toBeHidden();
	expect(await kvGet(page, 'vela.contacts')).toBe(book);

	// Confirmed: the address book goes, and only it.
	await row.getByRole('button').click();
	await question.getByRole('button', { name: en('settings.storage.clear') }).click();
	await expect(question).toBeHidden();
	await expect.poll(() => kvGet(page, 'vela.contacts')).toBeNull();
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
});

test('erase asks the phone’s question, and a confirm leaves nothing behind', async ({ page }) => {
	await openSettings(page);
	await page.evaluate(() => localStorage.setItem('vela.somethingNobodyHasWrittenYet', 'x'));
	await kvPut(page, 'vela.contacts', '[]');

	await page.getByRole('button', { name: en('settings.eraseDevice.title') }).click();
	const question = page.getByRole('dialog', { name: en('settings.eraseDevice.title') });
	await expect(question).toContainText(en('settings.eraseDevice.desc'));
	await question.getByRole('button', { name: en('settings.eraseDevice.confirm') }).click();

	// First run: the guard sends a browser with no wallet back to Welcome.
	await page.waitForURL(/\/en$/, { timeout: 20_000 });
	expect(await kvGet(page, 'vela.contacts')).toBeNull();
	expect(
		await page.evaluate(() => localStorage.getItem('vela.somethingNobodyHasWrittenYet'))
	).toBeNull();
});
