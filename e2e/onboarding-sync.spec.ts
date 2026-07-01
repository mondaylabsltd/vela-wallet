/**
 * Onboarding sync-failure E2E — the fund-safety invariant (US 1.3).
 *
 * Guards the recent fix "don't persist wallet locally until public key is synced"
 * (commit bfa6465): if the key upload to the index server fails, the wallet must
 * NOT be written to local storage — otherwise it would be usable on this device
 * but unrecoverable on any other, and boot auto-enters on any saved account so
 * the gap would stay silent.
 *
 * Approach: a CDP virtual WebAuthn authenticator makes passkey registration
 * produce a real, parseable attestation without a device, and all external hosts
 * are blocked so the index-server upload fails deterministically after its 3
 * retries.
 *
 * Run: npx playwright test onboarding-sync
 */
import { test, expect, type Page } from '@playwright/test';

/** Block every external host; the app bundle + routes are served from localhost. */
async function blockExternal(page: Page) {
  await page.route('**/*', (route) => {
    const h = new URL(route.request().url()).hostname;
    if (h === 'localhost' || h === '127.0.0.1') route.continue();
    else route.abort();
  });
}

const ACK_FRAGMENTS = [
  'This is a self-custodial wallet',
  'If you lose your device',
  'If your iCloud or Google account is compromised',
  'I agree to the',
];

test.describe('Onboarding — wallet is NOT persisted until the key syncs (US 1.3)', () => {
  test('sync failure keeps the account out of local storage and offers retry', async ({ page }) => {
    // 1. Virtual WebAuthn authenticator → passkey registration returns a real,
    //    parseable attestation with no physical device.
    const client = await page.context().newCDPSession(page);
    await client.send('WebAuthn.enable');
    await client.send('WebAuthn.addVirtualAuthenticator', {
      options: {
        protocol: 'ctap2',
        transport: 'internal',
        hasResidentKey: true,
        hasUserVerification: true,
        isUserVerified: true,
        automaticPresenceSimulation: true,
      },
    });

    // 2. Block the index server (and every other external host) → upload fails.
    await blockExternal(page);

    // 3. Land straight on the create form.
    await page.goto('/onboarding?mode=create');
    await page.waitForLoadState('networkidle');
    await expect(page.locator('body')).toContainText('Create Wallet', { timeout: 40_000 });

    // 4. Name + acknowledge all four checkboxes (the Create button is disabled
    //    until name is set and every box is checked).
    await page.getByPlaceholder('Enter a name for your account').fill('E2E Sync Test');
    for (const frag of ACK_FRAGMENTS) {
      await page.getByText(frag, { exact: false }).first().click();
    }

    // 5. Create → passkey registers (virtual authenticator) → upload retries 3×
    //    (1s + 2s backoff) and fails. The header and the button share the label
    //    "Create Wallet", so target the button (last in DOM order).
    await page.getByText('Create Wallet', { exact: true }).last().click();

    // 6. The sync-failed state must appear with retry + bug-report affordances.
    await expect(page.locator('body')).toContainText('Sync failed', { timeout: 30_000 });
    await expect(page.locator('body')).toContainText('Retry Upload');
    await expect(page.locator('body')).toContainText('Report this error');
    await page.screenshot({ path: 'e2e/screenshots/onboarding-sync-failed.png', fullPage: true });

    // 7. THE INVARIANT: the account is NOT in local storage (an unsynced wallet
    //    is usable here but unrecoverable elsewhere — so it must not be saved).
    const accounts = await page.evaluate(() => localStorage.getItem('vela.accounts'));
    expect(accounts === null || accounts === '[]').toBeTruthy();

    // A pending upload SHOULD exist — it's what the Retry button drives.
    const pending = await page.evaluate(() => localStorage.getItem('vela.pendingUploads'));
    expect(pending).toBeTruthy();
    expect(pending).not.toBe('[]');
  });
});
