/**
 * A send panel never quotes a balance the account does not have (issue 209).
 *
 * The reported journey: a brand-new account — total $0.00, no assets, "All
 * Networks 0" — and 转账 on a contact. That action navigates to
 * `/wallet?to=0x…` (`contact-handoff.ts`, pinned by its own unit test), and
 * the wallet route opens the send flow with the recipient prefilled. The core
 * steps straight to the form so the person can be shown WHO the money is for
 * before the token list answers — and on an account that holds nothing it
 * never can name a token. The web's SD2 overlay kept the drawn card when the
 * core had chosen none, so the panel read "USDT · Ethereum · Balance 53.4836"
 * beside a wallet reading $0.00.
 *
 * Both layouts are driven, because the drawn card is the same model on each:
 * the phone pushes SD2 over the home, the desktop draws DSD2 in its third
 * column, and the report came from the second.
 *
 * Every chain answers zero holdings; nothing leaves the machine.
 */
import { expect, test, type Page } from '@playwright/test';
import { CHAINS } from '../src/lib/services/chains';
import { en, seedSignedIn } from './live-helpers';
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
/** The contact the hand-off is about — any address the book could carry. */
const RECIPIENT = '0x' + 'ab'.repeat(20);

/** What the drawn SD2 / DSD2 says, and must never say on a live account. */
const DRAWN_BALANCE = '53.4836';
const DRAWN_SYMBOL = 'USDT';

test.beforeEach(async ({ page }) => {
	await seedSignedIn(page);
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
		if (method === 'eth_getCode') return '0x';
		if (method === 'eth_call') {
			// A brand-new account: every balance, on every chain, is zero.
			const call = params[0] as { data?: string } | undefined;
			const n = call?.data ? aggregate3CallCount(call.data) : 0;
			const data = '0x' + abiWord(0) + abiWord(0) + abiWord(0) + abiWord(0) + abiWord(0);
			return encodeAggregate3Result(Array.from({ length: n }, () => ({ success: true, data })));
		}
		return undefined;
	});
});

/** Land on the empty wallet, then arrive the way the address book sends you. */
async function handOffToSend(page: Page): Promise<void> {
	await page.goto('/en/wallet');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	// Overrides are read at pool-config time; seed them, then arrive again —
	// this time carrying the contact, exactly as 转账 on a contact navigates.
	await seedNetworkOverrides(
		page,
		CHAINS.map((c) => ({ chainId: c.chainId, rpcURL: `${STUB}/${c.chainId}` }))
	);
	// No wallet-name assertion here: on the phone the flow is a screen PUSHED
	// over the home, so the header the desktop keeps beside its panel is gone.
	// What the page is showing is asserted by the callers.
	await page.goto(`/en/wallet?to=${RECIPIENT}`);
}

/** No drawn token, anywhere on the page, in any layout. */
async function expectNoDrawnToken(page: Page): Promise<void> {
	await expect(page.getByText(DRAWN_BALANCE)).toHaveCount(0);
	await expect(page.getByText(`Send ${DRAWN_SYMBOL}`)).toHaveCount(0);
	await expect(page.getByText(en('send.maxBtn'), { exact: true })).toHaveCount(0);
}

/**
 * The panel is the picker, and it says why it is empty — the list the core
 * came back with, not a form about a token nobody holds.
 */
async function expectTheEmptyPicker(page: Page): Promise<void> {
	await expect(page.getByText(en('send.selectTokenTitle')).first()).toBeVisible({
		timeout: 20_000
	});
	await expect(page.getByText(en('send.noTokensWithBalance'))).toBeVisible();
	await expectNoDrawnToken(page);
}

test.describe('desktop — the third column', () => {
	// `BREAKPOINT_DESKTOP` is 1280; 1440 is the board's width, and the report's.
	test.use({ viewport: { width: 1440, height: 900 } });

	test('a hand-off on an empty account opens the picker, not a phantom balance', async ({
		page
	}) => {
		await handOffToSend(page);
		// The account really is empty: the home says so beside the panel.
		await expect(page.getByText('$0', { exact: true }).first()).toBeVisible({ timeout: 20_000 });
		await expectTheEmptyPicker(page);
	});
});

test.describe('phone — the pushed screen', () => {
	test.use({ viewport: { width: 390, height: 844 } });

	test('the pushed send screen quotes no balance either', async ({ page }) => {
		await handOffToSend(page);
		await expectTheEmptyPicker(page);
	});
});
