/**
 * The pool under everything (spec 025 T117 — SC-102), hermetically.
 *
 * A user-configured primary answers 401 (an auth wall — the core's
 * permanent-ban class); the built-in default answers correctly. The read
 * must succeed via the pool's next choice, the ban must reach
 * `vela.rpc.banned` in the cross-client bytes, and after a reload the
 * banned endpoint must not receive one free attempt.
 */
import { expect, test } from '@playwright/test';
import { seedSignedIn } from './live-helpers';
import { denyOffOrigin, poolCall, readKv, seedNetworkOverrides, stubJsonRpc } from './stub-chain';

test.use({ viewport: { width: 390, height: 844 } });

const PRIMARY = 'https://stub-primary.test/rpc';

test.beforeEach(async ({ page }) => {
	await seedSignedIn(page);
	await page.addInitScript(() => window.localStorage.setItem('vela.dev.console', '1'));
	await denyOffOrigin(page);
});

test('a failing primary is routed around; its ban persists; no free retry after reload', async ({
	page
}) => {
	let primaryHits = 0;
	await page.route(PRIMARY, (route) => {
		primaryHits += 1;
		return route.fulfill({ status: 401, body: 'auth wall' });
	});
	// The chain-1 default (publicnode) answers like a healthy endpoint.
	await stubJsonRpc(page, /publicnode\.com/, (method) => {
		if (method === 'eth_chainId') return '0x1';
		if (method === 'eth_blockNumber') return '0x1234';
		return undefined;
	});
	// Every other candidate tier (the other public nodes, index fetches…) stays denied.

	await page.goto('/en/wallet');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await seedNetworkOverrides(page, [{ chainId: 1, rpcURL: PRIMARY }]);
	// Since spec 025 phase 3 the home fetches balances on load, so chain 1's
	// pool config is already collected by now — WITHOUT the override. A fresh
	// document re-collects with it (the same seed-then-reload the home test
	// uses); this is also how a real settings edit reaches the pool.
	await page.reload();
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();

	// The read succeeds despite the poisoned primary.
	const result = await poolCall(page, 'eth_blockNumber', [], 1);
	expect((result as { result?: unknown }).result).toBe('0x1234');
	expect(primaryHits).toBeGreaterThan(0);

	// The 401 verdict landed in the persisted ban map, cross-client bytes.
	await expect
		.poll(async () => {
			const raw = await readKv(page, 'vela.rpc.banned');
			return raw === null ? [] : (JSON.parse(raw) as Array<{ url: string }>).map((b) => b.url);
		})
		.toContain(PRIMARY);

	// A fresh document: the ban must be loaded BEFORE the first selection.
	const hitsBeforeReload = primaryHits;
	await page.reload();
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	const again = await poolCall(page, 'eth_blockNumber', [], 1);
	expect((again as { result?: unknown }).result).toBe('0x1234');
	expect(primaryHits).toBe(hitsBeforeReload);
});
