/**
 * Send-to-group E2E — pick a whole contact group as the recipients of a send.
 *
 * Seeds a saved address book + a "Payroll" group, then from the Send screen opens
 * the recipient picker, taps the group, and asserts the split editor is seeded
 * with one row per member (the same buildSplitCalls path a hand-built split uses).
 * Group CRUD + membership resolution are unit-tested in contacts.test.ts; this
 * covers the picker → split wiring in a real browser.
 *
 * Run: npx playwright test send-to-group
 */
import { test, expect, type Page } from '@playwright/test';

const ME = '0x742d35cc6634c0532925a3b844bc454e4438f44e';
const ME_NO0X = ME.slice(2);
const A = '0x1111111111111111111111111111111111111111';
const B = '0x2222222222222222222222222222222222222222';

const BALANCE_HEX =
  '0x' + [32n, 1n, 32n, 1n, 64n, 32n, 10000n * 10n ** 18n]
    .map((n) => n.toString(16).padStart(64, '0'))
    .join('');

async function seedAndMock(page: Page) {
  await page.addInitScript(({ me, a, b }) => {
    localStorage.setItem('vela.accounts', JSON.stringify([{ id: 'e2e', name: 'E2E', address: me, createdAt: '2026-01-01T00:00:00.000Z' }]));
    localStorage.setItem('vela.activeAccountIndex', '0');
    localStorage.setItem('vela.contacts', JSON.stringify([
      { address: a, name: 'Alice', kind: 'unknown', txCount: 0, lastUsed: 1, firstSeen: 1, source: 'manual' },
      { address: b, name: 'Bob', kind: 'unknown', txCount: 0, lastUsed: 1, firstSeen: 1, source: 'manual' },
    ]));
    localStorage.setItem('vela.contactGroups', JSON.stringify([{ id: 'grp_1', name: 'Payroll', members: [a, b] }]));
  }, { me: ME, a: A, b: B });

  await page.route('**/*', (route) => {
    const req = route.request();
    const host = new URL(req.url()).hostname;
    if (host === 'localhost' || host === '127.0.0.1') return route.continue();
    const body = (req.postData() || '').toLowerCase();
    const id = Number(body.match(/"id"\s*:\s*(\d+)/)?.[1] ?? 1);
    if (body.includes('"eth_call"') && body.includes(ME_NO0X)) {
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ jsonrpc: '2.0', id, result: BALANCE_HEX }) });
    }
    return route.abort();
  });
}

test.describe('Send — pick a whole group as recipients', () => {
  test('choosing the Payroll group seeds the split editor with every member', async ({ page }) => {
    await seedAndMock(page);
    await page.goto('/send');
    await page.waitForLoadState('networkidle');

    await expect(page.locator('body')).toContainText('Select Token', { timeout: 40_000 });
    await page.getByText('Gas', { exact: true }).first().click();
    await page.getByText('BNB Chain', { exact: true }).click();

    // Open the recipient picker (address-book button carries an aria-label).
    await page.getByLabel('Choose recipient or scan').click();

    // The Groups section lists Payroll with its member count.
    await expect(page.getByTestId('group-row')).toBeVisible({ timeout: 15_000 });
    await expect(page.locator('body')).toContainText('Payroll');
    await expect(page.locator('body')).toContainText('2 members');

    // Pick the group → split editor seeded with one row per member.
    await page.getByTestId('group-row').click();
    await expect(page.locator('body')).toContainText('Recipient 1', { timeout: 15_000 });
    await expect(page.locator('body')).toContainText('Recipient 2');

    await page.screenshot({ path: 'e2e/screenshots/send-to-group.png', fullPage: true });
  });
});
