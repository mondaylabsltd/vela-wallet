/**
 * Every dApp interaction in Activity (spec 093), on the real page and through
 * the real core: a swap, a Permit2 permit and a Sign-In with Ethereum — stored
 * as the signing path now writes them — read by the `activity_feed` machine
 * and drawn by the wallet. The titles, the second lines, the allowance in red
 * and the detail's facts are the core's words; the stored request appears only
 * once "Technical details" is opened.
 *
 * The drawn screens land in `e2e/__screenshots__/093/` (phone and desktop
 * width, en and zh) for the device pass to compare.
 */
import { expect, test, type Page } from '@playwright/test';
import { CHAINS } from '../src/lib/services/chains';
import type { LocalTransaction } from '../src/lib/services/transactions-model';
import { dappActivityRecords } from '../src/lib/wallet/dapp-activity-fixtures';
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
/** The address the core DERIVES from this key (spec 019 invariant ②) — the
 *  records are the account's only when their `from` is that address. */
const SAFE = '0xD400866e00B055B20752a826CD5C89b811de130b';
const FIXTURE_PUBLIC_KEY =
	'04197db9030a1e166bec2cee05e0ddb94b26ee0b6d6f429f1748cda4eedac36f04fe546861a9c9dfaf75719b53c75e0b933d4aad6d325f18c75776a260d507647b';
const SHOTS = 'e2e/__screenshots__/093';

/** Alice, in the address book — the contact whose page the last test opens. */
const ALICE = '0x' + 'a1'.repeat(20);
const USDC = '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48';

/**
 * What a dApp's token transfer to Alice leaves behind: a `transfer(Alice, 5
 * USDC)` called on the token, as the signing path stores it.
 */
function transferToAlice(nowSec: number): LocalTransaction {
	const data =
		'0xa9059cbb' + ALICE.slice(2).padStart(64, '0') + (5_000_000).toString(16).padStart(64, '0');
	return {
		id: `dapp-${nowSec * 1000}-tx`,
		userOpHash: '0x' + 'e3'.repeat(32),
		txHash: '0x' + 'f3'.repeat(32),
		from: SAFE,
		to: USDC,
		value: '0x0',
		symbol: 'ETH',
		decimals: 18,
		chainId: 1,
		timestamp: nowSec,
		status: 'confirmed',
		type: 'dapp_tx',
		dappOrigin: 'https://pay.example',
		dappUrl: 'https://pay.example',
		intent: 'Transfer',
		signedRequest: { method: 'eth_sendTransaction', params: [{ to: USDC, data }] },
		requestTruncated: false,
		dappSummary: { action: 'call', calls: 1, contract: USDC },
		balanceChanges: [
			{
				type: 'erc20_trusted',
				token: USDC,
				delta: '-5000000',
				symbol: 'USDC',
				decimals: 6,
				in_trusted_set: true
			}
		]
	};
}

async function seed(page: Page, records: LocalTransaction[], contacts: unknown[] = []) {
	await page.addInitScript(
		([rows, book, safe, key]) => {
			localStorage.setItem('vela.intro.seen', String(Date.now()));
			if (localStorage.getItem('vela.accounts') === null) {
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
					const store = open.result.transaction('kv', 'readwrite').objectStore('kv');
					store.put(JSON.stringify(rows), 'vela.transactionHistory');
					if (book.length > 0) store.put(JSON.stringify(book), 'vela.contacts');
				};
			}
		},
		[records, contacts, SAFE, FIXTURE_PUBLIC_KEY] as const
	);
}

test.beforeEach(async ({ page }) => {
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
			// Zero holdings everywhere: the activity rows are the seeded records.
			const call = params[0] as { data?: string } | undefined;
			const n = call?.data ? aggregate3CallCount(call.data) : 0;
			const data = '0x' + abiWord(0) + abiWord(0) + abiWord(0) + abiWord(0) + abiWord(0);
			return encodeAggregate3Result(Array.from({ length: n }, () => ({ success: true, data })));
		}
		return undefined;
	});
});

async function openWallet(page: Page, locale = 'en'): Promise<void> {
	await page.goto(`/${locale}/wallet`);
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await seedNetworkOverrides(
		page,
		CHAINS.map((c) => ({ chainId: c.chainId, rpcURL: `${STUB}/${c.chainId}` }))
	);
	await page.reload();
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	// The hero has read every chain (zero holdings): the page has settled.
	await expect(page.getByText('$0', { exact: true }).first()).toBeVisible({ timeout: 20_000 });
}

const TITLES = ['Swap on Uniswap', 'Spending permit on Uniswap', 'Sign in on app.uniswap.org'];

const fixtures = (page: Page) =>
	seed(page, dappActivityRecords(SAFE, Math.floor(Date.now() / 1000) - 60));

