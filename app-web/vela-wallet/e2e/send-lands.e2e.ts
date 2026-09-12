/**
 * A send that lands (spec 026 T237 — SC-201), hermetically.
 *
 * The whole spine in one run, inside the parallel space so the passkey
 * ceremony is a fixed key rather than a device: pick a token → type an address
 * and an amount → the relay quotes → confirm → the fixture key signs → the
 * relay accepts → the record is persisted as pending → the tracker polls →
 * the receipt confirms. Chain and relay are both stubbed; nothing leaves the
 * machine, and the parallel badge says which wallet this is.
 */
import { expect, test } from '@playwright/test';
import { CHAINS } from '../src/lib/services/chains';
import { en } from './live-helpers';
import {
	abiWord,
	aggregate3CallCount,
	denyOffOrigin,
	encodeAggregate3Result,
	happyRelay,
	readKv,
	seedNetworkOverrides,
	stubChainRegistry,
	stubJsonRpc,
	stubRelay
} from './stub-chain';

test.use({ viewport: { width: 390, height: 844 } });
test.setTimeout(120_000);

const STUB = 'https://stub-rpc.test/rpc';
const RELAY = /vela-relay\.getvela\.app/;
const ETH = 1_000_000_000_000_000_000n;
const PRICE_8DP = 3000n * 100_000_000n;
const RECIPIENT = '0x' + 'ab'.repeat(20);
const USER_OP_HASH = '0x' + 'a1'.repeat(32);
const TX_HASH = '0x' + 'b2'.repeat(32);

/** Deployed Safe, 1.5 ETH at $3,000 — the chain every leg of this test reads. */
async function stubChain(page: import('@playwright/test').Page): Promise<void> {
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
		// A deployed Safe: the fee quote must not attach initCode.
		if (method === 'eth_getCode') return '0x6080';
		if (method === 'eth_getTransactionCount') return '0x0';
		if (method === 'eth_gasPrice') return '0x3b9aca00';
		if (method === 'eth_maxPriorityFeePerGas') return '0x3b9aca00';
		if (method === 'eth_getBlockByNumber') return { baseFeePerGas: '0x3b9aca00' };
		if (method === 'eth_call') {
			const call = params[0] as { data?: string } | undefined;
			const n = call?.data ? aggregate3CallCount(call.data) : 0;
			const balance = chainId === 1 ? (ETH * 3n) / 2n : 0n;
			const data =
				'0x' + abiWord(balance) + abiWord(PRICE_8DP) + abiWord(0) + abiWord(0) + abiWord(0);
			if (n === 0) return '0x' + abiWord(0);
			return encodeAggregate3Result(Array.from({ length: n }, () => ({ success: true, data })));
		}
		if (method === 'eth_getLogs') return [];
		return undefined;
	});
}

test('a send goes out and comes back confirmed, in the parallel space', async ({ page }) => {
	let receiptState: 'pending' | 'landed' = 'pending';
	await stubChain(page);
	await stubRelay(
		page,
		RELAY,
		happyRelay(USER_OP_HASH, TX_HASH, () => receiptState)
	);

	// The parallel space: the real app, with a fixed key where the passkey is.
	await page.goto('/en/parallel');
	await page.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
	await page.waitForURL(/\/en\/wallet$/);
	await expect(page.getByTestId('parallel-space-badge')).toBeVisible();

	await seedNetworkOverrides(
		page,
		CHAINS.map((c) => ({ chainId: c.chainId, rpcURL: `${STUB}/${c.chainId}` }))
	);
	await page.reload();
	await expect(page.getByText('$4,500', { exact: true })).toBeVisible({ timeout: 25_000 });

	// 1. Pick the token.
	await page
		.getByRole('button', { name: en('componentsUi.dock.send') })
		.first()
		.click();
	await expect(page.getByRole('heading', { name: en('send.selectTokenTitle') })).toBeVisible();
	await page.getByText('ETH', { exact: true }).first().click();

	// 2. Recipient and amount — typed, as a person types them.
	await page.getByRole('textbox', { name: en('send.recipientLabel') }).fill(RECIPIENT);
	// 最大 (spec 028 Phase 9, T489): the core's own reserve math fills the
	// field with the balance NET of the fee it estimates — never the raw 1.5.
	await page.getByRole('button', { name: en('send.maxBtn') }).click();
	await expect(page.getByRole('textbox', { name: 'ETH' })).toHaveValue(/^1\.4\d+$/, {
		timeout: 30_000
	});
	await page.getByRole('textbox', { name: 'ETH' }).fill('0.5');
	// The fee is on the form before Continue (T490): the row names a figure in
	// the fee coin, not the "—" the drawn row shows while nothing has quoted.
	const feeRow = page.getByRole('button', { name: en('send.feeTokenLabel') });
	await expect(feeRow).not.toContainText('—', { timeout: 30_000 });
	await expect(feeRow).toContainText('ETH');

	// 3. The relay's quote arrives and arms the CTA.
	const advance = page.getByRole('button', { name: en('send.continueBtn') });
	await expect(advance).toBeEnabled({ timeout: 30_000 });
	await advance.click();

	// 4. Confirm: what is about to be signed, in the core's words.
	await expect(page.getByRole('heading', { name: en('send.confirmTitle') })).toBeVisible();
	await expect(page.getByText('0.5 ETH', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: en('send.confirmSendBtn') }).click();

	// 5. Submitted: the fixture key signed, the relay accepted, and the record
	//    is pending in the store BEFORE any receipt exists.
	await expect(page.getByText(en('send.txSubmittedTitle'))).toBeVisible({ timeout: 30_000 });
	const pending = await readKv(page, 'vela.transactionHistory');
	expect(pending).toContain(USER_OP_HASH);
	expect(pending).toContain('"status":"pending"');

	// 6. The chain confirms it, and the tracker's own poll brings it home.
	receiptState = 'landed';
	await expect(page.getByText(en('componentsTx.receipt.done'))).toBeVisible({ timeout: 45_000 });
	const settled = await readKv(page, 'vela.transactionHistory');
	expect(settled).toContain('"status":"confirmed"');
	expect(settled).toContain(TX_HASH);
});
