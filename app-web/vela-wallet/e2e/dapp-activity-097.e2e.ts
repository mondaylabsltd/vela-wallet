/**
 * Spec 097 part B — Activity says what happened, with the figures the chain
 * proved, on the real page and through the real core. The rows are the second
 * real-money pass's (BNB Chain), stored as this shell stores them once the
 * tracker has closed them: a USDC→BNB swap that sent no coin, an Aave borrow
 * whose USDC the scan also recorded, a withdraw the network refused, a 1inch
 * order (a contract the sheet's reading named) and an approval of Permit2.
 *
 * The drawn screens land in `e2e/__screenshots__/097/` (phone and desktop
 * width, en and zh) for the device pass to compare.
 */
import { expect, test, type Page } from '@playwright/test';
import { CHAINS } from '../src/lib/services/chains';
import type { LocalTransaction } from '../src/lib/services/transactions-model';
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
/** The address the core derives from this key — the records are the account's. */
const SAFE = '0xD400866e00B055B20752a826CD5C89b811de130b';
const FIXTURE_PUBLIC_KEY =
	'04197db9030a1e166bec2cee05e0ddb94b26ee0b6d6f429f1748cda4eedac36f04fe546861a9c9dfaf75719b53c75e0b933d4aad6d325f18c75776a260d507647b';
const SHOTS = 'e2e/__screenshots__/097';

const USDC = '0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d';
const DEBT_USDC = '0xcdbbed5606d9c5c98eeedd67933991dc17f0c68d';
const AAVE_POOL = '0x6807dc923806fe8fd134338eabca509979a7e0cb';
const PCS_ROUTER = '0xd9c500dff816a1da21a48a732d3498bf09dc9aeb';
const PCS_PERMIT2 = '0x31c2f6fcff4f8759b3bd5bf0e1084a055615c768';
const PERMIT2 = '0x000000000022d473030f116ddee9f6b43ac78ba3';
const FACTORY = '0xe12e0f117d23a5ccc57f8935cd8c4e80cd91ff01';
const USDC_TOKEN = { address: USDC, symbol: 'USDC', decimals: 18 };

