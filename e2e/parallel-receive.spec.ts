/**
 * Receive flow — through the parallel space (Epic 4). P0: address integrity.
 *
 * The receive screen is read-only (no funds move), so it's fully hermetic. The key
 * security assertion is the anti-address-poisoning mask: copy/save are gated behind an
 * explicit "I Understand" before the address is usable.
 */
import { test, expect } from '@playwright/test';
import { enterParallel } from './support/parallel';

// Parallel One's Safe address, truncated the way ReceiveScreen shows it.
const ADDR_TRUNC = '0xD40086...de130b';

test.describe('parallel-space · Receive', () => {
  test.beforeEach(async ({ context }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write']).catch(() => {});
  });

  test('gates the address behind the anti-poisoning mask, then shows QR + networks', async ({ page }) => {
    await enterParallel(page);
    await page.goto('/receive');

    // The address-poisoning defence: an explicit acknowledgement mask over the QR.
    await expect(page.getByText('Before you receive')).toBeVisible({ timeout: 20_000 });
    const understand = page.getByText('I Understand', { exact: true });
    await expect(understand).toBeVisible();
    await understand.click();
    await expect(page.getByText('Before you receive')).toBeHidden();

    // The fixture address + the supported-network grid.
    await expect(page.getByText(ADDR_TRUNC)).toBeVisible();
    await expect(page.getByText(/Works on \d+ EVM networks/)).toBeVisible();
    await expect(page.getByText('Gnosis').first()).toBeVisible();

    // Copy → visible confirmation.
    await page.getByText(ADDR_TRUNC).click();
    await expect(page.getByText('Copied!')).toBeVisible({ timeout: 10_000 });
  });

  test('switches to the EIP-681 payment-request builder', async ({ page }) => {
    await enterParallel(page);
    await page.goto('/receive');
    await page.getByText('I Understand', { exact: true }).click();

    await page.getByText('Request', { exact: true }).click();
    await expect(page.getByText('Copy payment link')).toBeVisible({ timeout: 10_000 });
  });
});
