/**
 * The wide settings layout, live (spec 072).
 *
 * The phone layout was the one the settings suites spoke to, and the wide one
 * shipped with data-loss bugs nobody exercised: a storage row's Clear removed
 * the whole address book on one click, and its erase was drawn and did
 * nothing. (Its RPC providers panel lost saved keys too; the page itself is
 * gone since 2026-10-29 — saved keys become per-network RPCs, below.)
 *
 * Asserted the honest way — through the stores, and across a reload.
 */
import { expect, test, type Page } from '@playwright/test';
import { en, seedSignedIn } from './live-helpers';
import { denyOffOrigin } from './stub-chain';

// Past the desktop breakpoint (1280): the three-column settings.
test.use({ viewport: { width: 1440, height: 900 } });

test.beforeEach(async ({ page }) => {
	// The network cards probe their endpoints; none of them is reached here.
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

/**
 * One record of the IndexedDB KV, opened the way `services/storage.ts` opens
 * it — and closed once read: the erase deletes this database, and a
 * connection left open holds that delete back.
 */
function kvGet(page: Page, key: string): Promise<string | null> {
	return page.evaluate(
		(k) =>
			new Promise<string | null>((resolve, reject) => {
				const open = indexedDB.open('vela', 1);
				open.onupgradeneeded = () => open.result.createObjectStore('kv');
				open.onerror = () => reject(open.error);
				open.onsuccess = () => {
					const db = open.result;
					const request = db.transaction('kv', 'readonly').objectStore('kv').get(k);
					request.onsuccess = () => {
						db.close();
						resolve((request.result as string | undefined) ?? null);
					};
					request.onerror = () => {
						db.close();
						reject(request.error);
					};
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
					const db = open.result;
					const tx = db.transaction('kv', 'readwrite');
					tx.objectStore('kv').put(v, k);
					tx.oncomplete = () => {
						db.close();
						resolve();
					};
					tx.onerror = () => {
						db.close();
						reject(tx.error);
					};
				};
			}),
		[key, value] as const
	);
}

test('saved RPC provider keys become the networks’ own RPCs, once (2026-10-29)', async ({
	page
}) => {
	await openSettings(page);
	// A wallet from before: an Alchemy key, and Ethereum on a node of its own.
	// The node first, the key last: the page seeded here may still be booting
	// its network ledger, and a ledger that reads the key must read the node
	// with it — a key seen over an empty record was retired over it, then
	// the record's seed overwrote what the retirement wrote (webkit, 2 in 8).
	await kvPut(
		page,
		'vela.networkConfig',
		JSON.stringify([{ chainId: 1, rpcURL: 'https://my-node.example' }])
	);
	await kvPut(page, 'vela.rpcProviders', JSON.stringify({ alchemy: 'fake-e2e-key' }));
	await page.reload();
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();

	await expect.poll(() => kvGet(page, 'vela.rpcProviders')).toBeNull();
	const configs = JSON.parse((await kvGet(page, 'vela.networkConfig')) ?? '[]') as {
		chainId: number;
		rpcURL: string;
	}[];
	expect(configs.find((c) => c.chainId === 1)?.rpcURL).toBe('https://my-node.example');
	expect(configs.find((c) => c.chainId === 8453)?.rpcURL).toBe(
		'https://base-mainnet.g.alchemy.com/v2/fake-e2e-key'
	);
	// And the page that held them is gone from the column.
	await expect(
		page.locator('nav.settings-nav').getByRole('button', {
			name: en('settings.advanced.rpcProvidersTitle')
		})
	).toHaveCount(0);
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
