/**
 * EIP-681 payment-request E2E.
 *
 * Covers the two user paths we built:
 *  - /pay bridge page (public, no wallet needed)
 *  - the locked Send flow that a scan / pay-link produces, including every
 *    parameter exception (unsupported network, unknown token, insufficient
 *    balance). We seed a wallet into localStorage and block external network so
 *    fetchTokens / metadata resolve fast and the exception paths are
 *    deterministic.
 *
 * Run: npx playwright test eip681-pay
 */
import { test, expect, type Page } from '@playwright/test';

const ME = '0x742d35Cc6634C0532925a3b844Bc454e4438f44e';
const USDC_POLYGON = '0x3c499c542cEF5E3811e1192ce70d8cc03d5c3359';
const DEAD = '0x000000000000000000000000000000000000dEaD';

async function ready(page: Page, text: string) {
  await expect(page.locator('body')).toContainText(text, { timeout: 40_000 });
}

test.describe('EIP-681 /pay bridge', () => {
  test('renders a valid native request', async ({ page }) => {
    await page.goto(`/pay?to=${ME}&chain=1&amount=2&sym=ETH&dec=18&net=Ethereum`);
    await ready(page, 'Payment request');
    const body = (await page.textContent('body')) ?? '';
    expect(body).toContain('2 ETH');
    expect(body).toContain('Ethereum');
    expect(body).toContain('Open in Vela Wallet');
    expect(body).toContain('Pay with another wallet');
    await page.screenshot({ path: 'e2e/screenshots/pay-request.png', fullPage: true });
  });

  test('QR can switch between EIP-681 and plain address', async ({ page }) => {
    await page.goto(`/pay?to=${ME}&chain=137&token=${USDC_POLYGON}&amount=5&sym=USDC&dec=6&net=Polygon`);
    await ready(page, '5 USDC');
    await page.getByText('Pay with another wallet').click();
    await ready(page, 'Scan with an EIP-681 wallet');
    await page.screenshot({ path: 'e2e/screenshots/pay-other-eip681.png', fullPage: true });
    await page.getByText('Address', { exact: true }).click();
    await ready(page, 'Any wallet can scan');
    await page.screenshot({ path: 'e2e/screenshots/pay-other-address.png', fullPage: true });
  });

  test('rejects an invalid link', async ({ page }) => {
    await page.goto(`/pay?to=notanaddress&chain=1`);
    await ready(page, 'Invalid payment link');
  });
});

test.describe('Locked Send — pay-link / scan result + exceptions', () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript((me) => {
      localStorage.setItem(
        'vela.accounts',
        JSON.stringify([{ id: 'e2e', name: 'E2E', address: me, createdAt: '2026-01-01T00:00:00.000Z' }]),
      );
      localStorage.setItem('vela.activeAccountIndex', '0');
    }, ME);
    // Block external requests so token/metadata lookups resolve fast (empty),
    // making the exception branches deterministic. The app bundle is local.
    await page.route('**/*', (route) => {
      const h = new URL(route.request().url()).hostname;
      if (h === 'localhost' || h === '127.0.0.1') route.continue();
      else route.abort();
    });
  });

  test('unsupported network → offers to add it', async ({ page }) => {
    await page.goto(`/send?prefilledRecipient=${ME}&prefilledChainId=123456&locked=1`);
    await ready(page, 'Network not supported');
    expect(await page.textContent('body')).toContain('Add this network');
    await page.screenshot({ path: 'e2e/screenshots/send-unsupported-network.png', fullPage: true });
  });

  test('unknown token contract → error', async ({ page }) => {
    await page.goto(`/send?prefilledRecipient=${ME}&prefilledChainId=1&prefilledTokenAddress=${DEAD}&locked=1`);
    await ready(page, 'Unknown token');
    await page.screenshot({ path: 'e2e/screenshots/send-unknown-token.png', fullPage: true });
  });

  test('insufficient balance → warns and blocks', async ({ page }) => {
    await page.goto(`/send?prefilledRecipient=${ME}&prefilledChainId=1&prefilledAmountBase=1000000000000000000&locked=1`);
    await ready(page, 'You do not have enough ETH');
    await page.screenshot({ path: 'e2e/screenshots/send-insufficient-balance.png', fullPage: true });
  });
});

test.describe('Receive — Save uses the OS share sheet on web', () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript((me) => {
      localStorage.setItem(
        'vela.accounts',
        JSON.stringify([{ id: 'e2e', name: 'E2E', address: me, createdAt: '2026-01-01T00:00:00.000Z' }]),
      );
      localStorage.setItem('vela.activeAccountIndex', '0');
      // Record share-sheet invocations so we can assert Save routes to it.
      (window as unknown as { __shared: string[][] }).__shared = [];
      (navigator as unknown as { canShare: () => boolean }).canShare = () => true;
      (navigator as unknown as { share: (d: { files?: File[] }) => Promise<void> }).share = async (d) => {
        (window as unknown as { __shared: string[][] }).__shared.push((d.files || []).map((f) => f.type));
      };
    }, ME);
    await page.route('**/*', (route) => {
      const h = new URL(route.request().url()).hostname;
      if (h === 'localhost' || h === '127.0.0.1') route.continue();
      else route.abort();
    });
  });

  test('Save invokes navigator.share with a PNG (→ iOS "Save Image" to Photos)', async ({ page }) => {
    await page.goto('/receive');
    await page.getByText('I Understand').click({ timeout: 40_000 });
    await page.waitForTimeout(1500); // let the share image pre-render
    await page.getByText('Save image').click();
    await page.waitForTimeout(600);
    const shared = await page.evaluate(() => (window as unknown as { __shared: string[][] }).__shared);
    expect(shared.length).toBeGreaterThan(0);
    expect(shared[0]).toContain('image/png');
  });
});
