/**
 * A transaction detail belongs to the account it was opened on (issue 213).
 *
 * The A2 / DA2 screens arrive PRERENDERED with a drawn transaction inside them
 * — the mocks' "+120 USDT received, from 0x9F3c…21aE" — and the live layer only
 * replaced it when the tapped record resolved against the open account's feed.
 * Switching accounts re-points that feed under an open detail, so the drawn
 * receipt stood in for a record the new account never had: a Transaction
 * Details panel showing 120 USDT over an empty Activity list.
 *
 * Driven at desktop width, where it was reported: the third column sits BESIDE
 * the home, so the header's account button is still there to switch with. The
 * phone pushes A2 over the home instead and the switcher goes with it, so the
 * same journey has no phone shape — the phone's half of the rule (no record, no
 * sheet) is pinned in `live-detail.test.ts` instead.
 *
 * Chain access is stubbed; nothing leaves the machine.
 */
import { expect, test, type Page } from '@playwright/test';
import { CHAINS } from '../src/lib/services/chains';
import { en } from './live-helpers';
import {
	abiWord,
	aggregate3CallCount,
	denyOffOrigin,
	encodeAggregate3Result,
	seedNetworkOverrides,
	stubChainRegistry,
	stubJsonRpc
} from './stub-chain';

test.setTimeout(120_000);

const STUB = 'https://stub-rpc.test/rpc';
/** The address the core DERIVES from this key (spec 019 invariant ②) — a
 *  record is this account's only when its `from` is that address, so the pair
 *  has to agree. Borrowed from `activity-delete`. */
const SAFE = '0xD400866e00B055B20752a826CD5C89b811de130b';
const FIXTURE_PUBLIC_KEY =
	'04197db9030a1e166bec2cee05e0ddb94b26ee0b6d6f429f1748cda4eedac36f04fe546861a9c9dfaf75719b53c75e0b933d4aad6d325f18c75776a260d507647b';

const FUNDED = {
	id: 'e2e-credential-funded',
	name: 'Funded Wallet',
	address: SAFE,
	public_key_hex: FIXTURE_PUBLIC_KEY,
	created_at_iso: '2026-01-01T00:00:00.000Z',
	keys: []
};
/** The reporter's second account: signed in, and with nothing in its feed. */
const QUIET = {
	id: 'e2e-credential-quiet',
	name: 'hold on',
	address: '0x24fb1f4e2b9c7a5d8e3f6a1b4c7d9e2f5a8b1d2e',
	public_key_hex: '04' + 'cd'.repeat(64),
	created_at_iso: '2026-01-02T00:00:00.000Z',
	keys: []
};

const TX_HASH = '0x' + 'f6'.repeat(32);
const RECORD = {
	id: TX_HASH,
	userOpHash: '0x' + 'e5'.repeat(32),
	txHash: TX_HASH,
	from: SAFE,
	to: '0x' + 'ab'.repeat(20),
	value: '0.5',
	symbol: 'ETH',
	decimals: 18,
	chainId: 1,
	timestamp: Math.floor(Date.now() / 1000) - 60,
	status: 'confirmed',
	type: 'send'
};

/** What the drawn A2 / DA2 fixture says, and must never say on a live account. */
const DRAWN_AMOUNT = '+120 USDT';
const DRAWN_COUNTERPARTY = '0x9F3c…21aE';

async function seedTwoAccountsOneRecord(page: Page): Promise<void> {
	await page.addInitScript(
		([record, funded, quiet]) => {
			localStorage.setItem('vela.intro.seen', String(Date.now()));
			if (localStorage.getItem('vela.accounts') !== null) return;
			localStorage.setItem('vela.accounts', JSON.stringify([funded, quiet]));
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
		[RECORD, FUNDED, QUIET] as const
	);
}

test.beforeEach(async ({ page }) => {
	await seedTwoAccountsOneRecord(page);
	await denyOffOrigin(page);
	await stubChainRegistry(page, {
		1: {
			chainId: 1,
			name: 'Ethereum',
			nativeCurrency: { name: 'Ether', symbol: 'ETH', decimals: 18 },
			stables: [],
			wrappedNativeToken: null,
			dex: null,
			rpc: [`${STUB}/1`]
		}
	});
	await stubJsonRpc(page, /stub-rpc\.test\/rpc\/(\d+)/, (method, params, url) => {
		const chainId = Number(/\/rpc\/(\d+)/.exec(url)?.[1]);
		if (method === 'eth_chainId') return '0x' + chainId.toString(16);
		if (method === 'eth_blockNumber') return '0x10';
		if (method === 'eth_getLogs') return [];
		if (method === 'eth_call') {
			// Every chain answers zero holdings: the seeded send is the only row.
			const call = params[0] as { data?: string } | undefined;
			const n = call?.data ? aggregate3CallCount(call.data) : 0;
			const data = '0x' + abiWord(0) + abiWord(0) + abiWord(0) + abiWord(0) + abiWord(0);
			return encodeAggregate3Result(Array.from({ length: n }, () => ({ success: true, data })));
		}
		return undefined;
	});
});

async function openHomeOnFunded(page: Page): Promise<void> {
	await page.goto('/en/wallet');
	await expect(page.getByText(FUNDED.name).first()).toBeVisible();
	await seedNetworkOverrides(
		page,
		CHAINS.map((c) => ({ chainId: c.chainId, rpcURL: `${STUB}/${c.chainId}` }))
	);
	await page.reload();
	await expect(page.getByText(FUNDED.name).first()).toBeVisible();
}

/** Open the funded account's one record, then switch to the quiet account. */
async function openRecordThenSwitch(page: Page): Promise<void> {
	const row = page.getByRole('button', { name: new RegExp(en('history.labelSent')) }).first();
	await expect(row).toBeVisible({ timeout: 20_000 });
	await row.click();

	// The detail really is showing the funded account's record before the switch.
	await expect(page.getByText(en('history.deleteRecord'))).toBeVisible();
	await expect(page.getByText(DRAWN_AMOUNT)).toHaveCount(0);

	await page
		.getByRole('button', { name: new RegExp(FUNDED.name) })
		.first()
		.click();
	await page
		.getByRole('dialog')
		.getByRole('button', { name: new RegExp(QUIET.name) })
		.click();
	await expect(page.getByText(QUIET.name).first()).toBeVisible();
}

/** Whichever layout drew it, the drawn transaction is nowhere on the page. */
async function expectNoDrawnTransaction(page: Page): Promise<void> {
	await expect(page.getByText(DRAWN_AMOUNT)).toHaveCount(0);
	await expect(page.getByText(DRAWN_COUNTERPARTY)).toHaveCount(0);
	await expect(page.getByText(en('history.deleteRecord'))).toHaveCount(0);
	// The record that WAS open belonged to the other account: it is gone too.
	await expect(page.getByText('-0.5', { exact: true })).toHaveCount(0);
}

test.describe('desktop — the third column', () => {
	// `BREAKPOINT_DESKTOP` is 1280; 1440 is the board's width.
	test.use({ viewport: { width: 1440, height: 900 } });

	test('switching accounts under an open detail shows no drawn transaction', async ({ page }) => {
		await openHomeOnFunded(page);
		await openRecordThenSwitch(page);
		await expectNoDrawnTransaction(page);
	});
});
