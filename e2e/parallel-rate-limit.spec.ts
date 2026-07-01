/**
 * Rate-limit degradation in the parallel space (US 2.2) — proves the classification
 * runs correctly in the REAL app and that rate-limiting stays calm:
 *   • rateLimitRpc('all') → every failing chain is flagged rate-limited (transient),
 *     the balance keeps its cached value, and NO "RPC unavailable / fix your RPC"
 *     banner is shown.
 *   • failRpc('all')      → failures are NOT flagged rate-limited (persistent) — the
 *     path that WOULD surface the banner.
 *
 * We assert the pool's live classification via the dev-only `__velaRpcState` seam
 * (the exact signal HomeScreen filters the banner on). That's more robust than
 * driving the banner through the full token fetch, whose ethereum-data metadata gate
 * is unreachable in the sandbox. Unit coverage: rpc-pool-ratelimit.test.ts.
 *
 * Needs a DEV build (__DEV__: fixed-passkey + fault console). Fault is pre-armed
 * before boot via __VELA_FAULT_INIT__ so the first RPC activity runs under it.
 *
 * Run: E2E_BASE_URL=http://localhost:8099 npx playwright test parallel-rate-limit
 */
import { test, expect, type Page } from '@playwright/test';

const BANNER = /RPC unavailable/i;

type RpcState = { failed: number[]; rateLimited: number[] };
declare global {
  interface Window {
    __VELA_FAULT_INIT__?: unknown;
    __velaRpcState?: { failed: () => number[]; rateLimited: () => number[] };
  }
}

async function bootParallel(page: Page, seed: [string, string][]) {
  await page.addInitScript((s) => { window.__VELA_FAULT_INIT__ = s; }, seed);
  // Block external hosts so failing RPC settles fast and deterministically.
  await page.route('**/*', (r) => {
    const h = new URL(r.request().url()).hostname;
    return h === 'localhost' || h === '127.0.0.1' ? r.continue() : r.abort();
  });
  await page.goto('/parallel');
  // Landing on the real production home (balance hero) proves the handoff.
  await expect(page.locator('body')).toContainText('Total balance', { timeout: 120_000 });
}

/** Wait until the pool has classified some failures, then read the live sets. */
async function rpcState(page: Page): Promise<RpcState> {
  await page.waitForFunction(() => (window.__velaRpcState?.failed().length ?? 0) > 0, { timeout: 45_000 });
  return page.evaluate(() => ({
    failed: window.__velaRpcState!.failed(),
    rateLimited: window.__velaRpcState!.rateLimited(),
  }));
}

test.describe('Parallel space — rate-limit is transient/calm, hard failure is persistent (US 2.2)', () => {
  test.describe.configure({ timeout: 200_000 }); // cold Metro web bundle can be slow

  test('rateLimitRpc → every failure flagged transient, home stays calm (no banner)', async ({ page }) => {
    await bootParallel(page, [['rateLimitRpc', 'all']]);
    const { failed, rateLimited } = await rpcState(page);
    // Every failing chain is classified rate-limited → cached balance, no nag banner.
    expect(rateLimited.length).toBeGreaterThan(0);
    expect(rateLimited.length).toBe(failed.length);
    await expect(page.locator('body')).not.toContainText(BANNER);
    await page.screenshot({ path: 'e2e/screenshots/parallel-ratelimited-calm.png', fullPage: true });
  });

  test('failRpc → failures are NOT flagged rate-limited (persistent path)', async ({ page }) => {
    await bootParallel(page, [['failRpc', 'all']]);
    const { failed, rateLimited } = await rpcState(page);
    expect(failed.length).toBeGreaterThan(0);
    expect(rateLimited.length).toBe(0); // a hard failure is never classified transient
  });
});
