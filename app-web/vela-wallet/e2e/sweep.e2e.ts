/**
 * A sweep is ONE operation (spec 028 T445 — SC-404).
 *
 * Two assets on one network, chosen together, sent to one address — and the
 * relay receives exactly one `eth_sendUserOperation`, whose calldata carries
 * both transfers. The "one" is the whole point: a sweep that submitted N
 * operations would be N fees, N signatures and N chances to half-complete.
 *
 * Everything else on the way is the core's ruling, read off the screen: the
 * picker ticks what `multi_selected_ids` says, greys what `multi_chain_id`
 * says, and the confirm lists the reserved amounts the signature will move.
 * Chain and relay are stubbed per call; nothing leaves the machine.
 */
import { expect, test, type Page } from '@playwright/test';
import { CHAINS } from '../src/lib/services/chains';
import { en } from './live-helpers';
import {
	denyOffOrigin,
	happyRelay,
	seedNetworkOverrides,
	stubChainRegistry,
	stubJsonRpc,
	stubRelay
} from './stub-chain';
import {
	answerAggregate3,
	isAggregate3,
	MULTICALL_SEL as SEL,
	roundData,
	word
} from './stub-multicall';

test.use({ viewport: { width: 390, height: 844 } });
test.setTimeout(120_000);

const STUB = 'https://stub-rpc.test/rpc';
const RELAY = /vela-relay\.getvela\.app/;
const USDC = '0x' + 'cc'.repeat(20);
const ONE_AND_A_HALF_ETH = 1_500_000_000_000_000_000n;
const HUNDRED_USDC = 100_000_000n;
const ETH_USD_8DP = 3000n * 100_000_000n;
const RECIPIENT = '0x' + 'ab'.repeat(20);
const USER_OP_HASH = '0x' + 'a1'.repeat(32);
const TX_HASH = '0x' + 'b2'.repeat(32);

/**
 * Ethereum with a native coin AND a stablecoin, both held — two valuable
 * assets on one chain, which is the least a sweep can be. The multicall is
 * answered per call, so each slot decodes its own balance.
 */
