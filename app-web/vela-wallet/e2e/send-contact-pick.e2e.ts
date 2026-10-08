/**
 * The send form's person icon opens the book and a contact can be picked
 * (issue 467), hermetically — and the pick is by ADDRESS: the person whose
 * row was tapped becomes the recipient, not whoever sits at that place in a
 * book that re-sorts while names resolve.
 *
 * Same stubs as `send-denom-toggle.e2e.ts`: 1 ETH held on Ethereum, priced.
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

test.use({ viewport: { width: 390, height: 844 } });
test.setTimeout(120_000);

const STUB = 'https://stub-rpc.test/rpc';
const BALANCE = 1_000_000_000_000_000_000n;
const PRICE_8DP = 3000n * 100_000_000n;
const ALICE = '0x' + 'a1'.repeat(20);
const BOB = '0x' + 'b2'.repeat(20);

async function stubChain(page: Page): Promise<void> {
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
		if (method === 'eth_getCode') return '0x6080';
		if (method === 'eth_getTransactionCount') return '0x0';
		if (method === 'eth_gasPrice') return '0x3b9aca00';
		if (method === 'eth_maxPriorityFeePerGas') return '0x3b9aca00';
		if (method === 'eth_getBlockByNumber') return { baseFeePerGas: '0x3b9aca00' };
		if (method === 'eth_call') {
			const call = params[0] as { data?: string } | undefined;
			const n = call?.data ? aggregate3CallCount(call.data) : 0;
			const balance = chainId === 1 ? BALANCE : 0n;
			const data =
				'0x' + abiWord(balance) + abiWord(PRICE_8DP) + abiWord(0) + abiWord(0) + abiWord(0);
			if (n === 0) return '0x' + abiWord(0);
			return encodeAggregate3Result(Array.from({ length: n }, () => ({ success: true, data })));
		}
		if (method === 'eth_getLogs') return [];
		return undefined;
	});
}

async function addContact(page: Page, name: string, address: string): Promise<void> {
	await page
		.getByRole('button', { name: en('contacts.addContact') })
		.first()
		.click();
	await page.getByRole('menuitem', { name: en('contacts.addTitle') }).click();
	await page.getByLabel(en('contacts.nameLabel')).fill(name);
	await page.getByLabel(en('contacts.addressLabel')).fill(address);
	await page.getByRole('button', { name: en('contacts.save') }).click();
	await expect(page.getByText(name, { exact: true })).toBeVisible();
}

test('the person icon opens the book, and the contact tapped is the one picked (issue 467)', async ({
	page
}, testInfo) => {
	await stubChain(page);
	await page.goto('/en/parallel');
	await page.locator('li code').first().waitFor({ timeout: 60_000 });
	await page.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
	await page.waitForURL(/\/en\/wallet$/);
	await seedNetworkOverrides(
		page,
		CHAINS.map((c) => ({ chainId: c.chainId, rpcURL: `${STUB}/${c.chainId}` }))
	);

	// Two people in this wallet's book.
	await page.goto('/en/contacts');
	await expect(page.getByRole('heading', { name: en('contacts.title') }).first()).toBeVisible();
	await addContact(page, 'Alice', ALICE);
	await addContact(page, 'Bob', BOB);

	await page.goto('/en/wallet');
	await page
		.getByRole('button', { name: en('componentsUi.dock.send') })
		.first()
		.click();
	await expect(page.getByRole('heading', { name: en('send.selectTokenTitle') })).toBeVisible({
		timeout: 30_000
	});
	await page.getByText('ETH', { exact: true }).first().click();

	// The recipient row's two doors, as on every app (issue 468): the book and
	// the scanner.
	await expect(page.getByRole('button', { name: en('send.scanAria') })).toBeVisible();
	await page.getByRole('button', { name: en('send.recipientPickAria') }).click();
	const sheet = page.getByRole('dialog');
	await expect(sheet.getByText(en('send.pickContactTitle')).first()).toBeVisible();
	await expect(sheet.getByText('Alice', { exact: true })).toBeVisible();
	await page.screenshot({ path: testInfo.outputPath('467-picker.png') });

	await sheet.getByText('Bob', { exact: true }).click();
	await expect(page.getByRole('dialog')).toHaveCount(0);
	await expect(page.getByRole('textbox', { name: en('send.recipientLabel') })).toHaveValue(
		new RegExp(BOB, 'i')
	);

	// And again: the picker opens a second time, and Alice is Alice.
	await page.getByRole('button', { name: en('send.recipientPickAria') }).click();
	await page.getByRole('dialog').getByText('Alice', { exact: true }).click();
	await expect(page.getByRole('textbox', { name: en('send.recipientLabel') })).toHaveValue(
		new RegExp(ALICE, 'i')
	);
});
