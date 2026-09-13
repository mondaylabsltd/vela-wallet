/**
 * The address book on the core, durably (spec 024 T041 — SC-002/SC-003).
 */
import { expect, test, type Page } from '@playwright/test';
import { en, seedSignedIn } from './live-helpers';

// The live wiring's phase-3/4 scope is the mobile layout (desktop
// interactivity is a recorded debt) — so the suites speak to it.
test.use({ viewport: { width: 390, height: 844 } });

const ALICE_ADDR = '0x' + 'a1'.repeat(20);

test.beforeEach(async ({ page }) => {
	await seedSignedIn(page);
});

async function openContacts(page: Page): Promise<void> {
	await page.goto('/en/contacts');
	await expect(page.getByRole('heading', { name: en('contacts.title') }).first()).toBeVisible();
}

async function addContact(page: Page, name: string, address: string): Promise<void> {
	// The phone's "+" opens the drawn C5 sheet (new / import / export) since
	// 028 US5; 新建联系人 is its first row. The empty state's own button still
	// opens the form directly — this helper takes the header route on purpose.
	await page
		.getByRole('button', { name: en('contacts.addContact') })
		.first()
		.click();
	await page.getByRole('menuitem', { name: en('contacts.addTitle') }).click();
	await page.getByLabel(en('contacts.nameLabel')).fill(name);
	await page.getByLabel(en('contacts.addressLabel')).fill(address);
	await page.getByRole('button', { name: en('contacts.save') }).click();
}

test('the tab that used to swallow its tap now navigates (SC-003)', async ({ page }) => {
	await page.goto('/en/wallet');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await page
		.getByRole('button', { name: en('contacts.title') })
		.first()
		.click();
	await expect(page).toHaveURL('/en/contacts');
});

test('an added contact survives a reload; a deleted one stays gone', async ({ page }) => {
	await openContacts(page);
	// A fresh book invites.
	await expect(page.getByText(en('contacts.empty'))).toBeVisible();

	await addContact(page, 'Alice', ALICE_ADDR);
	await expect(page.getByText('Alice', { exact: true })).toBeVisible();

	// Durability: the book is the core's stored ledger, not screen state.
	await page.reload();
	await expect(page.getByText('Alice', { exact: true })).toBeVisible();

	// Delete goes through the drawn confirm sheet and the core's tombstones.
	await page.getByText('Alice', { exact: true }).click();
	await page.getByRole('button', { name: en('contacts.deleteContact') }).click();
	await page.getByRole('button', { name: en('contacts.delete'), exact: true }).click();
	await expect(page.getByText('Alice', { exact: true })).toHaveCount(0);

	// A tombstone is durable too — deletion does not resurrect on reload.
	await page.reload();
	await expect(page.getByText(en('contacts.empty'))).toBeVisible();
});

test('the invalid-address gate holds the form shut', async ({ page }) => {
	await openContacts(page);
	await page
		.getByRole('button', { name: en('contacts.addContact') })
		.first()
		.click();
	await page.getByRole('menuitem', { name: en('contacts.addTitle') }).click();
	await page.getByLabel(en('contacts.nameLabel')).fill('Mallory');
	await page.getByLabel(en('contacts.addressLabel')).fill('not-an-address');
	// The save button refuses; the corpus's own words say why.
	await expect(page.getByRole('button', { name: en('contacts.save') })).toBeDisabled();
	await expect(page.getByText(en('contacts.invalidAddress'))).toBeVisible();
});

test('a group is created, joined to the book, and survives restart', async ({ page }) => {
	await openContacts(page);
	await addContact(page, 'Alice', ALICE_ADDR);
	await expect(page.getByText('Alice', { exact: true })).toBeVisible();

	await page.getByText(en('contacts.groupNew'), { exact: true }).click();
	await page.getByLabel(en('contacts.groupNameLabel')).fill('Payroll');
	await page.getByRole('button', { name: en('contacts.save') }).click();
	await expect(page.getByText('Payroll', { exact: true })).toBeVisible();

	await page.reload();
	await expect(page.getByText('Payroll', { exact: true })).toBeVisible();
});
