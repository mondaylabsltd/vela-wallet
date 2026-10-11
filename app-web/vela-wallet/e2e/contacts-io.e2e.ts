/**
 * The book travels, and the book does things (spec 028 US5 — SC-408).
 *
 * An export re-imported changes nothing; an import colliding with an existing
 * entry keeps the existing one; a file the core cannot read is refused before
 * anything is written; a group can be filled from the book and from a file;
 * and the detail's actions lead somewhere. Every rule asserted here is the
 * core's — this suite only proves the web shell carries files and taps to it.
 */
import { readFileSync } from 'node:fs';
import { expect, test, type Page } from '@playwright/test';
import { en, seedSignedIn } from './live-helpers';
import { denyOffOrigin } from './stub-chain';

test.use({ viewport: { width: 390, height: 844 } });

const ALICE_ADDR = '0x' + 'a1'.repeat(20);
const BOB_ADDR = '0x' + 'b2'.repeat(20);

test.beforeEach(async ({ page }) => {
	await seedSignedIn(page);
});

function filled(template: string, vars: Record<string, string | number>): string {
	return template.replace(/\{\{\s*(\w+)\s*\}\}/g, (match, name: string) =>
		name in vars ? String(vars[name]) : match
	);
}

async function openContacts(page: Page): Promise<void> {
	await page.goto('/en/contacts');
	await expect(page.getByRole('heading', { name: en('contacts.title') }).first()).toBeVisible();
}

/** The phone's "+" → the drawn C5 sheet → one of its three rows. */
async function plusMenu(page: Page, row: string): Promise<void> {
	await page
		.getByRole('button', { name: en('contacts.addContact') })
		.first()
		.click();
	await page.getByRole('menuitem', { name: row }).click();
}

async function addContact(page: Page, name: string, address: string): Promise<void> {
	await plusMenu(page, en('contacts.addTitle'));
	await page.getByLabel(en('contacts.nameLabel')).fill(name);
	await page.getByLabel(en('contacts.addressLabel')).fill(address);
	await page.getByRole('button', { name: en('contacts.save') }).click();
	await expect(page.getByText(name, { exact: true })).toBeVisible();
}

async function addGroup(page: Page, name: string): Promise<void> {
	await page.getByText(en('contacts.groupNew'), { exact: true }).click();
	await page.getByLabel(en('contacts.groupNameLabel')).fill(name);
	await page.getByRole('button', { name: en('contacts.save') }).click();
	await expect(page.getByText(name, { exact: true })).toBeVisible();
}

/** Import through the given menu row; answer the picker with `content` (text is UTF-8). */
async function importFile(
	page: Page,
	openRow: () => Promise<void>,
	name: string,
	content: string | Buffer
): Promise<void> {
	const [chooser] = await Promise.all([page.waitForEvent('filechooser'), openRow()]);
	await chooser.setFiles({
		name,
		mimeType: 'application/octet-stream',
		buffer: typeof content === 'string' ? Buffer.from(content) : content
	});
}

async function expectReport(page: Page, body: string): Promise<void> {
	await expect(page.getByTestId('import-report')).toHaveText(body);
	await page.getByRole('button', { name: en('common.done') }).click();
	await expect(page.getByTestId('import-report')).toHaveCount(0);
}