async function stubChain(page: Page): Promise<void> {
	await denyOffOrigin(page);
	await stubChainRegistry(page, {
		1: {
			chainId: 1,
			name: 'Ethereum',
			nativeCurrency: { name: 'Ether', symbol: 'ETH', decimals: 18 },
			stables: [{ symbol: 'USDC', type: 'native', contract: USDC }],
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
		if (method === 'eth_getLogs') return [];
		if (method === 'eth_call') {
			const call = params[0] as { data?: string } | undefined;
			// A plain call (the Safe's `nonce()`, a lone `balanceOf`) expects one
			// word, not an aggregate3 envelope — send-lands answers it with zero.
			if (!call?.data || !isAggregate3(call.data)) return '0x' + word(0);
			if (chainId !== 1) return answerAggregate3(call.data, () => undefined);
			return answerAggregate3(call.data, (inner) => {
				switch (inner.selector) {
					case SEL.getEthBalance:
						return { success: true, data: '0x' + word(ONE_AND_A_HALF_ETH) };
					case SEL.balanceOf:
						return inner.target === USDC
							? { success: true, data: '0x' + word(HUNDRED_USDC) }
							: { success: true, data: '0x' + word(0) };
					case SEL.decimals:
						return { success: true, data: '0x' + word(6) };
					case SEL.latestRoundData:
						return { success: true, data: roundData(ETH_USD_8DP) };
					default:
						// DEX quotes and anything else: no such pool. The price ladder
						// falls through to the Chainlink feed above, as it is built to.
						return undefined;
				}
			});
		}
		return undefined;
	});
}

test('two assets on one network go out as ONE operation carrying both', async ({ page }) => {
	const sent: unknown[][] = [];
	const seen: string[] = [];
	const errors: string[] = [];
	page.on('console', (message) => {
		// The route says a refused confirm out loud as `[send] alert:` (a warning)
		// and a core fault as an error; both are the answer to "why did the
		// button do nothing".
		if (message.type() === 'error' || message.type() === 'warning') errors.push(message.text());
	});
	page.on('pageerror', (error) => errors.push(error.message));
	await stubChain(page);
	// The receipt stays PENDING for the whole test: with the default "landed"
	// answer, the tracker's poll can confirm the operation before the test
	// looks for the submitted title, and the assertion races the receipt.
	const relay = happyRelay(USER_OP_HASH, TX_HASH, () => 'pending');
	await stubRelay(page, RELAY, (method, params) => {
		seen.push(method);
		if (method === 'eth_sendUserOperation') sent.push(params);
		return relay(method, params);
	});

	// The parallel space: the real app, with a fixed key where the passkey is.
	await page.goto('/en/parallel');
	await page.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
	await page.waitForURL(/\/en\/wallet$/);
	await seedNetworkOverrides(
		page,
		CHAINS.map((c) => ({ chainId: c.chainId, rpcURL: `${STUB}/${c.chainId}` }))
	);
	await page.reload();
	// 1.5 ETH at $3,000 + 100 USDC at ≈$1.
	await expect(page.getByText('$4,600', { exact: true })).toBeVisible({ timeout: 25_000 });

	// 1. Open the picker and switch it to choosing several.
	await page
		.getByRole('button', { name: en('componentsUi.dock.send') })
		.first()
		.click();
	await expect(page.getByRole('heading', { name: en('send.selectTokenTitle') })).toBeVisible();
	await page.getByRole('button', { name: en('send.multiSendTitle') }).click();
	await expect(page.getByRole('heading', { name: en('send.multiSendTitle') })).toBeVisible();

	// 2. Tick both. The first tap pins the network; the notice says which.
	await page.getByText('ETH', { exact: true }).first().click();
	await expect(
		page.getByText(en('send.multiSendChainNotice').replace('{{network}}', 'Ethereum'))
	).toBeVisible();
	await page.getByText('USDC', { exact: true }).first().click();

	// The CTA counts what the core holds selected — two, on Ethereum.
	const proceed = page.getByRole('button', {
		name: en('send.multiSendContinue').replace('{{n}}', '2').replace('{{chain}}', 'Ethereum')
	});
	await expect(proceed).toBeVisible();
	await proceed.click();

	// 3. The sweep form: both rows, one recipient.
	await expect(page.getByText(en('send.multiSendSameRecipient'))).toBeVisible();
	await page.getByRole('textbox', { name: en('send.recipientLabel') }).fill(RECIPIENT);
	const advance = page.getByRole('button', { name: en('send.continueBtn') });
	await expect(advance).toBeEnabled({ timeout: 30_000 });
	await advance.click();

	// 4. Confirm: N assets, and the breakdown is the reserved amounts.
	await expect(page.getByRole('heading', { name: en('send.confirmTitle') })).toBeVisible();
	await expect(
		page.getByText(en('componentsTx.receipt.assetsCount').replace('{{n}}', '2'))
	).toBeVisible();
	await expect(page.getByText(/100 USDC/)).toBeVisible();
	await page.getByRole('button', { name: en('send.confirmSendBtn') }).click();

	// 5. Submitted — and the relay saw exactly one operation, carrying both
	//    transfers: the USDC contract is called, and the recipient appears for
	//    the ETH value AND as the transfer's argument.
	await expect
		.poll(() => sent.length, { timeout: 30_000 })
		.toBe(1)
		.catch(() => undefined);
	expect(
		sent,
		`no user operation reached the relay. relay methods: ${seen.join(',')}; console: ${errors.join(' | ')}`
	).toHaveLength(1);
	await expect(page.getByText(en('send.txSubmittedTitle'))).toBeVisible({ timeout: 30_000 });
	const op = sent[0][0] as { callData: string };
	const callData = op.callData.toLowerCase();
	expect(callData).toContain(USDC.slice(2));
	expect(callData.split(RECIPIENT.slice(2)).length - 1).toBeGreaterThanOrEqual(2);
});