/** The pass's rows, ending `nowSec`, one minute apart. */
function passRecords(nowSec: number): LocalTransaction[] {
	const tx = (id: string, at: number, extra: Partial<LocalTransaction>): LocalTransaction => ({
		id,
		userOpHash: '0x' + id.length.toString(16).padStart(2, '0').repeat(32),
		txHash: '',
		from: SAFE,
		to: AAVE_POOL,
		value: '0x0',
		symbol: 'BNB',
		decimals: 18,
		chainId: 56,
		timestamp: at,
		status: 'confirmed',
		type: 'dapp_tx',
		dappUrl: 'https://app.aave.com',
		...extra
	});
	const borrowTx = '0x' + 'cb'.repeat(32);
	return [
		tx('dapp-swap-tx', nowSec - 300, {
			to: USDC,
			txHash: '0x' + '3c'.repeat(32),
			dappUrl: 'https://pancakeswap.finance',
			intent: 'Swap',
			signedRequest: {
				method: 'wallet_sendCalls',
				params: [
					{
						calls: [
							{ to: USDC, data: '0x095ea7b3' },
							{ to: PCS_ROUTER, data: '0x3593564c' }
						]
					}
				]
			},
			dappSummary: {
				action: 'batch',
				calls: 3,
				contract: PCS_ROUTER,
				spender: PCS_PERMIT2,
				token: USDC,
				amount: '1160000000000000000',
				tokens: [USDC_TOKEN]
			},
			settlement: {
				moved: [
					{ token: USDC, delta: '-1160000000000000000' },
					{ token: null, delta: '1499036349071560' }
				]
			}
		}),
		tx('dapp-borrow-tx', nowSec - 240, {
			txHash: borrowTx,
			intent: 'Borrow',
			signedRequest: {
				method: 'eth_sendTransaction',
				params: [{ to: AAVE_POOL, data: '0xa415bcad' }]
			},
			dappSummary: { action: 'call', calls: 1, contract: AAVE_POOL, tokens: [USDC_TOKEN] },
			settlement: {
				moved: [
					{ token: DEBT_USDC, delta: '300000000000000001' },
					{ token: USDC, delta: '300000000000000000' }
				]
			}
		}),
		// The scan's own record of the borrowed USDC — folded into the borrow.
		{
			id: 'rx-borrow',
			userOpHash: '',
			txHash: borrowTx,
			from: AAVE_POOL,
			to: SAFE,
			value: '0.3',
			symbol: 'USDC',
			decimals: 18,
			chainId: 56,
			timestamp: nowSec - 230,
			status: 'confirmed',
			type: 'receive'
		},
		tx('dapp-approve-tx', nowSec - 180, {
			to: USDC,
			txHash: '0x' + 'd1'.repeat(32),
			dappUrl: 'https://app.uniswap.org',
			intent: 'Approve',
			signedRequest: { method: 'eth_sendTransaction', params: [{ to: USDC, data: '0x095ea7b3' }] },
			dappSummary: {
				action: 'approve',
				calls: 1,
				contract: USDC,
				spender: PERMIT2,
				token: USDC,
				symbol: 'USDC',
				decimals: 18,
				amount: '1160000000000000000'
			},
			settlement: { moved: [] }
		}),
		tx('dapp-order-tx', nowSec - 120, {
			to: FACTORY,
			value: '0xaa87bee538000',
			txHash: '0x' + 'af'.repeat(32),
			dappUrl: 'https://1inch.com',
			intent: 'create order',
			signedRequest: {
				method: 'eth_sendTransaction',
				params: [{ to: FACTORY, data: '0x8c72b608', value: '0xaa87bee538000' }]
			},
			dappSummary: {
				action: 'call',
				calls: 1,
				contract: FACTORY,
				contract_name: 'NativeOrderFactory',
				owner: '1inch'
			},
			settlement: { moved: [] }
		}),
		tx('dapp-withdraw-tx', nowSec - 60, {
			status: 'failed',
			intent: 'Withdraw',
			signedRequest: {
				method: 'eth_sendTransaction',
				params: [{ to: AAVE_POOL, data: '0x69328dec' }]
			},
			dappSummary: { action: 'call', calls: 1, contract: AAVE_POOL },
			settlement: { failure: 'refused' }
		})
	];
}

async function seed(page: Page, records: LocalTransaction[]) {
	await page.addInitScript(
		([rows, safe, key]) => {
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
				};
			}
		},
		[records, SAFE, FIXTURE_PUBLIC_KEY] as const
	);
}