test('export → delete → import restores the book; re-import changes nothing; existing wins', async ({
	page
}) => {
	await openContacts(page);
	await addContact(page, 'Alice', ALICE_ADDR);
	await addGroup(page, 'Payroll');

	// 添加成员: the group screen's ghost row opens the book with a tick per person.
	await page.getByText('Payroll', { exact: true }).click();
	await page.getByRole('button', { name: en('contacts.addMember') }).click();
	await page.getByRole('option', { name: /Alice/ }).click();
	await page.getByRole('button', { name: en('contacts.save') }).click();
	await expect(
		page.getByText(en('contacts.membersCount').replace('{{count}}', '1'), { exact: true })
	).toBeVisible();
	await page
		.getByRole('button', { name: en('contacts.cancel') })
		.first()
		.click();

	// Export: the core writes the file, the browser downloads it.
	const [download] = await Promise.all([
		page.waitForEvent('download'),
		(async () => {
			await plusMenu(page, en('contacts.exportTitle'));
			await page.getByRole('button', { name: 'JSON' }).click();
		})()
	]);
	expect(download.suggestedFilename()).toMatch(/^vela-contacts-\d{4}-\d{2}-\d{2}\.json$/);
	const path = await download.path();
	const backup = readFileSync(path, 'utf8');
	const parsed = JSON.parse(backup) as {
		version: number;
		contacts: unknown[];
		groups: { name: string; members: string[] }[];
	};
	expect(parsed.version).toBe(1);
	expect(parsed.contacts).toEqual([{ address: ALICE_ADDR, name: 'Alice' }]);
	expect(parsed.groups).toEqual([{ name: 'Payroll', members: [ALICE_ADDR] }]);

	// Re-import over the same book: nothing new, nothing changed (SC-408).
	await importFile(page, () => plusMenu(page, en('contacts.importFile')), 'backup.json', backup);
	await expectReport(page, filled(en('contacts.importDoneBody'), { added: 0, skipped: 1 }));
	await expect(page.getByText('Alice', { exact: true })).toBeVisible();

	// Existing wins: a local rename survives a file that spells the old name.
	await page.getByText('Alice', { exact: true }).click();
	await page
		.getByRole('button', { name: en('contacts.edit') })
		.first()
		.click();
	await page.getByLabel(en('contacts.nameLabel')).fill('Alice Local');
	await page.getByRole('button', { name: en('contacts.save') }).click();
	await page
		.getByRole('button', { name: en('contacts.cancel') })
		.first()
		.click();
	await importFile(page, () => plusMenu(page, en('contacts.importFile')), 'backup.json', backup);
	await expectReport(page, filled(en('contacts.importDoneBody'), { added: 0, skipped: 1 }));
	await expect(page.getByText('Alice Local', { exact: true })).toBeVisible();
	await expect(page.getByText('Alice', { exact: true })).toHaveCount(0);

	// Delete her, then restore from the file: the book — and her group — return.
	await page.getByText('Alice Local', { exact: true }).click();
	await page.getByRole('button', { name: en('contacts.deleteContact') }).click();
	await page.getByRole('button', { name: en('contacts.delete'), exact: true }).click();
	await expect(page.getByText(en('contacts.empty'))).toBeVisible();
	await importFile(
		page,
		() => page.getByRole('button', { name: en('contacts.importFile') }).click(),
		'backup.json',
		backup
	);
	await expectReport(page, filled(en('contacts.importDoneBody'), { added: 1, skipped: 0 }));
	await expect(page.getByText('Alice', { exact: true })).toBeVisible();
	await page.getByText('Payroll', { exact: true }).click();
	await expect(
		page.getByText(en('contacts.membersCount').replace('{{count}}', '1'), { exact: true })
	).toBeVisible();
});

test('a file with no address is refused before anything is written', async ({ page }) => {
	await openContacts(page);
	await addContact(page, 'Alice', ALICE_ADDR);
	await importFile(
		page,
		() => plusMenu(page, en('contacts.importFile')),
		'theirs.csv',
		'name,email\nBob,bob@example.com\n'
	);
	await expect(page.getByText(en('contacts.importFailTitle'))).toBeVisible();
	await expect(page.getByTestId('import-report')).toHaveText(en('contacts.importFailBody'));
	await page.getByRole('button', { name: en('common.done') }).click();
	// The book is exactly what it was: Alice, and nobody from the file.
	await expect(page.getByText('Alice', { exact: true })).toBeVisible();
	await expect(page.getByText('Bob', { exact: true })).toHaveCount(0);
});

/**
 * Issue 333: the bytes are the core's to decode. A CSV that Excel saved on
 * Chinese Windows (GBK) used to import every Chinese name as `jxjjx����` —
 * `File.text()` decoded it here, and leniently. Now it is refused with how
 * to save it, and the same names saved as UTF-16 or UTF-8 land exactly.
 */
