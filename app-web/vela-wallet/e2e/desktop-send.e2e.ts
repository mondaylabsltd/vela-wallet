/**
 * A send completes at desktop width (spec 028 T454 — SC-409).
 *
 * The same spine `send-lands` proves on the phone, driven through the third
 * column instead of a pushed screen: pick → type → the relay quotes → confirm
 * → the fixture key signs → the relay accepts → submitted. Until this phase the
 * wide layout showed these panels with live DATA and dead controls — a
 * Continue that did nothing on a form that knew the balance — because
 * `FlowsPanel` was handed no actions and the column read the nav stack instead
 * of the core's stage.
 *
 * Chain and relay are stubbed as in `send-lands`; nothing leaves the machine.
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

// The wide layout: `BREAKPOINT_DESKTOP` is 1280, and 1440 is the board's width.
test.use({ viewport: { width: 1440, height: 900 } });
test.setTimeout(120_000);

const STUB = 'https://stub-rpc.test/rpc';
const RELAY = /vela-relay\.getvela\.app/;
const ETH = 1_000_000_000_000_000_000n;
const PRICE_8DP = 3000n * 100_000_000n;
const RECIPIENT = '0x' + 'ab'.repeat(20);
const USER_OP_HASH = '0x' + 'a1'.repeat(32);
const TX_HASH = '0x' + 'b2'.repeat(32);

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

test('a send completes in the third column, through the same session as the phone', async ({
	page
}) => {
	await stubChain(page);
	await stubRelay(
		page,
		RELAY,
		happyRelay(USER_OP_HASH, TX_HASH, () => 'pending')
	);

	await page.goto('/en/parallel');
	await page.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
	await page.waitForURL(/\/en\/wallet$/);
	await seedNetworkOverrides(
		page,
		CHAINS.map((c) => ({ chainId: c.chainId, rpcURL: `${STUB}/${c.chainId}` }))
	);
	await page.reload();
	await expect(page.getByText('$4,500', { exact: true })).toBeVisible({ timeout: 25_000 });

	// 1. The wide layout's own Send pill opens the picker in the third column.
	await page
		.getByRole('button', { name: en('componentsUi.dock.send') })
		.first()
		.click();
	await expect(page.getByRole('heading', { name: en('send.selectTokenTitle') })).toBeVisible();
	// The picker's row, not the home's: at this width the assets list and the
	// picker are on screen together, and both name the row the same way. The
	// picker is the third column, so it is the LAST such button in the document.
	await page
		.getByRole('button', { name: /^ETH Ethereum/ })
		.last()
		.click();

	// 2. Recipient and amount, typed into the panel — the SAME controls the
	//    phone form has, driven by the same session.
	const recipient = page.getByRole('textbox', { name: en('send.recipientLabel') });
	await expect(recipient).toBeVisible();
	await recipient.fill(RECIPIENT);
	await page.getByRole('textbox', { name: 'ETH' }).fill('0.5');

	// 3. The relay's quote arms the CTA; the core's gate, not a fixture's.
	const advance = page.getByRole('button', { name: en('send.continueBtn') });
	await expect(advance).toBeEnabled({ timeout: 30_000 });
	await advance.click();

	// 4. Confirm, in the column.
	await expect(page.getByRole('heading', { name: en('send.confirmTitle') })).toBeVisible();
	await expect(page.getByText('0.5 ETH', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: en('send.confirmSendBtn') }).click();

	// 5. Submitted: signed by the fixture key, accepted by the relay, pending
	//    in the store — the same three facts `send-lands` checks on the phone.
	await expect(page.getByText(en('send.txSubmittedTitle'))).toBeVisible({ timeout: 30_000 });
	const pending = await readKv(page, 'vela.transactionHistory');
	expect(pending).toContain(USER_OP_HASH);
	expect(pending).toContain('"status":"pending"');
});
