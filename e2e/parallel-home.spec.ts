/**
 * Home daily-use flows — through the parallel space (Epic 2/6). P1.
 *
 * The fixture keyset gives three real accounts, so the multi-account UX (switcher +
 * SWITCH_ACCOUNT + header) is exercisable deterministically. Read-only (no funds).
 */
import { test, expect } from '@playwright/test';
import { enterParallel, stubWalletNetwork } from './support/parallel';

test.describe('parallel-space · Home', () => {
  test('switches between the fixture accounts', async ({ page }) => {
    await enterParallel(page); // lands on Home as "Parallel One"

    // Open the account switcher from the header (by role so it's identity-agnostic;
    // force-click because the Home's entering animations keep it "unstable").
    await page.getByRole('button', { name: /Switch account/ }).click({ force: true });

    // The switcher lists the other fixture accounts.
    await expect(page.getByText('Parallel Two')).toBeVisible({ timeout: 15_000 });
    await expect(page.getByText('Parallel Three')).toBeVisible();

    // Switch to Parallel Two → the Home header reflects it, with its address.
    await page.getByText('Parallel Two').click({ force: true });
    await expect(page.getByRole('button', { name: /Parallel Two/ })).toBeVisible({ timeout: 10_000 });
    await expect(page.getByText('0x031d...772b')).toBeVisible();
  });

  test('toggles Activity ⇄ Connections tabs', async ({ page }) => {
    await stubWalletNetwork(page);
    await enterParallel(page);

    await page.getByText('Connections', { exact: true }).click({ force: true });
    await expect(page.getByText('No active connection')).toBeVisible({ timeout: 10_000 });
    await page.getByText('Activity', { exact: true }).click({ force: true });
  });
});
