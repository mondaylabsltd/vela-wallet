/**
 * Playwright helpers for driving the parallel space + the local test dApp.
 *
 * The wallet is the REAL app under `/parallel/*` (fixed passkey, everything else real),
 * so the whole connect → request → approve → respond loop runs over the real transport.
 * We drive the wallet by its on-screen English text (no production testIDs — the app
 * stays pixel-identical) and assert round-trips from the dApp side, which the relay
 * serves with full testIDs + a `window.fire()` API.
 */
import { expect, type Page, type BrowserContext } from '@playwright/test';
import { startRelay } from './relay';

export type Relay = Awaited<ReturnType<typeof startRelay>>;

export const RELAY_PORT = 8791;

/** Fixture Safe addresses (see src/services/dev/passkey-fixture.ts). */
export const FIXTURE = {
  one: '0xD400866e00B055B20752a826CD5C89b811de130b',
  two: '0x031d7D57c99CAF891e1C250554691Fd12D84772b',
  three: '0x58cd0ce6A27099220543b31710d7860d75Ba1d3d',
};

/**
 * Stub every OUTBOUND call the wallet makes to an external RPC / bundler / price
 * service with a fast benign JSON-RPC reply, so a hermetic UI run never hangs on the
 * network or spends xDAI. Local app assets (:8081) and the relay (:PORT) pass through.
 */
export async function stubWalletNetwork(page: Page, _relayPort = RELAY_PORT): Promise<void> {
  await page.route('**/*', (route) => {
    const url = route.request().url();
    // The app (any port) and the local relay are both localhost; everything external
    // (RPC / bundler / price feeds) gets a fast benign reply so nothing hangs.
    const local =
      /^https?:\/\/(localhost|127\.0\.0\.1)(:\d+)?(\/|$)/.test(url) ||
      url.startsWith('data:') || url.startsWith('blob:');
    if (local) return route.continue();
    return route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ jsonrpc: '2.0', id: 1, result: null }),
    });
  });
}

/** Open the wallet's real Connect screen inside the parallel space. */
export async function openWalletConnect(page: Page): Promise<void> {
  await page.goto('/parallel/connect');
  await page.waitForLoadState('networkidle').catch(() => {});
  // The PARALLEL SPACE badge proves the mode is armed (fixed passkey + fixture wallet).
  await expect(page.getByTestId('parallel-space-badge')).toBeVisible({ timeout: 20_000 });
  // The paste field is the real ConnectScreen's link input.
  await expect(page.getByPlaceholder(/Paste/i)).toBeVisible({ timeout: 20_000 });
}

/** Paste a relay connect URL and wait for the wallet to show "Connected". */
export async function connectWallet(page: Page, connectUrl: string): Promise<void> {
  const input = page.getByPlaceholder(/Paste/i);
  await input.fill(connectUrl);
  await input.press('Enter');
  await expect(page.getByText('Connected', { exact: true })).toBeVisible({ timeout: 20_000 });
}

/** Open the served test dApp page bound to a specific relay session. */
export async function openTestDapp(context: BrowserContext, relay: Relay, session: { sessionId: string; nonce: string; secret: string }): Promise<Page> {
  const page = await context.newPage();
  await page.goto(`${relay.baseUrl}/?s=${session.sessionId}&n=${session.nonce}&k=${session.secret}`);
  await expect(page.getByTestId('dapp-relay-status')).toHaveText(/connected/i, { timeout: 15_000 });
  return page;
}

/** Fire a request from the dApp and return the wallet's response after `interact()`. */
export async function request(
  dapp: Page,
  method: string,
  params: unknown[],
  interact: () => Promise<void>,
): Promise<{ id: string; result?: unknown; error?: { code: number; message: string } }> {
  await dapp.evaluate(({ m, p }) => { (window as any).__pending = (window as any).fire(m, p); }, { m: method, p: params });
  await interact();
  return dapp.evaluate(() => (window as any).__pending);
}

/** Fire an instant read request (no wallet approval needed) and return the response. */
export async function requestInstant(dapp: Page, method: string, params: unknown[]): Promise<any> {
  return dapp.evaluate(({ m, p }) => (window as any).fire(m, p), { m: method, p: params });
}

/** Click a signing-sheet button by its exact English label. */
export async function clickSheetButton(wallet: Page, label: string): Promise<void> {
  await wallet.getByText(label, { exact: true }).last().click();
}

/**
 * Click the sheet's confirm affordance, whatever its label resolved to
 * (Confirm / Approve / Sign / Confirm <intent>). Skips Reject/Disconnect.
 */
export async function confirmSheet(wallet: Page): Promise<void> {
  for (const rx of [/^Confirm/, /^Approve$/, /^Sign$/]) {
    const btn = wallet.getByText(rx).last();
    if (await btn.isVisible().catch(() => false)) { await btn.click(); return; }
  }
  throw new Error('no confirm button visible on the signing sheet');
}

/** Read a fixture Safe's native (xDAI) balance on Gnosis, in wei. */
export async function gnosisBalanceWei(address: string): Promise<bigint> {
  const r = await fetch('https://rpc.gnosischain.com', {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'eth_getBalance', params: [address, 'latest'] }),
  });
  return BigInt((await r.json()).result ?? '0x0');
}
