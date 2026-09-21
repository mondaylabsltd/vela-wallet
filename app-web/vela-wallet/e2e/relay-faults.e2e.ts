/**
 * When the relay misbehaves (spec 026 T237 — SC-205).
 *
 * Four failures a person can actually meet, each injected through the fault
 * console and each checked for the same property: the wallet says something a
 * person can act on, and never shows the relay's own words. A raw
 * `-32521 user operation reverted during simulation` on a confirm screen is
 * not an error message, it is a leak.
 *
 * The faults are planted BEFORE the app boots (`__VELA_FAULT_INIT__`), so the
 * very first read already runs under them.
 */
import { expect, test } from '@playwright/test';
import { en } from './live-helpers';
import { denyOffOrigin, happyRelay, readKv, stubRelay } from './stub-chain';

test.use({ viewport: { width: 390, height: 844 } });
test.setTimeout(90_000);

// The app asks the Cloudflare relay (`vela-relay-cf`, endpoints.ts since spec 060);
// the older host is kept so a stub written against either still intercepts.
const RELAY = /vela-relay(-cf)?\.getvela\.app/;
const USER_OP_HASH = '0x' + 'e5'.repeat(32);
const TX_HASH = '0x' + 'f6'.repeat(32);
const SAFE = '0xD400866e00B055B20752a826CD5C89b811de130b';
const FIXTURE_PUBLIC_KEY =
	'04197db9030a1e166bec2cee05e0ddb94b26ee0b6d6f429f1748cda4eedac36f04fe546861a9c9dfaf75719b53c75e0b933d4aad6d325f18c75776a260d507647b';

const PENDING_RECORD = {
	id: USER_OP_HASH,
	userOpHash: USER_OP_HASH,
	txHash: '',
	from: SAFE,
	to: '0x' + 'ab'.repeat(20),
	value: '0.5',
	symbol: 'ETH',
	decimals: 18,
	chainId: 1,
	timestamp: Math.floor(Date.now() / 1000),
	status: 'pending',
	type: 'send'
};

/** A signed-in wallet whose address is derived from the fixture key. */
async function seedWallet(page: import('@playwright/test').Page, faults: [string, unknown][]) {
	await page.addInitScript(
		([record, safe, key, planted]) => {
			localStorage.setItem('vela.intro.seen', String(Date.now()));
			localStorage.setItem('vela.dev.console', '1');
			localStorage.setItem(
				'vela.accounts',
				JSON.stringify([
					{
						id: 'e2e-credential-id',
						name: 'E2E Wallet',
						address: safe,
						public_key_hex: key,
						created_at_iso: '2026-01-01T00:00:00.000Z',
						keys: []
					}
				])
			);
			localStorage.setItem('vela.activeAccountIndex', '0');
			(window as unknown as { __VELA_FAULT_INIT__: unknown }).__VELA_FAULT_INIT__ = planted;
			const open = indexedDB.open('vela', 1);
			open.onupgradeneeded = () => open.result.createObjectStore('kv');
			open.onsuccess = () => {
				open.result
					.transaction('kv', 'readwrite')
					.objectStore('kv')
					.put(JSON.stringify([record]), 'vela.transactionHistory');
			};
		},
		[PENDING_RECORD, SAFE, FIXTURE_PUBLIC_KEY, faults] as const
	);
}

test('a silent receipt leaves the payment submitted — never turned into a failure', async ({
	page
}) => {
	await denyOffOrigin(page);
	await stubRelay(
		page,
		RELAY,
		happyRelay(USER_OP_HASH, TX_HASH, () => 'landed')
	);
	// The relay accepted it and the chain would confirm it — but the receipt
	// never comes back. The record must stay pending: a slow poll is not a
	// verdict, and a person whose money left must not be told it failed.
	await seedWallet(page, [['silentReceipt', 1]]);

	await page.goto('/en/wallet');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await expect(page.getByText(en('history.labelSent')).first()).toBeVisible({ timeout: 20_000 });

	await page.waitForTimeout(12_000);
	const store = await readKv(page, 'vela.transactionHistory');
	expect(store).toContain('"status":"pending"');
	expect(store).not.toContain('"status":"failed"');
	expect(store).not.toContain('"status":"confirmed"');
});

test('an unreachable relay is quiet, not alarming — the record stays as it was', async ({
	page
}) => {
	await denyOffOrigin(page);
	await stubRelay(
		page,
		RELAY,
		happyRelay(USER_OP_HASH, TX_HASH, () => 'landed')
	);
	await seedWallet(page, [['failRelay', 1]]);

	await page.goto('/en/wallet');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await expect(page.getByText(en('history.labelSent')).first()).toBeVisible({ timeout: 20_000 });

	await page.waitForTimeout(10_000);
	// Unreachable is an honest unknown (the tracker's rule), so nothing moves.
	expect(await readKv(page, 'vela.transactionHistory')).toContain('"status":"pending"');
	// And no relay text reached the screen.
	const text = await page.evaluate(() => document.body.innerText);
	expect(text).not.toMatch(/-32\d{3}|jsonrpc|bundler|relay unreachable/i);
});

test('the fault console is not reachable without its gate', async ({ page }) => {
	await denyOffOrigin(page);
	await page.addInitScript(() => {
		localStorage.setItem('vela.intro.seen', String(Date.now()));
	});
	await page.goto('/en/wallet');
	await page.waitForLoadState('networkidle');
	expect(
		await page.evaluate(
			() => (window as unknown as { vela?: Record<string, unknown> }).vela?.failRelay !== undefined
		)
	).toBe(false);
});
