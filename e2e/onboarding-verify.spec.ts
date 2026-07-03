/**
 * Onboarding verify-before-persist E2E — the dead-passkey invariant (issue #1).
 *
 * A provider can report a successful create() and still fail to durably store
 * the credential (issue #1: "created successfully" yet absent from Google
 * Password Manager, with nowhere to sign). The creation flow must therefore
 * prove the passkey can SIGN before anything is persisted or the address is
 * ever shown: register → test signature → index upload → save → success.
 *
 * Approach: a CDP virtual WebAuthn authenticator (Chrome's virtual passkeys)
 * drives real create()/get() ceremonies. For the dead-passkey case the test
 * gates navigator.credentials.get() behind a resumable latch, removes the
 * just-created credential via CDP while the app is paused at the latch, then
 * releases it — deterministically simulating "credential vanished between
 * creation and first use" with no race.
 *
 * Run: npx playwright test onboarding-verify
 */
import { test, expect, type Page } from '@playwright/test';

const ACK_FRAGMENTS = [
  'This is a self-custodial wallet',
  'If you lose your device',
  'If your iCloud or Google account is compromised',
  'I agree to the',
];

const AUTHENTICATOR_OPTIONS = {
  protocol: 'ctap2',
  transport: 'internal',
  hasResidentKey: true,
  hasUserVerification: true,
  isUserVerified: true,
  automaticPresenceSimulation: true,
} as const;

/**
 * Stub the network: local assets pass through, the public-key index gets a
 * stateful in-memory mock (create stores, query echoes — uploadPublicKey
 * verifies the stored key matches), everything else external gets a benign
 * JSON-RPC null so nothing hangs or spends real funds.
 */
async function stubNetworkWithIndexMock(page: Page): Promise<void> {
  let record: Record<string, unknown> | null = null;

  await page.route('**/*', (route) => {
    const url = route.request().url();
    const local =
      /^https?:\/\/(localhost|127\.0\.0\.1)(:\d+)?(\/|$)/.test(url) ||
      url.startsWith('data:') || url.startsWith('blob:');
    if (local) return route.continue();

    if (url.includes('/api/health')) {
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ service: 'webauthn-p256-publickey-index', status: 'ok' }),
      });
    }
    if (url.includes('/api/create')) {
      record = { ...(route.request().postDataJSON() as Record<string, unknown>), createdAt: Date.now() };
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(record) });
    }
    if (url.includes('/api/query')) {
      return record
        ? route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(record) })
        : route.fulfill({ status: 404, contentType: 'application/json', body: '{"error":"not found"}' });
    }
    return route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ jsonrpc: '2.0', id: 1, result: null }),
    });
  });
}

async function fillCreateForm(page: Page, name: string): Promise<void> {
  await page.goto('/onboarding?mode=create');
  await page.waitForLoadState('networkidle');
  await expect(page.locator('body')).toContainText('Create Wallet', { timeout: 40_000 });
  await page.getByPlaceholder('Enter a name for your account').fill(name);
  for (const frag of ACK_FRAGMENTS) {
    await page.getByText(frag, { exact: false }).first().click();
  }
}

