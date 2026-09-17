/**
 * Issue 191 — a contact history suggested "cannot be named", and shows its
 * address where a name goes.
 *
 * Two halves, both run against the SHIPPED wasm in a real browser:
 *
 * 1. **What a row is called.** The person's own name → a resolved identity (a
 *    name service, or the name a Vela user registered with the passkey index)
 *    → and only then the address. A history row used to stop at the name
 *    captured at the moment of the send: nothing asked about it until its
 *    detail was opened, and what that learned was written back to SAVED
 *    contacts only. The core now asks about the book's unnamed rows as it
 *    loads, and a row with no stored record adopts the answer.
 * 2. **How it gets a name.** Naming always worked — through a muted "Edit" at
 *    the far end of the desktop's third column, some 780px below the address
 *    standing in for the name. The action now sits under that name and says
 *    what it does.
 *
 * Hermetic: every off-origin request is denied; the index answers from here.
 */
import { expect, test, type Page } from '@playwright/test';
import { en, seedSignedIn, TEST_ACCOUNT_ADDRESS } from './live-helpers';
import { denyOffOrigin, readKv } from './stub-chain';

/** The index knows this one by name. */
const KNOWN = '0x' + 'b0b0'.repeat(10);
/** Nobody knows this one — the reporter's `0x600746…95f4d4`. */
const STRANGER = '0x600746' + '0'.repeat(28) + '95f4d4';
const STRANGER_SHORT = '0x600746…95f4d4';
const REGISTRY_NAME = "Bob's Vela";

function sendTo(to: string, id: string, ageSeconds: number) {
	return {
		id,
		userOpHash: '0x' + id.padEnd(64, '1').slice(0, 64),
		txHash: '0x' + id.padEnd(64, '2').slice(0, 64),
		from: TEST_ACCOUNT_ADDRESS,
		to,
		value: '0.001',
		symbol: 'XDAI',
		decimals: 18,
		chainId: 100,
		timestamp: Math.floor(Date.now() / 1000) - ageSeconds,
		status: 'confirmed',
		type: 'send'
	};
}

/** Two confirmed sends in local history, and nothing in the address book. */
async function seedHistory(page: Page): Promise<void> {
	await page.addInitScript(
		(records) => {
			// Once: this runs before every document, and a reload must find what
			// the app wrote, not a fresh seed.
			if (window.localStorage.getItem('e2e.191.seeded') !== null) return;
			window.localStorage.setItem('e2e.191.seeded', '1');
			const open = indexedDB.open('vela', 1);
			open.onupgradeneeded = () => open.result.createObjectStore('kv');
			open.onsuccess = () => {
				open.result
					.transaction('kv', 'readwrite')
					.objectStore('kv')
					.put(JSON.stringify(records), 'vela.transactionHistory');
			};
		},
		[sendTo(KNOWN, 'aa', 3600), sendTo(STRANGER, 'bb', 7200)]
	);
}

/** The passkey index, by `walletRef` (the address left-padded to 32 bytes). */
async function stubIndex(page: Page): Promise<string[]> {
	const asked: string[] = [];
	await page.route(/\/api\/query\?walletRef=/, (route) => {
		const ref = new URL(route.request().url()).searchParams.get('walletRef') ?? '';
		asked.push(ref);
		if (ref.endsWith(KNOWN.slice(2)))
			return route.fulfill({
				contentType: 'application/json',
				body: JSON.stringify({ rpId: 'getvela.app', name: REGISTRY_NAME, createdAt: 1 })
			});
		return route.fulfill({ status: 404, contentType: 'application/json', body: '{}' });
	});
	return asked;
}

async function openContacts(page: Page): Promise<void> {
	await page.goto('/en/contacts');
	await expect(page.getByRole('heading', { name: en('contacts.title') }).first()).toBeVisible();
}

test.beforeEach(async ({ page }) => {
	await denyOffOrigin(page);
	await seedSignedIn(page);
	await seedHistory(page);
});

test.describe('desktop', () => {
	test.use({ viewport: { width: 1440, height: 900 } });

	test('the list calls a history row by its registry name; the address is the last resort', async ({
		page
	}) => {
		const asked = await stubIndex(page);
		await openContacts(page);

		// Nobody opened a detail: the book asked on its own.
		await expect(page.getByText(REGISTRY_NAME, { exact: true }).first()).toBeVisible();
		// …and the row nobody knows still introduces itself by its address.
		await expect(page.getByText(STRANGER_SHORT, { exact: true }).first()).toBeVisible();
		// Once per address — a second render is not a second ask.
		expect(asked.filter((ref) => ref.endsWith(KNOWN.slice(2)))).toHaveLength(1);
		// Being recognised is not being saved: nothing was written to the book.
		expect(await readKv(page, 'vela.contacts')).toBeNull();
	});

	test('a history row is named from under its own name, and the name survives a reload', async ({
		page
	}) => {
		await stubIndex(page);
		await openContacts(page);
		await page.getByText(STRANGER_SHORT, { exact: true }).first().click();

		// The action sits in the identity block, not only in the footer.
		const save = page.getByRole('button', { name: en('contacts.saveToContacts'), exact: true });
		await expect(save).toBeVisible();
		const name = page.locator('.panel .name');
		const gap = ((await save.boundingBox())?.y ?? 0) - ((await name.boundingBox())?.y ?? 0);
		expect(gap).toBeGreaterThan(0);
		expect(gap).toBeLessThan(80);

		await save.click();
		await expect(
			page.getByRole('heading', { name: en('contacts.saveToContacts') }).first()
		).toBeVisible();
		await page.getByLabel(en('contacts.nameLabel')).fill('Carol from history');
		await page.getByRole('button', { name: en('contacts.save'), exact: true }).click();

		await expect(page.getByText('Carol from history', { exact: true }).first()).toBeVisible();
		// Named by the person now: the prompt to name it is gone.
		await expect(save).toHaveCount(0);

		await page.reload();
		await expect(page.getByText('Carol from history', { exact: true }).first()).toBeVisible();
		await expect(page.getByText(STRANGER_SHORT, { exact: true })).toHaveCount(1); // the row's address line
		expect(await readKv(page, 'vela.contacts')).toContain('"source":"manual"');
	});

	test("the person's own name outranks the registry's", async ({ page }) => {
		await stubIndex(page);
		await openContacts(page);
		await page.getByText(REGISTRY_NAME, { exact: true }).first().click();
		// Recognised, but still not the person's word for them — so still offered.
		await page.getByRole('button', { name: en('contacts.saveToContacts'), exact: true }).click();
		await page.getByLabel(en('contacts.nameLabel')).fill('Bob from the gym');
		await page.getByRole('button', { name: en('contacts.save'), exact: true }).click();

		await expect(page.getByText('Bob from the gym', { exact: true }).first()).toBeVisible();
		await expect(page.getByText(REGISTRY_NAME, { exact: true })).toHaveCount(0);
	});
});

test.describe('phone', () => {
	test.use({ viewport: { width: 390, height: 844 } });

	test('the same two rules on the phone layout', async ({ page }) => {
		await stubIndex(page);
		await openContacts(page);
		await expect(page.getByText(REGISTRY_NAME, { exact: true }).first()).toBeVisible();

		await page.getByText(STRANGER_SHORT, { exact: true }).first().click();
		await page.getByRole('button', { name: en('contacts.saveToContacts'), exact: true }).click();
		await page.getByLabel(en('contacts.nameLabel')).fill('Carol from history');
		await page.getByRole('button', { name: en('contacts.save'), exact: true }).click();
		await expect(page.getByText('Carol from history', { exact: true }).first()).toBeVisible();
	});
});