test.beforeEach(async ({ page }) => {
	await denyOffOrigin(page);
	await stubChainRegistry(page, {
		56: {
			chainId: 56,
			name: 'BNB Chain',
			nativeCurrency: { name: 'BNB', symbol: 'BNB', decimals: 18 },
			stables: [],
			wrappedNativeToken: null,
			dex: null,
			rpc: [`${STUB}/56`]
		}
	});
	await stubJsonRpc(page, /stub-rpc\.test\/rpc\/(\d+)/, (method, params, url) => {
		const chainId = Number(/\/rpc\/(\d+)/.exec(url)?.[1]);
		if (method === 'eth_chainId') return '0x' + chainId.toString(16);
		if (method === 'eth_blockNumber') return '0x10';
		if (method === 'eth_getLogs') return [];
		if (method === 'eth_call') {
			const call = params[0] as { data?: string } | undefined;
			const n = call?.data ? aggregate3CallCount(call.data) : 0;
			const data = '0x' + abiWord(0) + abiWord(0) + abiWord(0) + abiWord(0) + abiWord(0);
			return encodeAggregate3Result(Array.from({ length: n }, () => ({ success: true, data })));
		}
		return undefined;
	});
	await seed(page, passRecords(Math.floor(Date.now() / 1000) - 60));
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
	await expect(page.getByText('$0', { exact: true }).first()).toBeVisible({ timeout: 20_000 });
}

const TITLES = [
	'Withdraw on Aave',
	'Create order on 1inch',
	'Approve on Uniswap',
	'Borrow on Aave',
	'Swap on PancakeSwap'
];

async function open(page: Page, title: RegExp) {
	await page.getByRole('button', { name: title }).first().click({ timeout: 20_000 });
	// The sheet's entrance has to finish before the picture is worth taking.
	await page.waitForTimeout(800);
}

test.describe('phone width', () => {
	test.use({ viewport: { width: 390, height: 844 } });

	test('the pass reads what the chain proved; a failed row says why', async ({ page }) => {
		await openWallet(page);
		for (const title of TITLES) {
			await expect(page.getByRole('button', { name: new RegExp(title) }).first()).toBeVisible({
				timeout: 20_000
			});
		}
		// The borrow: nothing left, the USDC it brought in stands alone.
		await expect(page.getByText('+0.3', { exact: true })).toBeVisible();
		// The swap leads with the USDC it took: BNB Chain's registry stablecoin,
		// trusted with no simulation on the sheet (097 D).
		await expect(page.getByText('−1.16', { exact: true })).toBeVisible();
		await page.screenshot({ path: `${SHOTS}/activity-phone-en.png`, fullPage: true });

		await open(page, /Borrow on Aave/);
		let sheet = page.getByRole('dialog');
		await expect(sheet.getByText('+0.3 USDC', { exact: true }).first()).toBeVisible();
		await expect(sheet.getByText(/≈ \$0\.00/)).toHaveCount(0);
		await page.screenshot({ path: `${SHOTS}/borrow-detail-phone-en.png` });
		await page.keyboard.press('Escape');
		await page.waitForTimeout(500);

		await open(page, /Swap on PancakeSwap/);
		sheet = page.getByRole('dialog');
		await expect(sheet.getByText(en('componentsUi.signing.balanceChangesTitle'))).toBeVisible();
		// The hero and its balance-change line (097 D: the row leads with it).
		await expect(sheet.getByText('−1.16 USDC', { exact: true })).toHaveCount(2);
		await page.screenshot({ path: `${SHOTS}/swap-detail-phone-en.png` });
		await page.keyboard.press('Escape');
		await page.waitForTimeout(500);

		await open(page, /Withdraw on Aave/);
		sheet = page.getByRole('dialog');
		await expect(sheet.getByText(en('componentsUi.signing.refused'))).toBeVisible();
		await page.screenshot({ path: `${SHOTS}/failed-detail-phone-en.png` });
		await page.keyboard.press('Escape');
		await page.waitForTimeout(500);

		await open(page, /Create order on 1inch/);
		sheet = page.getByRole('dialog');
		await expect(sheet.getByText('NativeOrderFactory')).toBeVisible();
		await expect(sheet.getByText(/≈ \$0\.00/)).toHaveCount(0);
		await page.screenshot({ path: `${SHOTS}/order-detail-phone-en.png` });
	});

	test('the same rows in Chinese', async ({ page }) => {
		await openWallet(page, 'zh');
		await expect(page.getByRole('button', { name: /在 Aave 借入/ }).first()).toBeVisible({
			timeout: 20_000
		});
		await page.screenshot({ path: `${SHOTS}/activity-phone-zh.png`, fullPage: true });
		await open(page, /在 Aave 取出/);
		await page.screenshot({ path: `${SHOTS}/failed-detail-phone-zh.png` });
	});
});

test.describe('desktop width', () => {
	test.use({ viewport: { width: 1440, height: 900 } });

	test('the rows, and the borrow and the refused withdraw in the third column', async ({
		page
	}) => {
		await openWallet(page);
		await expect(page.getByRole('button', { name: /Borrow on Aave/ }).first()).toBeVisible({
			timeout: 20_000
		});
		await page.screenshot({ path: `${SHOTS}/activity-desktop-en.png` });
		await open(page, /Borrow on Aave/);
		await expect(page.getByText('+0.3 USDC', { exact: true }).first()).toBeVisible();
		await page.screenshot({ path: `${SHOTS}/borrow-detail-desktop-en.png` });
		await open(page, /Withdraw on Aave/);
		await expect(page.getByText(en('componentsUi.signing.refused'))).toBeVisible();
		await page.screenshot({ path: `${SHOTS}/failed-detail-desktop-en.png` });
	});
});