test.describe('Onboarding — passkey must prove it can sign before anything persists (issue #1)', () => {

  test('happy path: create → auto-verify → address only on success → Enter Wallet', async ({ page }) => {
    const client = await page.context().newCDPSession(page);
    await client.send('WebAuthn.enable');
    await client.send('WebAuthn.addVirtualAuthenticator', { options: AUTHENTICATOR_OPTIONS });
    await stubNetworkWithIndexMock(page);

    await fillCreateForm(page, 'E2E Verify Test');
    await page.getByText('Create Wallet', { exact: true }).last().click();

    // Register + test signature + upload all run inside one flow now; the
    // success screen appears only after signing is proven AND the key synced.
    await expect(page.locator('body')).toContainText('Your wallet is ready!', { timeout: 30_000 });
    await expect(page.locator('body')).toContainText(/0x[0-9a-fA-F]/);
    await expect(page.locator('body')).toContainText('Enter Wallet');

    // Persisted exactly one account — signing was proven, key confirmed synced.
    const accounts = await page.evaluate(() => localStorage.getItem('vela.accounts'));
    expect(accounts).toBeTruthy();
    expect(JSON.parse(accounts!)).toHaveLength(1);

    await page.getByText('Enter Wallet', { exact: true }).click();
    await expect(page.locator('body')).toContainText('E2E Verify Test', { timeout: 20_000 });
  });

  test('dead passkey: created but unable to sign → NOTHING persisted, resume offered, no second passkey', async ({ page }) => {
    const client = await page.context().newCDPSession(page);
    await client.send('WebAuthn.enable');
    const { authenticatorId } = await client.send('WebAuthn.addVirtualAuthenticator', {
      options: AUTHENTICATOR_OPTIONS,
    });
    await stubNetworkWithIndexMock(page);

    // Latch navigator.credentials.get() so the credential can be removed
    // deterministically between create() resolving and the verify signature.
    await page.addInitScript(() => {
      const w = window as unknown as {
        __gateNextGet?: boolean;
        __releaseGet?: (() => void) | null;
      };
      const orig = navigator.credentials.get.bind(navigator.credentials);
      navigator.credentials.get = async (options?: CredentialRequestOptions) => {
        if (w.__gateNextGet) {
          w.__gateNextGet = false;
          await new Promise<void>((resolve) => { w.__releaseGet = resolve; });
        }
        return orig(options);
      };
    });

    const created: string[] = [];
    client.on('WebAuthn.credentialAdded', (event) => {
      created.push(event.credential.credentialId);
    });

    await fillCreateForm(page, 'E2E Dead Passkey');
    await page.evaluate(() => { (window as unknown as { __gateNextGet?: boolean }).__gateNextGet = true; });
    await page.getByText('Create Wallet', { exact: true }).last().click();

    // The passkey registers, then the app parks at the latched get(). Remove
    // the credential — "provider lost it" — and let the verify signature run.
    await expect.poll(() => created.length, { timeout: 20_000 }).toBe(1);
    await expect.poll(() =>
      page.evaluate(() => Boolean((window as unknown as { __releaseGet?: unknown }).__releaseGet)),
      { timeout: 20_000 },
    ).toBe(true);
    await client.send('WebAuthn.removeCredential', { authenticatorId, credentialId: created[0] });
    await page.evaluate(() => {
      const w = window as unknown as { __releaseGet?: (() => void) | null };
      w.__releaseGet?.();
      w.__releaseGet = null;
    });

    // Verification fails → resume state, NOT a success screen.
    await expect(page.locator('body')).toContainText('Verification was cancelled', { timeout: 30_000 });
    await expect(page.locator('body')).toContainText('Finish Verification');
    await expect(page.locator('body')).not.toContainText('Your wallet is ready!');

    // THE INVARIANT: a passkey that cannot sign leaves NO trace — no local
    // account (would sit dead in the switcher forever) and no pending upload
    // (the index must never hear about an unusable credential).
    const accounts = await page.evaluate(() => localStorage.getItem('vela.accounts'));
    expect(accounts === null || accounts === '[]').toBeTruthy();
    const pending = await page.evaluate(() => localStorage.getItem('vela.pendingUploads'));
    expect(pending === null || pending === '[]').toBeTruthy();

    // Resume retries ONLY the signature — it must never mint a second passkey.
    await page.getByText('Finish Verification', { exact: true }).click();
    await expect(page.locator('body')).toContainText('Verification was cancelled', { timeout: 30_000 });
    expect(created).toHaveLength(1);

    await page.screenshot({ path: 'e2e/screenshots/onboarding-dead-passkey.png', fullPage: true });
  });
});
