/**
 * Send flow — through the parallel space (Epic 3). P0: the core money-flow.
 *
 * Reliable, hermetic coverage of the Send ENTRY: the flow opens on the token picker
 * with search + add-token. We never proceed to Continue, so no funds move and no
 * UserOp is sent (the slide-to-confirm gate means a stray tap can't fire it anyway).
 *
 * NOTE (backlog): the deeper flow — select a funded token → recipient/amount → the
 * first-time-recipient risk badge → slide-to-confirm — needs the real Gnosis xDAI
 * balance to load into the picker (a warm balance cache). It's real-network-timing
 * dependent; drive it by warming the Home balances first, then navigating to Send.
 */
import { test, expect } from '@playwright/test';
import { enterParallel } from './support/parallel';

test.describe('parallel-space · Send', () => {
  test('opens the Send token picker (entry smoke)', async ({ page }) => {
    await enterParallel(page);
    await page.goto('/send');

    await expect(page.getByText('Select Token')).toBeVisible({ timeout: 25_000 });
    await expect(page.getByPlaceholder('Search tokens...')).toBeVisible();
    await expect(page.getByText('Add Token').first()).toBeVisible();

    // The search field filters (deterministic even with no tokens): a nonsense query
    // yields the empty state.
    await page.getByPlaceholder('Search tokens...').fill('zzzznotatoken');
    await expect(page.getByText(/No matching tokens/i)).toBeVisible({ timeout: 10_000 });
  });
});
