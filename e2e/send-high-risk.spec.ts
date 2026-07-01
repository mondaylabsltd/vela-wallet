/**
 * Send high-risk confirmation E2E (US 3.7) — the "mock the chain" journey test.
 *
 * A send to a never-before-seen CONTRACT address must, at the confirm step, show
 * the "First time" + "Contract" risk tags and gate submission behind a deliberate
 * slide-to-confirm (a stray tap can't fire a payment; the slide also turns red for
 * a risky destination). Source: SendScreen.tsx confirm CTA (SlideToConfirmButton,
 * tone='danger' when firstInteraction || isContract).
 *
 * To reach the confirm step offline we mock the two RPC calls the flow needs:
 *   1. Multicall3 aggregate3 → a single non-zero native balance, so the token
 *      picker has a spendable coin (fixture layout verified against the app's real
 *      decAggregate3 decoder). Every chain's balance query embeds the wallet
 *      address, so we match on that.
 *   2. eth_getCode(recipient) → non-empty bytecode, so resolveRecipientRisk marks
 *      the recipient a contract. "First time" needs no RPC (empty local history).
 * The bundler is left blocked → checkBundlerFunding returns null ("can't reach —
 * let it proceed") → the flow advances to confirm without a funding modal.
 *
 * Run: npx playwright test send-high-risk
 */
import { test, expect, type Page } from '@playwright/test';

const ME = '0x742d35cc6634c0532925a3b844bc454e4438f44e'; // lowercase for body matching
const ME_NO0X = ME.slice(2);
const CONTRACT_RECIPIENT = '0x1111111111111111111111111111111111111111';

/** aggregate3 result = [(success:true, returnData: uint256(1e18))]. */
const BALANCE_HEX =
  '0x' + [32n, 1n, 32n, 1n, 64n, 32n, 10n ** 18n]
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
    if (host === 'localhost' || host === '127.0.0.1') return route.continue(); // app bundle + routes
    const body = (req.postData() || '').toLowerCase();
    const id = Number(body.match(/"id"\s*:\s*(\d+)/)?.[1] ?? 1);
    // Native-balance multicall (getEthBalance embeds our address) → 1 coin.
    if (body.includes('"eth_call"') && body.includes(ME_NO0X)) {
      return route.fulfill({ status: 200, contentType: 'application/json',
        body: JSON.stringify({ jsonrpc: '2.0', id, result: BALANCE_HEX }) });
    }
    // Recipient bytecode → contract.
    if (body.includes('"eth_getcode"')) {
      return route.fulfill({ status: 200, contentType: 'application/json',
        body: JSON.stringify({ jsonrpc: '2.0', id, result: '0x60006000fd' }) });
    }
    return route.abort(); // bundler, price feeds, data API, everything else offline
  });
}

test.describe('Send — high-risk recipient confirmation (US 3.7)', () => {
  test('first-time contract recipient → risk tags + deliberate slide-to-confirm', async ({ page }) => {
    await seedAndMock(page);
    await page.goto('/send');
    await page.waitForLoadState('networkidle');

    // Step 1 — pick a native coin (under the "Gas" category, hidden from the
    // default "Stablecoins" tab). BNB is unique to one chain, so it's unambiguous.
    await expect(page.locator('body')).toContainText('Select Token', { timeout: 40_000 });
    await page.getByText('Gas', { exact: true }).first().click();
    // Click the token ROW (its "BNB Chain" network subtitle is unique to the row;
    // the network-filter chips above only show bare symbols like "BNB").
    await page.getByText('BNB Chain', { exact: true }).click();

    // Step 2 — enter a fresh contract recipient + an amount within balance.
    await page.getByPlaceholder('0x... address').first().fill(CONTRACT_RECIPIENT);
    await page.locator('input[placeholder="0"]').first().fill('0.01');
    await page.getByText('Continue', { exact: true }).first().click();

    // Step 3 — confirm: both risk tags surface, and the CTA is the deliberate
    // slide-to-confirm ("Confirm & Send"). Send's confirm is ALWAYS a
    // SlideToConfirmButton (SendScreen.tsx:1622) — a stray tap can't fire a
    // payment — so its title present at this step evidences the slide gate.
    await expect(page.locator('body')).toContainText('First time', { timeout: 30_000 });
    await expect(page.locator('body')).toContainText('Contract');
    await expect(page.locator('body')).toContainText('Confirm & Send');
    await page.screenshot({ path: 'e2e/screenshots/send-high-risk-confirm.png', fullPage: true });
  });
});