test('a GBK file is refused with how to save it; UTF-16 and UTF-8 names land exactly', async ({
	page
}) => {
	await openContacts(page);
	await addContact(page, 'Alice', ALICE_ADDR);
	const gbk = Buffer.concat([
		Buffer.from(`address,name\r\n${BOB_ADDR},jxjjx`),
		Buffer.from([0xb2, 0xe2, 0xca, 0xd4]), // 测试
		Buffer.from('\r\n')
	]);
	await importFile(page, () => plusMenu(page, en('contacts.importFile')), 'excel.csv', gbk);
	await expect(page.getByText(en('contacts.importFailTitle'))).toBeVisible();
	await expect(page.getByTestId('import-report')).toHaveText(en('contacts.importFailEncoding'));
	await page.getByRole('button', { name: en('common.done') }).click();
	await expect(page.getByText(/\uFFFD/)).toHaveCount(0);
	await expect(page.getByText('Alice', { exact: true })).toBeVisible();

	const names = ['jxjjx测试', 'Иван Петров', 'Zoë 🦊'];
	const rows = (from: number) =>
		names.map((name, i) => `0x${(from + i).toString(16).padStart(40, '0')},${name}`);
	const utf16 = Buffer.concat([
		Buffer.from([0xff, 0xfe]),
		Buffer.from(['address,name', ...rows(0x100)].join('\r\n'), 'utf16le')
	]);
	await importFile(page, () => plusMenu(page, en('contacts.importFile')), 'unicode.csv', utf16);
	await expectReport(page, filled(en('contacts.importDoneBody'), { added: 3, skipped: 0 }));
	for (const name of names) await expect(page.getByText(name, { exact: true })).toBeVisible();

	const utf8 = Buffer.concat([
		Buffer.from([0xef, 0xbb, 0xbf]),
		Buffer.from(['address,name', `0x${'0'.repeat(37)}200,张小明`].join('\n'))
	]);
	await importFile(page, () => plusMenu(page, en('contacts.importFile')), 'utf8.csv', utf8);
	await expectReport(page, filled(en('contacts.importDoneBody'), { added: 1, skipped: 0 }));
	await expect(page.getByText('张小明', { exact: true })).toBeVisible();
});

test('a CSV imported into a group seats every valid row in it', async ({ page }) => {
	await openContacts(page);
	await addContact(page, 'Alice', ALICE_ADDR);
	await addGroup(page, 'Payroll');
	await page.getByText('Payroll', { exact: true }).click();
	// An empty group's 群发转账 is disabled — and says why.
	await expect(page.getByRole('button', { name: en('contacts.batchSend') })).toBeDisabled();
	await expect(page.getByText(en('contacts.batchSendNeedsMembers'))).toBeVisible();
	// The group screen's ⋯ → 导入到本组.
	await importFile(
		page,
		async () => {
			await page.getByRole('button', { name: en('contacts.manage') }).click();
			await page.getByRole('menuitem', { name: en('contacts.importGroup') }).click();
		},
		'team.csv',
		`address,name\n${ALICE_ADDR},Imported Alice\n${BOB_ADDR},Bob\nnot-an-address,Nobody\n`
	);
	await expectReport(
		page,
		`${filled(en('contacts.importDoneBody'), { added: 1, skipped: 1 })} ${filled(en('contacts.importDoneInvalid'), { invalid: 1 })}`
	);
	// Alice (existing, name kept) and Bob (new) are both members now.
	await expect(
		page.getByText(en('contacts.membersCount').replace('{{count}}', '2'), { exact: true })
	).toBeVisible();
	await expect(page.getByText('Alice', { exact: true })).toBeVisible();
	await expect(page.getByText('Bob', { exact: true })).toBeVisible();
	await expect(page.getByText('Imported Alice')).toHaveCount(0);
});

test('the detail offers Send alone, and 转账 hands the person to the wallet', async ({ page }) => {
	// Hermetic: with the chain unreachable the send flow stops at its first
	// screen, the token pick, instead of auto-selecting whatever a live fetch
	// finds — which is the screen this test can name deterministically.
	await denyOffOrigin(page);
	await openContacts(page);
	await addContact(page, 'Alice', ALICE_ADDR);
	await page.getByText('Alice', { exact: true }).click();

	// Issue 479: a contact's page is about paying this person. Receive went
	// to the WALLET's own code and the QR card drew this person's address as a
	// code; both are gone, and no code is drawn anywhere on the page.
	const send = page.getByRole('button', { name: en('componentsUi.dock.send') });
	await expect(send).toBeVisible();
	await expect(page.getByRole('button', { name: en('componentsUi.dock.receive') })).toHaveCount(0);
	await expect(page.locator('svg[role="img"][aria-label="Alice"]')).toHaveCount(0);
	// The address is still the page's, in full, with its copy.
	await expect(
		page.getByRole('button', { name: en('componentsUi.identiconViewer.copyAddress') })
	).toBeVisible();

	// 转账 hands the person to /wallet, which reads the hand-off once, drops
	// the query, and opens the send flow with the recipient filled. Which
	// screen shows first is the core's call: the token pick, or — when the
	// wallet holds exactly one asset — straight to the form.
	await send.click();
	const picker = page.getByRole('heading', { name: en('send.selectTokenTitle') });
	const recipient = page.getByRole('textbox', { name: en('send.recipientLabel') });
	await expect(picker.or(recipient)).toBeVisible();
	if (await recipient.isVisible()) await expect(recipient).toHaveValue(ALICE_ADDR);
	// The query was read once and dropped: a reload here is a plain visit.
	await expect(page).toHaveURL('/en/wallet');
});
