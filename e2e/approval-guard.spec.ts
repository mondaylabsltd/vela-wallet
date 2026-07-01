/**
 * Approval-guard E2E — the "never unlimited" mandate at the UI layer (US 5.3).
 *
 * clear-signing.spec.ts already checks that an unlimited approve shows a warning.
 * This strengthens that into the interactive guarantee the founder mandate is
 * actually about: an unbounded ERC-20 approve renders the finite-cap EDITOR with
 * NO default choice (so the confirm control is gated), offers no "Max"/"Unlimited"
 * preset, and lets the user set a finite cap. The pure unbounded-detection math is
 * covered by src/__tests__/services/approval-guard.test.ts.
 *
 * Run: npx playwright test approval-guard
 */
import { test, expect, type Page } from '@playwright/test';

const TEST_PAGE = '/clear-signing-test';

async function openScenario(page: Page, label: string) {
  await page.goto(TEST_PAGE);
  await page.waitForLoadState('networkidle');
  await page.waitForTimeout(2000);
  await page.locator(`text=${label}`).first().click();
  await page.waitForTimeout(1500);
}

test.describe('Approval guard — no unlimited approvals (US 5.3)', () => {
  test('unbounded approve renders a finite-cap editor, gated until a finite amount is set', async ({ page }) => {
    await openScenario(page, 'ERC-20 Approve');
    const body = page.locator('body');

    // The editable spending-cap control replaced the passive "unlimited" banner.
    await expect(body).toContainText('Spending cap');
    // No default choice — the confirm stays gated until the user picks a finite amount.
    await expect(body).toContainText('Set a finite amount to continue.');
    // There is intentionally no "Max"/"Unlimited" preset — only Custom + Revoke
    // for an unbounded request.
    await expect(body).toContainText('Custom');
    await expect(body).toContainText('Revoke');

    // Enter a finite cap → the plain-language summary confirms a bounded amount
    // and the gating prompt disappears (confirm becomes possible).
    await page.locator('input[placeholder="0"]').first().fill('100');
    await expect(body).toContainText('can spend up to');
    await expect(body).not.toContainText('Set a finite amount to continue.');

    await page.screenshot({ path: 'e2e/screenshots/approval-finite-cap.png', fullPage: true });
  });

  test('a reasonable finite approve is pre-accepted (not gated)', async ({ page }) => {
    await openScenario(page, 'ERC-20 Limited Approve');
    const body = page.locator('body');
    await expect(body).toContainText('Spending cap');
    // A finite, reasonable request needs no "set a finite amount" gate.
    await expect(body).not.toContainText('Set a finite amount to continue.');
  });
});
