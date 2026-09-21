/**
 * A tab closed mid-flight (spec 026 T237 — SC-204).
 *
 * The promise: a submitted operation is durable the moment the relay accepts
 * it, and reopening the wallet picks it up and settles it. Nothing about that
 * depends on the send screen still being open — the record is written before
 * any receipt exists, and the tracker's recovery sweep is what runs on the
 * next boot.
 *
 * This is the failure a person meets by simply closing a tab, and the reason
 * the persist-then-track ordering is an invariant rather than a convention.
 */
import { expect, test } from '@playwright/test';
import { en } from './live-helpers';
import { denyOffOrigin, happyRelay, readKv, stubRelay } from './stub-chain';

test.use({ viewport: { width: 390, height: 844 } });
test.setTimeout(90_000);

// The app asks the Cloudflare relay (`vela-relay-cf`, endpoints.ts since spec 060);
// the older host is kept so a stub written against either still intercepts.
const RELAY = /vela-relay(-cf)?\.getvela\.app/;
const USER_OP_HASH = '0x' + 'c3'.repeat(32);
const TX_HASH = '0x' + 'd4'.repeat(32);
const SAFE = '0xD400866e00B055B20752a826CD5C89b811de130b';
/**
 * Fixture key #1. The address is DERIVED from it (spec 019 invariant ②), so
 * seeding this key is the only way to make the stored record's account and the
 * wallet's own account the same wallet.
 */
const FIXTURE_PUBLIC_KEY =
	'04197db9030a1e166bec2cee05e0ddb94b26ee0b6d6f429f1748cda4eedac36f04fe546861a9c9dfaf75719b53c75e0b933d4aad6d325f18c75776a260d507647b';

/** The record the last process left behind: accepted, no hash yet. */
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

test('an operation submitted before the tab closed is picked up and settled on reopen', async ({
	page
}) => {
	let landed = false;
	await denyOffOrigin(page);
	// No chain stub: this test is about the relay and the store, and
	// `denyOffOrigin` already makes every other read fail fast — which is
	// itself part of the point, since none of them may change the verdict.
	await stubRelay(
		page,
		RELAY,
		happyRelay(USER_OP_HASH, TX_HASH, () => (landed ? 'landed' : 'pending'))
	);

	// A wallet with a pending record and nothing else — exactly what a closed
	// tab leaves behind.
	await page.addInitScript(
		([record, safe, key]) => {
			localStorage.setItem('vela.intro.seen', String(Date.now()));
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
			const open = indexedDB.open('vela', 1);
			open.onupgradeneeded = () => open.result.createObjectStore('kv');
			open.onsuccess = () => {
				open.result
					.transaction('kv', 'readwrite')
					.objectStore('kv')
					.put(JSON.stringify([record]), 'vela.transactionHistory');
			};
		},
		[PENDING_RECORD, SAFE, FIXTURE_PUBLIC_KEY] as const
	);

	await page.goto('/en/wallet');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	// The wallet is the one the record belongs to — the address is derived from
	// the seeded key, never read from the stored field.
	await expect(page.getByText('0xD40086…de130b', { exact: false }).first()).toBeVisible();

	// The feed shows it as pending — the money left, and the wallet says so
	// without any screen having stayed open.
	await expect(page.getByText(en('history.labelSent')).first()).toBeVisible({ timeout: 20_000 });
	expect(await readKv(page, 'vela.transactionHistory')).toContain('"status":"pending"');

	// The chain lands it. The tracker is already polling — no screen, no tap.
	landed = true;
	await expect
		.poll(async () => (await readKv(page, 'vela.transactionHistory')) ?? '', { timeout: 45_000 })
		.toContain('"status":"confirmed"');
	expect(await readKv(page, 'vela.transactionHistory')).toContain(TX_HASH);
});
