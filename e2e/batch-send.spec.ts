/**
 * Payroll batch-send E2E — the "priced in fiat, paid in token" journey.
 *
 * Proves the whole import wiring in a real browser: on the Send screen, "Import
 * list" opens the batch sheet; a pasted (address, fiat-amount) table converts to
 * token amounts at an editable rate; applying seeds the split editor with one row
 * per payee — the ordinary single-UserOp split path from there.
 *
 * Conversion is driven by a MANUAL rate (the company's internal RMB↔token rate),
 * so the test is deterministic without mocking any price feed. The fiat→token math
 * itself is exhaustively covered in fiat-convert.test.ts; here we assert the UI.
 *
 * Offline like send-high-risk: we seed an account and mock the native-balance
 * multicall so the picker has a spendable coin; everything else is aborted.
 *
 * Run: npx playwright test batch-send
 */
import { test, expect, type Page } from '@playwright/test';

const ME = '0x742d35cc6634c0532925a3b844bc454e4438f44e';
const ME_NO0X = ME.slice(2);
const A = '0x1111111111111111111111111111111111111111';
const B = '0x2222222222222222222222222222222222222222';

/** aggregate3 result = [(success:true, returnData: uint256(10000e18))] — a fat
 *  native balance so a 3000-token payroll total sits comfortably within it. */
const BALANCE_HEX =
  '0x' + [32n, 1n, 32n, 1n, 64n, 32n, 10000n * 10n ** 18n]
    .map((n) => n.toString(16).padStart(64, '0'))
    .join('');

async function seedAndMock(page: Page) {
  await page.addInitScript((me) => {
    localStorage.setItem(
      'vela.accounts',
      JSON.stringify([{ id: 'e2e', name: 'E2E', address: me, createdAt: '2026-01-01T00:00:00.000Z' }]),
    );
    localStorage.setItem('vela.activeAccountIndex', '0');
  }, ME);

  await page.route('**/*', (route) => {
    const req = route.request();
    const host = new URL(req.url()).hostname;
    if (host === 'localhost' || host === '127.0.0.1') return route.continue();
    const body = (req.postData() || '').toLowerCase();
    const id = Number(body.match(/"id"\s*:\s*(\d+)/)?.[1] ?? 1);
    if (body.includes('"eth_call"') && body.includes(ME_NO0X)) {
      return route.fulfill({ status: 200, contentType: 'application/json',
        body: JSON.stringify({ jsonrpc: '2.0', id, result: BALANCE_HEX }) });
    }
    return route.abort(); // bundler, prices, data API — all offline
  });
}

test.describe('Send — payroll batch import (fiat → token)', () => {
  test('paste a fiat table, convert at a custom rate, seed the split editor', async ({ page }) => {
    await seedAndMock(page);
    await page.goto('/send');
    await page.waitForLoadState('networkidle');

    // Step 1 — pick the native coin (BNB row, unique via its "BNB Chain" subtitle).
    await expect(page.locator('body')).toContainText('Select Token', { timeout: 40_000 });
    await page.getByText('Gas', { exact: true }).first().click();
    await page.getByText('BNB Chain', { exact: true }).click();

    // Step 2 — open the batch importer.
    await page.getByTestId('send-batch-import').click();
    await expect(page.getByTestId('batch-paste')).toBeVisible({ timeout: 15_000 });

    // Read amounts as fiat. The currency picker is the SAME searchable sheet as
    // the home balance (proves the list is consistent + the nested sheet works).
    await page.getByTestId('batch-unit-fiat').click();
    await page.getByTestId('batch-currency').click();
    await expect(page.getByText('Display currency')).toBeVisible({ timeout: 10_000 });
    await page.getByPlaceholder('Search currency').fill('CNY');
    await page.getByText('Chinese Yuan').click();
    // The picker closes on selection, back to the importer priced in CNY.
    await expect(page.getByText('Display currency')).toBeHidden({ timeout: 10_000 });
    await expect(page.getByTestId('batch-currency')).toContainText('CNY');
    // Pin a custom rate: 1 BNB = 7.1 CNY.
    await page.getByTestId('batch-rate').fill('7.1');

    // Two payees, priced in fiat. 7100 / 7.1 = 1000; 14200 / 7.1 = 2000.
    await page.getByTestId('batch-paste').fill(`${A},7100\n${B},14200`);

    // Preview: two valid rows and the converted token total (3000, ungrouped).
    await expect(page.getByTestId('batch-row-ok')).toHaveCount(2);
    await expect(page.locator('body')).toContainText('3000');
    await page.screenshot({ path: 'e2e/screenshots/batch-import-ratecard.png', fullPage: true });

    // Apply → the split editor is seeded with both payees.
    await page.getByTestId('batch-apply').click();
    await expect(page.locator('body')).toContainText('Recipient 1', { timeout: 15_000 });
    await expect(page.locator('body')).toContainText('Recipient 2');
    // The converted total carries into the editor, and Continue is now actionable.
    await expect(page.locator('body')).toContainText('3000');
    await expect(page.getByText('Continue', { exact: true }).first()).toBeVisible();

    await page.screenshot({ path: 'e2e/screenshots/batch-send-import.png', fullPage: true });
  });
});