test.describe('phone width', () => {
	test.use({ viewport: { width: 390, height: 844 } });
	test.beforeEach(({ page }) => fixtures(page));

	test('the three rows say what happened and where; the permit opens to its facts', async ({
		page
	}) => {
		await openWallet(page);
		for (const title of TITLES) {
			await expect(page.getByRole('button', { name: new RegExp(title) }).first()).toBeVisible({
				timeout: 20_000
			});
		}
		// The swap: what left (an estimate) and what came back.
		await expect(page.getByText('≈ −100', { exact: true })).toBeVisible();
		await expect(page.getByText('≈ +0.03', { exact: true })).toBeVisible();
		// The permit: no money moved, the allowance where a figure would be.
		await expect(
			page.getByText(en('componentsUi.signingApprove.unlimitedValue'), { exact: true }).first()
		).toBeVisible();
		await page.screenshot({ path: `${SHOTS}/activity-phone-en.png`, fullPage: true });

		await page
			.getByRole('button', { name: /Spending permit on Uniswap/ })
			.first()
			.click();
		const sheet = page.getByRole('dialog');
		await expect(sheet.getByText(en('connect.detail.offChainNote'))).toBeVisible();
		await expect(sheet.getByText(en('componentsUi.signingApprove.noExpiry'))).toBeVisible();
		// The stored request is not on the screen until it is asked for.
		await expect(sheet.getByText(/"primaryType"/)).toHaveCount(0);
		await sheet.getByRole('button', { name: en('componentsUi.signing.advancedToggle') }).click();
		await expect(sheet.getByText('PermitSingle', { exact: true })).toBeVisible();
		// The core's reading of the kept request: the typed document, pretty.
		await expect(sheet.getByText(/"primaryType": "PermitSingle"/)).toBeVisible();
		await sheet.getByText(en('componentsTx.detail.labelOperation')).scrollIntoViewIfNeeded();
		await page.screenshot({ path: `${SHOTS}/permit-detail-phone-en.png` });
	});

	test('the swap opens to what left, what came back, and its balance changes', async ({ page }) => {
		await openWallet(page);
		await page
			.getByRole('button', { name: /Swap on Uniswap/ })
			.first()
			.click({ timeout: 20_000 });
		const sheet = page.getByRole('dialog');
		await expect(sheet.getByText('≈ −100 USDC', { exact: true }).first()).toBeVisible();
		await expect(sheet.getByText('≈ +0.03 ETH', { exact: true }).first()).toBeVisible();
		await expect(sheet.getByText(en('componentsUi.signing.balanceChangesTitle'))).toBeVisible();
		// The router a swap called is its Contract — a noun, never "Interacting with".
		await expect(sheet.getByText(en('tokenDetail.labelContract'), { exact: true })).toBeVisible();
		await expect(sheet.getByText(en('connect.detail.offChainNote'))).toHaveCount(0);
		await sheet.getByText(en('componentsUi.signing.balanceChangesTitle')).scrollIntoViewIfNeeded();
		// The sheet's entrance has to finish before the picture is worth taking.
		await page.waitForTimeout(800);
		await page.screenshot({ path: `${SHOTS}/swap-detail-phone-en.png` });
	});

	test('the same rows in Chinese', async ({ page }) => {
		await openWallet(page, 'zh');
		for (const title of ['在 Uniswap 兑换', '在 Uniswap 授权签名', '在 app.uniswap.org 登录']) {
			await expect(page.getByRole('button', { name: new RegExp(title) }).first()).toBeVisible({
				timeout: 20_000
			});
		}
		await page.screenshot({ path: `${SHOTS}/activity-phone-zh.png`, fullPage: true });
	});
});

test.describe('desktop width', () => {
	test.use({ viewport: { width: 1440, height: 900 } });
	test.beforeEach(({ page }) => fixtures(page));

	test('the rows and the permit’s technical details in the third column', async ({ page }) => {
		await openWallet(page);
		for (const title of TITLES) {
			await expect(page.getByRole('button', { name: new RegExp(title) }).first()).toBeVisible({
				timeout: 20_000
			});
		}
		await page.screenshot({ path: `${SHOTS}/activity-desktop-en.png` });

		await page
			.getByRole('button', { name: /Spending permit on Uniswap/ })
			.first()
			.click();
		await expect(page.getByText(en('connect.detail.offChainNote'))).toBeVisible();
		await page
			.getByRole('button', { name: en('componentsUi.signing.advancedToggle') })
			.first()
			.click();
		await expect(page.getByText('PermitSingle', { exact: true })).toBeVisible();
		await page.screenshot({ path: `${SHOTS}/permit-detail-desktop-en.png` });
	});
});

/**
 * Spec 093: a contact's 最近往来 is the core's rows for that contact, drawn by
 * Activity's builder — a dApp's token transfer to Alice reads as the verb it
 * was ("Transfer …"), never the page's own "Sent".
 */
test.describe('a contact page', () => {
	test.use({ viewport: { width: 390, height: 844 } });
	test.beforeEach(({ page }) =>
		seed(
			page,
			[transferToAlice(Math.floor(Date.now() / 1000) - 60)],
			[
				{
					address: ALICE,
					name: 'Alice',
					kind: 'unknown',
					favorite: false,
					txCount: 1,
					lastUsed: Date.now(),
					firstSeen: Date.now(),
					source: 'manual'
				}
			]
		)
	);

	test("a dApp's transfer to Alice reads as its verb on her page", async ({ page }) => {
		await page.goto('/en/contacts');
		await expect(page.getByRole('heading', { name: en('contacts.title') }).first()).toBeVisible();
		await page.getByText('Alice', { exact: true }).first().click();
		await expect(page.getByText(en('contacts.recentActivity')).first()).toBeVisible();
		const verb = en('componentsUi.signing.intentTransfer');
		await expect(page.getByText(new RegExp(`^${verb}`)).first()).toBeVisible({ timeout: 20_000 });
		await expect(page.getByText(en('history.labelSent'), { exact: true })).toHaveCount(0);
		await expect(page.getByText('≈ \u22125', { exact: true })).toBeVisible();
		await page.waitForTimeout(400);
		await page.screenshot({ path: `${SHOTS}/contact-detail-phone-en.png`, fullPage: true });
	});
});
