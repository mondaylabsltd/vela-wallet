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
 * Hermetic: every off-origin request is denied; the chain and the index answer
 * from here, in the three hops `public-key-index.ts` documents.
 */
import { expect, test, type Page } from '@playwright/test';
import { en, seedSignedIn, TEST_ACCOUNT_ADDRESS } from './live-helpers';
import { denyOffOrigin, readKv, stubJsonRpc } from './stub-chain';

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

/** `SafeWebAuthnSharedSigner` and the founding key it holds for KNOWN. */
const SHARED_SIGNER = '0x94a4f6affbd8975951142c3999aeab7ecee555c2';
const KEY_X = '19'.repeat(32);
const KEY_Y = 'fe'.repeat(32);
const UNIT_ID = 10;

function metadataHex(address: string, names: string[]): string {
	const json = JSON.stringify({
		version: 1,
		address,
		wallet_version: 'safe-1.4.1',
		key_names: names,
		created_at_iso: '2026-08-21T00:00:00Z'
	});
	return Buffer.from(json, 'utf8').toString('hex');
}

/**
 * The registry, the way the v2 index can actually be asked about an ADDRESS:
 * the chain says which key founded the Safe, the index says which units that
 * key founded, and the unit whose metadata carries this address names it.
 * Registered AFTER the deny net, so these win. Returns what the index was asked.
 */
async function stubRegistry(page: Page): Promise<string[]> {
	const asked: string[] = [];
	await stubJsonRpc(page, /^https?:\/\/(?!localhost|127\.0\.0\.1)/, (method, params) => {
		if (method !== 'eth_call') return undefined;
		const call = params[0] as { to?: string; data?: string };
		const known =
			call.to?.toLowerCase() === SHARED_SIGNER && call.data?.endsWith(KNOWN.slice(2)) === true;
		// Everything else — an unconfigured Safe, every name-service registry —
		// reads as zero: nobody.
		return known ? '0x' + KEY_X + KEY_Y + '0'.repeat(61) + '100' : '0x' + '0'.repeat(192);
	});
	await page.route(/\/api\/query\?/, (route) => {
		const url = new URL(route.request().url());
		asked.push(url.search);
		const json = (body: unknown) =>
			route.fulfill({ contentType: 'application/json', body: JSON.stringify(body) });
		if (url.searchParams.get('publicKey') === '04' + KEY_X + KEY_Y)
			return json({ entry: { entryId: 1 }, groups: { total: 1, unitIds: [UNIT_ID] } });
		if (url.searchParams.get('unitId') === String(UNIT_ID))
			return json({ unit: { unitId: UNIT_ID, metadata: metadataHex(KNOWN, [REGISTRY_NAME]) } });
		// What the v2 index really says to the old by-address question.
		if (url.searchParams.has('walletRef'))
			return route.fulfill({ status: 400, contentType: 'application/json', body: '{}' });
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
		const asked = await stubRegistry(page);
		await openContacts(page);

		// Nobody opened a detail: the book asked on its own.
		await expect(page.getByText(REGISTRY_NAME, { exact: true }).first()).toBeVisible();
		// …and the row nobody knows still introduces itself by its address.
		await expect(page.getByText(STRANGER_SHORT, { exact: true }).first()).toBeVisible();
		// Once: the book and the activity feed both want this name, and share one
		// waterfall. And never the question the v2 index cannot answer.
		expect(asked.filter((query) => query.includes('publicKey='))).toHaveLength(1);
		expect(asked.some((query) => query.includes('walletRef'))).toBe(false);
		// Being recognised is not being saved: nothing was written to the book.
		expect(await readKv(page, 'vela.contacts')).toBeNull();
	});

	test('a history row is named from under its own name, and the name survives a reload', async ({
		page
	}) => {
		await stubRegistry(page);
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
		await stubRegistry(page);
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
		await stubRegistry(page);
		await openContacts(page);
		await expect(page.getByText(REGISTRY_NAME, { exact: true }).first()).toBeVisible();

		await page.getByText(STRANGER_SHORT, { exact: true }).first().click();
		await page.getByRole('button', { name: en('contacts.saveToContacts'), exact: true }).click();
		await page.getByLabel(en('contacts.nameLabel')).fill('Carol from history');
		await page.getByRole('button', { name: en('contacts.save'), exact: true }).click();
		await expect(page.getByText('Carol from history', { exact: true }).first()).toBeVisible();
	});
});
