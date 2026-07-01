/**
 * Clear-signing coverage through the REAL connect flow (Epic 5, US 5.2/5.3).
 *
 * Fires every production clear-signing scenario (the same catalog the /clear-signing-test
 * harness uses) from the test dApp, over the real transport, into the real Home →
 * Connections signing sheet in the parallel space. Breadth: every scenario renders and is
 * rejectable. Depth: spot-checks the key intents (Send, the never-unlimited spending-cap,
 * blind-sign warning, SIWE phishing mismatch, plain message).
 *
 * One connection is shared across the whole file (serial) so the 25 scenarios run fast.
 */
import { test, expect, type Page } from '@playwright/test';
import { startRelay } from './support/relay';
import {
  RELAY_PORT, type Relay,
  stubWalletNetwork, openWalletConnect, connectWallet, openTestDapp,
  request, clickSheetButton,
} from './support/parallel';
import { CLEAR_SIGNING_SCENARIOS } from '../src/screens/settings/clear-signing-scenarios';

test.describe.configure({ mode: 'serial' });

let relay: Relay;
let wallet: Page;
let dapp: Page;
let ctx: import('@playwright/test').BrowserContext;

test.beforeAll(async ({ browser }) => {
  relay = await startRelay({ port: RELAY_PORT + 2 });
  ctx = await browser.newContext();
  const session = relay.newSession();
  wallet = await ctx.newPage();
  await stubWalletNetwork(wallet);
  dapp = await openTestDapp(ctx, relay, session);
  await openWalletConnect(wallet);
  await connectWallet(wallet, session.connectUrl);
  await expect(dapp.getByTestId('dapp-wallet-status')).toHaveText(/connected/i, { timeout: 15_000 });
});

// Close the pages (ending their SSE streams) BEFORE stopping the relay.
test.afterAll(async () => { await ctx?.close().catch(() => {}); await relay?.stop(); });

/** Fire a scenario, wait for its signing sheet, reject it, and return the response. */
async function fireAndReject(sc: { request: { method: string; params: unknown[] } }) {
  return request(dapp, sc.request.method, sc.request.params, async () => {
    await expect(wallet.getByText('Reject', { exact: true })).toBeVisible({ timeout: 20_000 });
    await clickSheetButton(wallet, 'Reject');
  });
}

test('every clear-signing scenario renders through the real flow and is rejectable', async () => {
  test.setTimeout(180_000);
  for (const sc of CLEAR_SIGNING_SCENARIOS) {
    const resp = await fireAndReject(sc);
    expect(resp.error?.code, `scenario ${sc.id} should reject with 4001`).toBe(4001);
  }
});

// --- Depth: key intents render correctly (built-in ABI / approval-guard, no descriptor) ---

test('ERC-20 transfer clear-signs as a Send', async () => {
  const sc = CLEAR_SIGNING_SCENARIOS.find((s) => s.id === 'erc20-transfer')!;
  await request(dapp, sc.request.method, sc.request.params, async () => {
    await expect(wallet.getByText(/Send/i).first()).toBeVisible({ timeout: 20_000 });
    await clickSheetButton(wallet, 'Reject');
  });
});

test('an unlimited ERC-20 approve surfaces the spending-cap editor', async () => {
  const sc = CLEAR_SIGNING_SCENARIOS.find((s) => s.id === 'erc20-approve')!;
  await request(dapp, sc.request.method, sc.request.params, async () => {
    await expect(wallet.getByText('Spending cap')).toBeVisible({ timeout: 20_000 });
    await clickSheetButton(wallet, 'Reject');
  });
});

test('a genuinely blind transaction shows the Unknown / blind-sign warning', async () => {
  const sc = CLEAR_SIGNING_SCENARIOS.find((s) => s.id === 'blind-tx')!;
  await request(dapp, sc.request.method, sc.request.params, async () => {
    await expect(wallet.getByText(/Unknown|Unable to decode|blind|not.*decoded/i).first()).toBeVisible({ timeout: 20_000 });
    await clickSheetButton(wallet, 'Reject');
  });
});

test('a SIWE message with a mismatched domain is flagged', async () => {
  const sc = CLEAR_SIGNING_SCENARIOS.find((s) => s.id === 'siwe-phish')!;
  await request(dapp, sc.request.method, sc.request.params, async () => {
    // The message claims app.uniswap.org but the request origin is the test dApp.
    await expect(wallet.getByText(/uniswap\.org/i).first()).toBeVisible({ timeout: 20_000 });
    await clickSheetButton(wallet, 'Reject');
  });
});

test('a plain personal_sign shows the decoded message', async () => {
  const sc = CLEAR_SIGNING_SCENARIOS.find((s) => s.id === 'personal-sign')!;
  await request(dapp, sc.request.method, sc.request.params, async () => {
    await expect(wallet.getByText(/OpenSea/i).first()).toBeVisible({ timeout: 20_000 });
    await clickSheetButton(wallet, 'Reject');
  });
});
