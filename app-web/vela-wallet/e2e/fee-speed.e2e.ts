/**
 * Spec 068 — the fee is something you can refresh, and a speed you can choose.
 *
 * The one thing this file exists to pin is the agreement between the screen
 * and the chain: whatever speed the send form says it is running at, THAT
 * name is the third element of `eth_sendUserOperation`'s params
 * (`[userOperation, entryPoint, tier?]`, the relay's wire since it learned
 * about speed). A picker that changed the number on screen without changing
 * what was submitted would be the exact "the screen says one thing, the chain
 * does another" failure this whole batch is about — and no unit test can see
 * it, because the tier crosses four modules on its way out.
 *
 * Chain and relay are stubbed per call; nothing leaves the machine.
 */
import { expect, test, type Page } from '@playwright/test';
import { CHAINS } from '../src/lib/services/chains';
import { en, seedSignedIn } from './live-helpers';
import {
	abiWord,
	aggregate3CallCount,
	denyOffOrigin,
	encodeAggregate3Result,
	happyRelay,
	seedNetworkOverrides,
	stubChainRegistry,
	stubJsonRpc,
	stubRelay
} from './stub-chain';

test.use({ viewport: { width: 390, height: 844 } });
test.setTimeout(120_000);

const STUB = 'https://stub-rpc.test/rpc';
// The app asks the Cloudflare relay (`vela-relay-cf`, endpoints.ts since spec 060);
// the older host is kept so a stub written against either still intercepts.
const RELAY = /vela-relay(-cf)?\.getvela\.app/;
const ETH = 1_000_000_000_000_000_000n;
const PRICE_8DP = 3000n * 100_000_000n;
const RECIPIENT = '0x' + 'ab'.repeat(20);
const USER_OP_HASH = '0x' + 'a1'.repeat(32);
const TX_HASH = '0x' + 'b2'.repeat(32);

/** A chain's gas market, as the stub answers it: hex wei. */
interface GasMarket {
	gasPrice: string;
	tip: string;
	baseFee: string;
}

/** A CALM chain: 1 gwei base, a 0.1 gwei tip. See the note in `stubChain`. */
const CALM: GasMarket = { gasPrice: '0x3b9aca00', tip: '0x5f5e100', baseFee: '0x3b9aca00' };

/** Deployed Safe, 1.5 ETH at $3,000 — `send-lands`' chain, verbatim. */
async function stubChain(page: Page, gas: GasMarket = CALM): Promise<void> {
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
		// A CALM chain: 1 gwei base, a 0.1 gwei tip. The relay's tier rows below
		// then sit above it, so `max(chain, relay)` — what the in-band
		// reimbursement is priced against — really is the tier's own number and
		// the three options can differ. On the shared `happyRelay` chain (2 gwei)
		// the chain price dominates every tier and all three read the same,
		// which is honest arithmetic but tests nothing.
		if (method === 'eth_gasPrice') return gas.gasPrice;
		if (method === 'eth_maxPriorityFeePerGas') return gas.tip;
		if (method === 'eth_getBlockByNumber') return { baseFeePerGas: gas.baseFee };
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

/**
 * The relay's three reported tiers — 1.2 / 2.0 / 3.0 gwei of NETWORK fee.
 *
 * `networkFeePerGas` is the field that matters: the in-band reimbursement is
 * priced against `max(that, our own chain measurement)`, so a relay that
 * publishes only `maxFeePerGas` prices every tier at the chain price and the
 * three options read identically. All three sit within the client's
 * `MAX_QUOTE_VS_CHAIN_MULTIPLE` (×3 of the 1.1 gwei chain price above — a
 * quote further above our own measurement is REFUSED, not paid).
 */
const TIER_QUOTES = {
	slow: {
		maxFeePerGas: '0x59682f00',
		maxPriorityFeePerGas: '0x59682f00',
		networkFeePerGas: '0x47868c00',
		relayerFeePerGas: '0x11e1a300'
	},
	standard: {
		maxFeePerGas: '0x9502f900',
		maxPriorityFeePerGas: '0x9502f900',
		networkFeePerGas: '0x77359400',
		relayerFeePerGas: '0x1dcd6500'
	},
	fast: {
		maxFeePerGas: '0xd09dc300',
		maxPriorityFeePerGas: '0xd09dc300',
		networkFeePerGas: '0xb2d05e00',
		relayerFeePerGas: '0x1dcd6500'
	}
};

/** `happyRelay`, with the tier rows this spec needs to tell apart. */
function tieredRelay(receipt: () => 'landed' | 'pending' | 'failed') {
	const base = happyRelay(USER_OP_HASH, TX_HASH, receipt);
	return (method: string, params: unknown[]) =>
		method === 'pimlico_getUserOperationGasPrice' ? TIER_QUOTES : base(method, params);
}

/** Into the parallel space, on the stubbed chain, with the balance showing. */
async function enterWallet(page: Page): Promise<void> {
	await page.goto('/en/parallel');
	// The button is in the server's HTML before the page hydrates; pressed then,
	// it does nothing and the URL wait runs out the whole test (batch.e2e.ts).
	// The fixture list fills on mount — a row in it means the handlers are on.
	await page.locator('li code').first().waitFor({ timeout: 60_000 });
	await page.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
	await page.waitForURL(/\/en\/wallet$/);
	await seedNetworkOverrides(
		page,
		CHAINS.map((c) => ({ chainId: c.chainId, rpcURL: `${STUB}/${c.chainId}` }))
	);
	await page.reload();
	await expect(page.getByText('$4,500', { exact: true })).toBeVisible({ timeout: 25_000 });
}

/** A form with a recipient and an amount, quoted — the state before Continue. */
async function fillForm(page: Page): Promise<void> {
	await page
		.getByRole('button', { name: en('componentsUi.dock.send') })
		.first()
		.click();
	await expect(page.getByRole('heading', { name: en('send.selectTokenTitle') })).toBeVisible();
	await page.getByText('ETH', { exact: true }).first().click();
	await page.getByRole('textbox', { name: en('send.recipientLabel') }).fill(RECIPIENT);
	await page.getByRole('textbox', { name: 'ETH' }).fill('0.5');
	const feeRow = page.getByRole('button', { name: en('send.feeTokenLabel') });
	await expect(feeRow).not.toContainText('—', { timeout: 30_000 });
}

test('the speed on the form is the tier on the wire — default, and after a pick', async ({
	page
}) => {
	const sent: unknown[][] = [];
	await stubChain(page);
	await stubRelay(page, RELAY, (method, params) => {
		if (method === 'eth_sendUserOperation') sent.push(params);
		return tieredRelay(() => 'pending')(method, params);
	});
	await enterWallet(page);
	await fillForm(page);

	// 1. Folded, and folded on THEIR default — which on a fresh device is the
	//    factory `fast`, i.e. exactly what every shell did before spec 068.
	const speed = page.getByRole('button', { expanded: false }).filter({
		hasText: en('send.feeSpeedLabel')
	});
	await expect(speed).toContainText(en('send.gasTier.fast'));
	await expect(page.getByText(en('send.feeSpeedOnce'))).toBeHidden();

	// 2. Opened: three options, named by what they buy, each with its OWN fee.
	//    The stub relay quotes `fast` at 3 gwei and `slow` at 1.2, so "cheaper"
	//    has to be visibly cheaper here or the figures are not really per-tier.
	await speed.click();
	await expect(page.getByText(en('send.feeSpeedOnce'))).toBeVisible();
	const economy = page.getByRole('button', { pressed: false }).filter({
		hasText: en('send.gasTier.slow')
	});
	await expect(economy).toContainText('ETH', { timeout: 30_000 });
	const fastFee = await page
		.getByRole('button', { pressed: true })
		.filter({ hasText: en('send.gasTier.fast') })
		.textContent();
	const slowFee = await economy.textContent();
	expect(fastFee, 'the option in force shows a figure').toMatch(/[\d.]+\s*ETH/);
	expect(slowFee).toMatch(/[\d.]+\s*ETH/);
	const coin = (text: string | null) => Number(/([\d.]+)\s*ETH/.exec(text ?? '')?.[1] ?? 'NaN');
	expect(coin(slowFee), 'Slow really is cheaper than Fast').toBeLessThan(coin(fastFee));

	const standardFee = await page
		.getByRole('button', { pressed: false })
		.filter({ hasText: en('send.gasTier.standard') })
		.textContent();

	// The row as it stands BEFORE any pick, so what follows is a move and not
	// a coincidence.
	const feeRow = page.getByRole('button', { name: en('send.feeTokenLabel') });
	expect(coin(await feeRow.textContent()), 'the row starts at the default tier').toBeCloseTo(
		coin(fastFee),
		12
	);

	// 3. Pick Standard. The control folds, the summary says what this send is
	//    now running at — AND THE MONEY MOVES WITH THE NAME. That last part is
	//    the assertion the rest of this file cannot make: the caption above and
	//    the wire below are both satisfied by Continue's pre-check, which
	//    re-quotes unconditionally on its way to the confirm, so a picker that
	//    renamed the row without re-pricing it would pass everything else here
	//    while the person read Fast's money under another tier's name for two
	//    whole screens.
	await page
		.getByRole('button', { pressed: false })
		.filter({ hasText: en('send.gasTier.standard') })
		.click();
	await expect(speed).toContainText(en('send.gasTier.standard'));
	await expect
		.poll(async () => coin(await feeRow.textContent()), { timeout: 30_000 })
		.toBeCloseTo(coin(standardFee), 12);

	// 3b. And again, from a form whose estimate has now DEFINITELY settled —
	//     which is the case the first pick cannot cover. `form_estimate_key` in
	//     the send core is token | payee | fee-coin and knows nothing about a
	//     speed, so anything that re-prices by re-asking the core for the same
	//     three is a no-op from here on, and only the second pick says so.
	await speed.click();
	const economyAgain = page
		.getByRole('button', { pressed: false })
		.filter({ hasText: en('send.gasTier.slow') });
	await expect(economyAgain).toContainText('ETH', { timeout: 30_000 });

	// Everything the fee row says from the tap onward (issue 681).
	//
	// This is the only place in the tree that reaches the ROUTE's own
	// `pickSpeed` — every other test of the promotion drives the library
	// directly, so without this, deleting the one `tierPreview.promote(...)` line
	// would leave every gate green. The tapped row is a settled quote of THIS
	// operation at THAT tier, so promotion puts its figure straight on the row
	// and the log holds exactly that. A re-quote cannot: the estimate in hand
	// belongs to the speed just left and must never be drawn under this one's
	// name, so the row has to blank to "…" while it measures.
	await feeRow.evaluate((row) => {
		const said: string[] = [];
		(window as unknown as { __feeRowSaid: string[] }).__feeRowSaid = said;
		new MutationObserver(() => said.push(row.textContent ?? '')).observe(row, {
			subtree: true,
			childList: true,
			characterData: true
		});
	});
	await economyAgain.click();
	await expect(speed).toContainText(en('send.gasTier.slow'));
	await expect
		.poll(async () => coin(await feeRow.textContent()), { timeout: 30_000 })
		.toBeCloseTo(coin(slowFee), 12);
	const said = await page.evaluate(
		() => (window as unknown as { __feeRowSaid: string[] }).__feeRowSaid
	);
	// The observer was live and caught the move; an empty log would make the
	// negative below a pass nobody earned.
	expect(coin(said.at(-1) ?? null), 'the row moved to the tapped figure').toBeCloseTo(
		coin(slowFee),
		12
	);
	expect(
		said.filter((text) => text.includes('…')),
		'nothing was measured in between — the tapped quote simply took over'
	).toEqual([]);

	// 3c. Re-opened, the figure they chose is still theirs, and the three rows
	//     are still three different prices — not one number wearing three names.
	await speed.click();
	const fastOption = page
		.getByRole('button', { pressed: false })
		.filter({ hasText: en('send.gasTier.fast') });
	await expect(fastOption).toContainText('ETH', { timeout: 30_000 });
	const reopenedFast = await fastOption.textContent();
	const reopenedSlow = await page
		.getByRole('button', { pressed: true })
		.filter({ hasText: en('send.gasTier.slow') })
		.textContent();
	expect(coin(reopenedSlow), 'the tier in force still shows its own fee').toBeCloseTo(
		coin(slowFee),
		12
	);
	expect(coin(reopenedFast)).toBeGreaterThan(coin(reopenedSlow));
	await page
		.getByRole('button', { expanded: true })
		.filter({ hasText: en('send.feeSpeedLabel') })
		.click();

	// 4. Send it. The relay is told `slow` — the same name the form is showing.
	const advance = page.getByRole('button', { name: en('send.continueBtn') });
	await expect(advance).toBeEnabled({ timeout: 30_000 });
	await advance.click();
	await expect(page.getByRole('heading', { name: en('send.confirmTitle') })).toBeVisible();
	// The last screen before a signature restates a speed that was CHOSEN, so a
	// payment deliberately bumped off the usual pace — or mis-tapped on the way
	// past — is caught while it can still be undone.
	await expect(page.getByText(en('send.feeSpeedLabel'), { exact: true })).toBeVisible();
	await page.getByRole('button', { name: en('send.confirmSendBtn') }).click();
	await expect(page.getByText(en('send.txSubmittedTitle'))).toBeVisible({ timeout: 30_000 });

	expect(sent).toHaveLength(1);
	// `[userOperation, entryPoint, tier]` — the tier is the THIRD element, and
	// it is the wire NAME, never a wei figure: the relay resolves it against
	// the base fee it reads at submit time.
	expect(sent[0]).toHaveLength(3);
	expect(sent[0][2]).toBe('slow');
});

/**
 * Issue 684 — what each speed actually buys, all the way through.
 *
 * Everything else about this figure is proved in node and in a component test.
 * Neither can prove the part that actually broke before: the tip is read off
 * the relay's row, crosses into the wasm core, is turned into a price there and
 * comes back on `FeeView`. A dropped field anywhere on that path leaves every
 * other gate green and the row empty.
 *
 * The stub chain's base fee is 1 gwei and the three tier rows sign 1.5 / 2.5 /
 * 3.5 gwei of tip, each under a cap that can deliver it whole — so the three
 * effective prices are the tips, and they are what a person reads.
 */
test('every speed says the gas price it buys, from the relay row to the screen', async ({
	page
}) => {
	await stubChain(page);
	await stubRelay(
		page,
		RELAY,
		tieredRelay(() => 'pending')
	);
	await enterWallet(page);
	await fillForm(page);

	const speed = page.getByRole('button', { expanded: false }).filter({
		hasText: en('send.feeSpeedLabel')
	});
	await speed.click();
	const option = (tier: 'fast' | 'standard' | 'slow', selected: boolean) =>
		page.getByRole('button', { pressed: selected }).filter({ hasText: en(`send.gasTier.${tier}`) });

	await expect(option('slow', false)).toContainText('ETH', { timeout: 30_000 });
	// `base + this tier's signed tip`, never the cap the relay submits at: the
	// chain charges the first and never the second.
	await expect(option('fast', true)).toContainText('3.5 gwei');
	await expect(option('standard', false)).toContainText('2.5 gwei');
	await expect(option('slow', false)).toContainText('1.5 gwei');
	// The figure is named for anyone listening, and the name is the corpus's.
	await expect(option('fast', true)).toContainText(en('send.gasPriceLabel'));
});

/**
 * Issue 685 — the gas price as a range, all the way through.
 *
 * The relay rows here are shaped the way the relay really prices a tier
 * (`vela-relay/docs/fees.md` §2b) on this 1 gwei base, 0.1 gwei tip chain:
 * caps of 1.5 / 2 / 3 × base + a tip of 1.00 / 1.25 / 2.00 × the market's.
 * Unlike `TIER_QUOTES` above, whose caps equal their tips and so bind, every
 * cap here sits above `base + tip` — so each tier's two ends differ, and the
 * top of each range only reaches the screen if the cap crossed the wasm core
 * and came back on `FeeView` as `max_gas_price`.
 */
const RANGED_TIER_QUOTES = {
	slow: {
		maxFeePerGas: '0x5f5e1000', // 1.6 gwei = 1.5 × 1 + 0.1
		maxPriorityFeePerGas: '0x5f5e100', // 0.1 gwei
		networkFeePerGas: '0x3b9aca00', // 1.0 gwei = 0.6 × 1.5 + 0.1
		relayerFeePerGas: '0x23c34600'
	},
	standard: {
		maxFeePerGas: '0x7ea8ed40', // 2.125 gwei = 2 × 1 + 0.125
		maxPriorityFeePerGas: '0x7735940', // 0.125 gwei
		networkFeePerGas: '0x4ef9e540', // 1.325 gwei
		relayerFeePerGas: '0x2faf0800'
	},
	fast: {
		maxFeePerGas: '0xbebc2000', // 3.2 gwei = 3 × 1 + 0.2
		maxPriorityFeePerGas: '0xbebc200', // 0.2 gwei
		networkFeePerGas: '0x77359400', // 2.0 gwei
		relayerFeePerGas: '0x47868c00'
	}
};

test('every speed says its gas price as bid ~ cap, from the relay row to the screen', async ({
	page
}) => {
	await stubChain(page);
	const base = happyRelay(USER_OP_HASH, TX_HASH, () => 'pending');
	await stubRelay(page, RELAY, (method, params) =>
		method === 'pimlico_getUserOperationGasPrice' ? RANGED_TIER_QUOTES : base(method, params)
	);
	await enterWallet(page);
	await fillForm(page);

	await page
		.getByRole('button', { expanded: false })
		.filter({ hasText: en('send.feeSpeedLabel') })
		.click();
	const option = (tier: 'fast' | 'standard' | 'slow', selected: boolean) =>
		page.getByRole('button', { pressed: selected }).filter({ hasText: en(`send.gasTier.${tier}`) });

	await expect(option('slow', false)).toContainText('ETH', { timeout: 30_000 });
	// Low end `base + signed tip`, high end the cap — never `base + cap + tip`,
	// which would be 3.7 / 3.25 / 2.7 gwei, above anything the chain can charge.
	await expect(option('fast', true)).toContainText('1.2 ~ 3.2 gwei');
	await expect(option('standard', false)).toContainText('1.13 ~ 2.13 gwei');
	await expect(option('slow', false)).toContainText('1.1 ~ 1.6 gwei');
});

/**
 * Wait until `count` has stopped moving for a while, and return where it
 * settled — so a count taken after it measures one action and nothing that
 * was still in flight from before.
 */
async function settledCount(page: Page, count: () => number): Promise<number> {
	let last = -1;
	while (last !== count()) {
		last = count();
		await page.waitForTimeout(1_500);
	}
	return last;
}

/**
 * How many quotes one press of ⟳ costs. Every `fee_policy` session that
 * prices this operation simulates it once (`eth_estimateUserOperationGas`,
 * never cached — the relay's tier rows are, for 15 s), and a refresh re-prices
 * every session alive: the fee in force, and any tier priced beside it.
 */
async function quotesPerRefresh(page: Page, estimates: () => number): Promise<number> {
	const before = await settledCount(page, estimates);
	await page.getByRole('button', { name: en('send.feeRefresh') }).click();
	await expect.poll(estimates, { timeout: 30_000 }).toBeGreaterThan(before);
	return (await settledCount(page, estimates)) - before;
}

test('a send nobody touched the speed of names the default out loud', async ({ page }) => {
	const sent: unknown[][] = [];
	let estimates = 0;
	await stubChain(page);
	await stubRelay(page, RELAY, (method, params) => {
		if (method === 'eth_sendUserOperation') sent.push(params);
		if (method === 'eth_estimateUserOperationGas') estimates += 1;
		return tieredRelay(() => 'pending')(method, params);
	});
	await enterWallet(page);
	await fillForm(page);

	// Issue 686 rule 1, on the route itself: the factory default IS the
	// fastest, so a free upgrade has nothing to ask about and nothing is priced
	// beside the fee in force. One refresh, one quote. (Since spec 078 a tier
	// priced alongside shares that one simulation too — see the A test below.)
	expect(await quotesPerRefresh(page, () => estimates), 'no quote beside the fee in force').toBe(1);

	const advance = page.getByRole('button', { name: en('send.continueBtn') });
	await expect(advance).toBeEnabled({ timeout: 30_000 });
	await advance.click();
	await expect(page.getByRole('heading', { name: en('send.confirmTitle') })).toBeVisible();
	// Nobody chose anything, so the confirm says nothing about speed: most sends
	// need no decision about it, and a permanent line would ask for one.
	await expect(page.getByText(en('send.feeSpeedLabel'), { exact: true })).toBeHidden();
	await page.getByRole('button', { name: en('send.confirmSendBtn') }).click();
	await expect(page.getByText(en('send.txSubmittedTitle'))).toBeVisible({ timeout: 30_000 });

	expect(sent).toHaveLength(1);
	// Named, not omitted. An absent tier leaves the relay at its own pace,
	// which is NOT the same thing as `fast` — being explicit is what makes the
	// screen and the chain agree.
	expect(sent[0][2]).toBe('fast');
});

test('the refresh really measures again, and says it is doing so', async ({ page }) => {
	let quotes = 0;
	await stubChain(page);
	await stubRelay(page, RELAY, (method, params) => {
		if (method === 'pimlico_getUserOperationGasPrice') quotes += 1;
		return tieredRelay(() => 'pending')(method, params);
	});
	await enterWallet(page);
	await fillForm(page);

	const refresh = page.getByRole('button', { name: en('send.feeRefresh') });
	await expect(refresh).toBeEnabled({ timeout: 30_000 });
	const before = quotes;
	await refresh.click();
	// `requote()` drops the 15 s fee-signal cache (issue 212) BEFORE dispatching,
	// so "look again" is a fresh measurement rather than the number already on
	// screen. Without that, this count would not move at all.
	await expect.poll(() => quotes, { timeout: 30_000 }).toBeGreaterThan(before);
});

/**
 * The stored default (spec 068 R3), which is a different lifetime from the
 * picker above: urgency is per-transaction, thrift is not. A person who always
 * wants economy must be able to say so once instead of tapping the fee down on
 * every single send.
 */
test.describe('the stored default', () => {
	test.beforeEach(async ({ page }) => {
		await seedSignedIn(page);
	});

	test('is chosen once in Settings, and is still chosen after a reload', async ({ page }) => {
		await page.goto('/en/settings');
		await expect(page.getByText('E2E Wallet').first()).toBeVisible();

		// It lives in 高级 beside the other network-behaviour rows, which is a
		// collapsed disclosure on the phone.
		await page.getByText(en('settings.sections.advanced'), { exact: true }).click();
		const row = page.getByText(en('settings.advanced.feeSpeedTitle'), { exact: true });
		await expect(row).toBeVisible();
		// Fresh device: the factory default, so nothing about a send changes
		// until somebody deliberately changes it.
		await expect(page.getByText(en('send.gasTier.fast'), { exact: true })).toBeVisible();

		await row.click();
		await expect(page.getByText(en('settings.feeSpeed.subtitle'))).toBeVisible();
		await page.getByText(en('send.gasTier.slow'), { exact: true }).click();
		await expect(page.getByText(en('send.gasTier.slow'), { exact: true })).toBeVisible();

		// A preference that does not survive was never set.
		await page.reload();
		await expect(page.getByText('E2E Wallet').first()).toBeVisible();
		await page.getByText(en('settings.sections.advanced'), { exact: true }).click();
		await expect(page.getByText(en('send.gasTier.slow'), { exact: true })).toBeVisible();
	});
});

/**
 * Issue 686 A — when the fastest speed costs no more, this send takes it.
 *
 * A chain so cheap that every tier's real cost is a sliver of a cent: 1,000
 * wei of base fee and a 100 wei tip, with relay rows shaped as the relay
 * really prices a tier (`fees.md` §2b: caps of 1.5 / 2 / 3 × base + a tip of
 * 1.00 / 1.25 / 2.00 × the market's). `fee_policy` lifts every tier to the
 * same $0.01 floor, so all three charge the same wei — while each still bids
 * a different gas price. Somebody whose stored default is Slow therefore
 * gains nothing by it here, and this send goes Fast.
 *
 * The end-to-end half is what no unit test can see: that the swap reaches
 * the WIRE (`eth_sendUserOperation`'s third element), that the fee row keeps a
 * figure through it (a quote the send machine is never told about leaves the
 * row on "…", because it refuses another tier's figure under this tier's
 * name), and that the stored preference is still Slow afterwards.
 */
const DUST_MARKET: GasMarket = { gasPrice: '0x44c', tip: '0x64', baseFee: '0x3e8' };
const DUST_TIER_QUOTES = {
	slow: {
		maxFeePerGas: '0x640', // 1,600 = 1.5 × 1,000 + 100
		maxPriorityFeePerGas: '0x64', // 100
		networkFeePerGas: '0x3e8', // 1,000 = 0.6 × 1,500 + 100
		relayerFeePerGas: '0x258'
	},
	standard: {
		maxFeePerGas: '0x855', // 2,125 = 2 × 1,000 + 125
		maxPriorityFeePerGas: '0x7d', // 125
		networkFeePerGas: '0x52d', // 1,325
		relayerFeePerGas: '0x320'
	},
	fast: {
		maxFeePerGas: '0xc80', // 3,200 = 3 × 1,000 + 200
		maxPriorityFeePerGas: '0xc8', // 200
		networkFeePerGas: '0x7d0', // 2,000
		relayerFeePerGas: '0x4b0'
	}
};

/** The stored preference as Settings would have left it (`vela.feeTier`). */
async function storeDefaultTier(page: Page, tier: string): Promise<void> {
	await page.evaluate(
		(value) =>
			new Promise<void>((resolve, reject) => {
				const open = indexedDB.open('vela', 1);
				open.onupgradeneeded = () => open.result.createObjectStore('kv');
				open.onerror = () => reject(open.error);
				open.onsuccess = () => {
					const tx = open.result.transaction('kv', 'readwrite');
					tx.objectStore('kv').put(value, 'vela.feeTier');
					tx.oncomplete = () => resolve();
					tx.onerror = () => reject(tx.error);
				};
			}),
		tier
	);
}

async function readDefaultTier(page: Page): Promise<unknown> {
	return page.evaluate(
		() =>
			new Promise<unknown>((resolve, reject) => {
				const open = indexedDB.open('vela', 1);
				open.onerror = () => reject(open.error);
				open.onsuccess = () => {
					const get = open.result.transaction('kv').objectStore('kv').get('vela.feeTier');
					get.onsuccess = () => resolve(get.result);
					get.onerror = () => reject(get.error);
				};
			})
	);
}

test('a slower default goes Fast where Fast costs no more — and says so', async ({ page }) => {
	const sent: unknown[][] = [];
	let estimates = 0;
	await stubChain(page, DUST_MARKET);
	const base = happyRelay(USER_OP_HASH, TX_HASH, () => 'pending');
	await stubRelay(page, RELAY, (method, params) => {
		if (method === 'eth_sendUserOperation') sent.push(params);
		if (method === 'eth_estimateUserOperationGas') estimates += 1;
		return method === 'pimlico_getUserOperationGasPrice' ? DUST_TIER_QUOTES : base(method, params);
	});
	await enterWallet(page);
	await storeDefaultTier(page, 'slow');
	await page.reload();
	await expect(page.getByText('$4,500', { exact: true })).toBeVisible({ timeout: 25_000 });
	await fillForm(page);

	// Folded: the tier actually in force, and the one calm line saying why —
	// Settings says Slow, so a bare "Fast" here would be the screen saying one
	// thing while the preference says another.
	const speed = page.getByRole('button', { expanded: false }).filter({
		hasText: en('send.feeSpeedLabel')
	});
	await expect(speed).toContainText(en('send.gasTier.fast'), { timeout: 30_000 });
	await expect(page.getByText(en('send.feeSpeedFree'))).toBeVisible();
	// The fee row keeps a real figure after the swap. Sampled straight off the
	// DOM rather than through a retrying assertion, which would wait out a
	// stuck "…" and pass.
	const feeRow = page.getByRole('button', { name: en('send.feeTokenLabel') });
	for (let i = 0; i < 6; i += 1) {
		const text = (await feeRow.textContent()) ?? '';
		expect(text, `sample ${i}`).not.toContain('…');
		expect(text).toMatch(/[\d.]+\s*ETH/);
		await page.waitForTimeout(200);
	}
	// The one tier priced beside the fee in force (their default, now that Fast
	// is in force) re-prices with it — on the SAME simulation (spec 078): the
	// operation is identical whatever the speed, so the session in force and
	// the partner share one relay estimate and settle together. Still one per
	// refresh, and the free upgrade — which needs the partner's settled figure
	// — still stands after it.
	expect(await quotesPerRefresh(page, () => estimates), 'one simulation for both').toBe(1);
	await expect(speed).toContainText(en('send.gasTier.fast'));
	await expect(page.getByText(en('send.feeSpeedFree'))).toBeVisible();

	const advance = page.getByRole('button', { name: en('send.continueBtn') });
	await expect(advance).toBeEnabled({ timeout: 30_000 });
	await advance.click();
	await expect(page.getByRole('heading', { name: en('send.confirmTitle') })).toBeVisible();
	// Off the person's usual pace, so the last screen before the signature says
	// so — which speed, and why (rule 5 reaches the confirm too).
	const speedFact = page.getByRole('listitem').filter({ hasText: en('send.feeSpeedLabel') });
	await expect(speedFact).toContainText(en('send.gasTier.fast'));
	await expect(speedFact).toContainText(en('send.feeSpeedFree'));
	await page.getByRole('button', { name: en('send.confirmSendBtn') }).click();
	await expect(page.getByText(en('send.txSubmittedTitle'))).toBeVisible({ timeout: 30_000 });

	expect(sent).toHaveLength(1);
	expect(sent[0][2]).toBe('fast');
	// This send's, not the person's: the stored default is untouched.
	expect(await readDefaultTier(page)).toBe('slow');
});

/**
 * The same equal-fee market, from the other side: a TAP (issue 681's
 * promotion) onto a tier that charges exactly what the one in force charges.
 *
 * The swapped-in quote costs the same wei in the same coin to the same
 * recipient, so a send machine told about a new quote only when the CHARGE
 * changes never hears of it: it keeps the old tier's estimate, and the fee
 * row, which never draws one tier's figure under another's name, sits on "…"
 * until something else happens to re-price it. Issue 686 made this path the common
 * one (every free upgrade is such a swap); the mirror now keys a quote by the
 * whole quote.
 */
test('a speed tapped at the same fee keeps its figure on the row, and on the wire', async ({
	page
}) => {
	const sent: unknown[][] = [];
	await stubChain(page, DUST_MARKET);
	const base = happyRelay(USER_OP_HASH, TX_HASH, () => 'pending');
	await stubRelay(page, RELAY, (method, params) => {
		if (method === 'eth_sendUserOperation') sent.push(params);
		return method === 'pimlico_getUserOperationGasPrice' ? DUST_TIER_QUOTES : base(method, params);
	});
	await enterWallet(page);
	await fillForm(page);

	const feeRow = page.getByRole('button', { name: en('send.feeTokenLabel') });
	await expect(feeRow).toContainText(/[\d.]+\s*ETH/, { timeout: 30_000 });
	const speed = page.getByRole('button', { expanded: false }).filter({
		hasText: en('send.feeSpeedLabel')
	});
	await speed.click();
	const slow = page.getByRole('button', { pressed: false }).filter({
		hasText: en('send.gasTier.slow')
	});
	await expect(slow).toContainText('ETH', { timeout: 30_000 });

	// Everything the fee row says from the tap onward. Read off the live DOM,
	// never through an auto-retrying assertion: the defect was a "…" that sat
	// there for a second before something else re-priced the row, and a
	// retrying `expect` simply waits it out and passes.
	await feeRow.evaluate((row) => {
		const said: string[] = [];
		(window as unknown as { __feeRowSaid: string[] }).__feeRowSaid = said;
		new MutationObserver(() => said.push(row.textContent ?? '')).observe(row, {
			subtree: true,
			childList: true,
			characterData: true
		});
	});
	await slow.click();
	await expect(speed).toContainText(en('send.gasTier.slow'));
	await page.waitForTimeout(1_500);
	const said = await page.evaluate(
		() => (window as unknown as { __feeRowSaid: string[] }).__feeRowSaid
	);
	expect(said.length, 'the observer saw the row change').toBeGreaterThan(0);
	expect(
		said.filter((text) => text.includes('…')),
		'the tapped quote took over — the row never fell back to "…"'
	).toEqual([]);
	expect(await feeRow.textContent()).toMatch(/[\d.]+\s*ETH/);

	const advance = page.getByRole('button', { name: en('send.continueBtn') });
	await expect(advance).toBeEnabled({ timeout: 30_000 });
	await advance.click();
	await page.getByRole('button', { name: en('send.confirmSendBtn') }).click();
	await expect(page.getByText(en('send.txSubmittedTitle'))).toBeVisible({ timeout: 30_000 });
	expect(sent[0][2]).toBe('slow');
});

/**
 * The link between the two surfaces (spec 068 R2/R3): what the folded control
 * shows is THEIR default, and a send nobody touches goes out at it.
 *
 * Worth its own run because it is where a plausible half-implementation
 * hides — a picker that works and a preference that is only ever read by
 * Settings would look right on both screens and submit the wrong tier.
 */
test('a stored default is what the send form starts at, with no tap at all', async ({ page }) => {
	const sent: unknown[][] = [];
	await stubChain(page);
	await stubRelay(page, RELAY, (method, params) => {
		if (method === 'eth_sendUserOperation') sent.push(params);
		return tieredRelay(() => 'pending')(method, params);
	});
	await enterWallet(page);
	// The preference as Settings would have left it — under the `vela.` prefix,
	// in the KV store `services/storage.ts` owns.
	await page.evaluate(
		() =>
			new Promise<void>((resolve, reject) => {
				const open = indexedDB.open('vela', 1);
				open.onupgradeneeded = () => open.result.createObjectStore('kv');
				open.onerror = () => reject(open.error);
				open.onsuccess = () => {
					const tx = open.result.transaction('kv', 'readwrite');
					tx.objectStore('kv').put('slow', 'vela.feeTier');
					tx.oncomplete = () => resolve();
					tx.onerror = () => reject(tx.error);
				};
			})
	);
	await page.reload();
	await expect(page.getByText('$4,500', { exact: true })).toBeVisible({ timeout: 25_000 });
	await fillForm(page);

	const speed = page.getByRole('button', { expanded: false }).filter({
		hasText: en('send.feeSpeedLabel')
	});
	await expect(speed).toContainText(en('send.gasTier.slow'));

	const advance = page.getByRole('button', { name: en('send.continueBtn') });
	await expect(advance).toBeEnabled({ timeout: 30_000 });
	await advance.click();
	await page.getByRole('button', { name: en('send.confirmSendBtn') }).click();
	await expect(page.getByText(en('send.txSubmittedTitle'))).toBeVisible({ timeout: 30_000 });
	expect(sent[0][2]).toBe('slow');
});

/**
 * Spec 069 on the dApp sheet: a fee coin picked there is part of the question
 * every speed is priced for. The machine picks USDC (spec 078); the person
 * picks ETH, then Slow — the fee is still ETH.
 *
 * Until this was pinned, only the session in force heard the coin
 * (`select_fee_asset`); the speed previews priced the OLD coin, and tapping a
 * speed promoted one — the payment switched back to ETH under the person, who
 * had just walked away from it.
 */
test('the dApp sheet keeps the coin picked when a speed is picked after it', async ({ page }) => {
	const USDC = '0x' + 'cc'.repeat(20);
	await page.addInitScript(() => localStorage.setItem('vela.dev.console', '1'));
	await stubChain(page);
	const base = tieredRelay(() => 'pending');
	await stubRelay(page, RELAY, (method, params) => {
		if (method === 'vela_getInBandGasQuote') {
			return [
				{
					recipient: '0x' + 'fe'.repeat(20),
					asset: 'native',
					feeToken: null,
					balance: '0x14d1120d7b160000',
					decimals: 18,
					symbol: 'ETH',
					usdBalance: '4500',
					usdPrice: '3000'
				},
				{
					recipient: '0x' + 'fe'.repeat(20),
					asset: 'erc20',
					feeToken: USDC,
					balance: '0x' + 100_000_000n.toString(16),
					decimals: 6,
					symbol: 'USDC',
					usdBalance: '100',
					usdPrice: '1'
				}
			];
		}
		return base(method, params);
	});
	await enterWallet(page);
	await page.waitForFunction(
		() => (window as unknown as { vela?: { requester?: unknown } }).vela?.requester !== undefined,
		null,
		{ timeout: 20_000 }
	);

	// A page asks for a plain transfer, from the account on screen.
	await page.evaluate((to) => {
		const accounts = JSON.parse(localStorage.getItem('vela.accounts') ?? '[]') as {
			address: string;
		}[];
		const from = accounts[Number(localStorage.getItem('vela.activeAccountIndex') ?? 0)].address;
		const vela = window as unknown as {
			vela: { requester: { fire(method: string, params: unknown[]): Promise<unknown> } };
		};
		void vela.vela.requester
			.fire('eth_sendTransaction', [{ from, to, value: '0x2386f26fc10000' }])
			.catch(() => undefined);
	}, RECIPIENT);
	await expect(page.getByTestId('signing-confirm')).toBeVisible({
		timeout: 25_000
	});

	// Nobody chose a coin for this request, so the fee machine did (spec 078):
	// the stablecoin the transfer does not move. Then the person picks ETH.
	const feeRow = page.getByRole('button', {
		name: new RegExp('^' + en('componentsUi.gas.networkFee'))
	});
	await expect(feeRow).toContainText('USDC', { timeout: 30_000 });
	await feeRow.click();
	await page.getByRole('button', { name: /ETH/ }).first().click();
	await expect(feeRow).toContainText('ETH', { timeout: 30_000 });
	await expect(feeRow).not.toContainText('USDC');

	// Then a speed — the preview it promotes was priced in the PICKED coin
	// too, not re-chosen by the machine behind the person's back.
	const speed = page.getByRole('button', { expanded: false }).filter({
		hasText: en('send.feeSpeedLabel')
	});
	await speed.click();
	const slow = page.getByRole('button', { pressed: false }).filter({
		hasText: en('send.gasTier.slow')
	});
	await expect(slow).toContainText('ETH', { timeout: 30_000 });
	await slow.click();
	await expect(speed).toContainText(en('send.gasTier.slow'));
	await expect(feeRow).toContainText('ETH', { timeout: 30_000 });
	await expect(feeRow).not.toContainText('USDC');
});

/**
 * Spec 079 (F11 — the owner: "可信签名器签完后，回到签名提示框，似乎没有任何提示"):
 * from the approval on, the dApp sheet is a STATUS, never the form with a
 * greyed confirm. Send dust from a page, in the parallel space (its fixture keys
 * sign for real), on the stubbed chain: "Submitting to network…" with a live ✕,
 * then the landing, which closes by itself once the chain confirms. Lives here
 * for this file's chain and relay stubs.
 */
/**
 * Send dust from a page in the parallel space, up to the confirm armed. The
 * relay holds `eth_sendUserOperation` for 1.5 s, so "submitting" can be seen;
 * `landed()` flips the receipt from pending to landed.
 */
async function dappSendArmed(page: Page): Promise<{ land: () => void }> {
	await page.addInitScript(() => localStorage.setItem('vela.dev.console', '1'));
	await stubChain(page);
	let landed = false;
	await stubRelay(
		page,
		RELAY,
		tieredRelay(() => (landed ? 'landed' : 'pending'))
	);
	// Registered after `stubRelay`, so it runs first and then hands on.
	await page.route(RELAY, async (route) => {
		const body = route.request().postDataJSON() as { method?: string } | null;
		if (body?.method === 'eth_sendUserOperation') await new Promise((r) => setTimeout(r, 1_500));
		await route.fallback();
	});
	await enterWallet(page);
	await page.waitForFunction(
		() => (window as unknown as { vela?: { requester?: unknown } }).vela?.requester !== undefined,
		null,
		{ timeout: 20_000 }
	);
	await page.evaluate((to) => {
		const accounts = JSON.parse(localStorage.getItem('vela.accounts') ?? '[]') as {
			address: string;
		}[];
		const from = accounts[Number(localStorage.getItem('vela.activeAccountIndex') ?? 0)].address;
		const vela = window as unknown as {
			vela: { requester: { fire(method: string, params: unknown[]): Promise<unknown> } };
			__answer?: unknown;
			__error?: unknown;
		};
		void vela.vela.requester
			.fire('eth_sendTransaction', [{ from, to, value: '0x1' }])
			.then((answer) => (vela.__answer = answer))
			.catch((error) => (vela.__error = error));
	}, RECIPIENT);
	await expect(page.getByTestId('signing-confirm')).toBeEnabled({ timeout: 30_000 });
	return { land: () => (landed = true) };
}

test('a dApp send reads as a status from the approval, and its landing closes by itself', async ({
	page
}) => {
	const { land } = await dappSendArmed(page);
	const confirm = page.getByTestId('signing-confirm');

	// 083 H3: one call, no calldata — the page only moves the chain's own coin.
	// The sheet says so (what, how much, to whom), exact to the wei, and not
	// "unable to decode (0 bytes)" in red, which is what it drew before.
	const sheet = page.getByRole('dialog').filter({ has: confirm });
	await expect(sheet.locator('p.intent')).toHaveText(en('componentsUi.signing.intentSend'));
	// U+2212, the sheet's minus (spec 082 G19/RJ15) — never an ASCII hyphen.
	await expect(sheet.getByText('\u22120.000000000000000001', { exact: true })).toBeVisible();
	await expect(
		sheet.getByText(en('componentsUi.signing.recipientLabel'), { exact: true })
	).toBeVisible();
	const blindHead = en('componentsUi.signing.blindDecodeWarning').split('{{')[0].trim();
	await expect(sheet.getByText(blindHead)).toHaveCount(0);

	// From here on, record every frame the sheet draws: a greyed confirm must
	// never appear (busy is never drawn as disabled), and the status must.
	await page.evaluate(() => {
		const seen = { dimmedConfirm: false, status: [] as string[] };
		(window as unknown as { __seen: typeof seen }).__seen = seen;
		const look = () => {
			if (document.querySelector('button[data-testid="signing-confirm"]:disabled')) {
				seen.dimmedConfirm = true;
			}
			const status = document.querySelector('[data-testid="signing-status"]');
			const text = status?.textContent?.trim();
			if (text && !seen.status.includes(text)) seen.status.push(text);
		};
		new MutationObserver(look).observe(document.body, {
			subtree: true,
			childList: true,
			attributes: true,
			characterData: true
		});
	});
	await confirm.click();

	const status = page.getByTestId('signing-status');
	await expect(status).toContainText(en('send.txSubmitting'), { timeout: 20_000 });
	await expect(status).toContainText(en('send.txBackgroundHint'));
	// Signed and on its way: the ✕ closes without refusing — it is live.
	await expect(
		page.getByRole('button', { name: en('componentsUi.signing.close'), exact: true })
	).toBeEnabled();

	// The relay took it: the landing, with the chain's clock.
	const receipt = page.getByTestId('dapp-receipt');
	await expect(receipt).toContainText(en('componentsTx.receipt.statusSubmitted'), {
		timeout: 20_000
	});
	land();
	await expect(receipt).toContainText(en('componentsTx.receipt.statusConfirmed'), {
		timeout: 30_000
	});
	// …and a success closes by itself (~2.6 s), with no tap.
	await expect(receipt).toBeHidden({ timeout: 8_000 });
	await expect
		.poll(() => page.evaluate(() => (window as unknown as { __answer?: unknown }).__answer))
		.toBeTruthy();

	const seen = await page.evaluate(
		() => (window as unknown as { __seen: { dimmedConfirm: boolean; status: string[] } }).__seen
	);
	expect(seen.dimmedConfirm).toBe(false);
	expect(seen.status.some((text) => text.includes(en('send.txSubmitting')))).toBe(true);
	// 083 H3: from the approval to the passkey prompt the wallet is preparing
	// (funding, nonce, estimate) — "Waiting for biometric" only once the prompt
	// is up (the parallel space's signer still runs inside the ceremony), then
	// submitting. Frame order, first sight of each.
	const first = (key: string) => seen.status.findIndex((text) => text.includes(en(key)));
	expect(first('send.txPreparing'), seen.status.join(' | ')).toBeGreaterThanOrEqual(0);
	expect(first('send.txSigning'), seen.status.join(' | ')).toBeGreaterThan(
		first('send.txPreparing')
	);
	expect(first('send.txSubmitting'), seen.status.join(' | ')).toBeGreaterThan(
		first('send.txSigning')
	);
});

/**
 * Spec 079: after the approval the ✕ closes without refusing — the operation
 * goes on and the page still gets its answer — and nothing of it comes back
 * over the page when the answer lands (Android 002f5a5c, iOS the same).
 */
test('a dApp send closed after approving is still answered, and no landing comes back', async ({
	page
}) => {
	const { land } = await dappSendArmed(page);
	land();
	await page.getByTestId('signing-confirm').click();

	const status = page.getByTestId('signing-status');
	await expect(status).toContainText(en('send.txSubmitting'), { timeout: 20_000 });
	await page.getByRole('button', { name: en('componentsUi.signing.close'), exact: true }).click();
	await expect(status).toBeHidden();
	// Watch every frame from the close on: a receipt must never rise again.
	await page.evaluate(() => {
		const seen = { receipt: false };
		(window as unknown as { __afterClose: typeof seen }).__afterClose = seen;
		new MutationObserver(() => {
			if (document.querySelector('[data-testid="dapp-receipt"]')) seen.receipt = true;
		}).observe(document.body, { subtree: true, childList: true });
	});

	// The page is answered with the transaction — not refused.
	await expect
		.poll(() => page.evaluate(() => (window as unknown as { __answer?: unknown }).__answer), {
			timeout: 30_000
		})
		.toBeTruthy();
	expect(await page.evaluate(() => (window as unknown as { __error?: unknown }).__error)).toBe(
		undefined
	);
	// …and no receipt rose over the wallet the person went back to.
	await page.waitForTimeout(1_500);
	expect(
		await page.evaluate(
			() => (window as unknown as { __afterClose: { receipt: boolean } }).__afterClose.receipt
		)
	).toBe(false);
	await expect(page.getByTestId('dapp-receipt')).toHaveCount(0);
});

/**
 * PR 3 — the confirm waits for the simulation's verdict, four seconds at most.
 *
 * The confirm gate looked at the request, the reading, the approval guard and
 * the fee, and not at the sheet's own simulation: a person could confirm
 * before the one part of the sheet a site cannot write was on screen. Here,
 * on the real machines, with a node that is asked to simulate and holds its
 * answer: the confirm is shut with the core's line under it; at the core's
 * deadline it opens beside the caution that nothing could be checked, in the
 * verdict's place; and through all of it the confirm, in the sheet's foot,
 * does not move.
 *
 * What comes after the deadline has two endings, and the caution outlives
 * neither wrongly: the pool gives up — "could not check" IS then the verdict,
 * and the caution stays, the same card (it used to leave the sheet at that
 * moment, said and taken back over an open confirm); or the node answers
 * after all, and its verdict takes the caution's place. And a node that says
 * at once that it does not offer the simulation draws the caution with no
 * wait at all, and it stays past the deadline of a wait that is over.
 *
 * Lives here for this file's chain and relay stubs: the fee has to be READY
 * for the simulation to be the one thing holding the confirm.
 */

/**
 * What stands in the sheet's foot and in its verdict's place right now: can
 * the confirm be pressed, the line shown under it, and what the
 * balance-changes card says.
 */
async function verdictPlace(page: Page) {
	return page.evaluate(() => {
		const sheet = document.querySelector('[role="dialog"]');
		const confirm = sheet?.querySelector<HTMLButtonElement>('[data-testid="signing-confirm"]');
		const line = sheet?.querySelector<HTMLElement>('.confirm-note');
		return {
			disabled: confirm ? confirm.disabled : true,
			line:
				line && getComputedStyle(line).visibility !== 'hidden'
					? (line.textContent?.trim() ?? '')
					: null,
			said: sheet?.querySelector('section.balances .note')?.textContent?.trim() ?? null,
			top: confirm ? confirm.getBoundingClientRect().top : null
		};
	});
}

/**
 * The sheet is waiting for its simulation: held with the core's line, or —
 * past the deadline — open beside the caution. Every look on the way there
 * is checked: a confirm that can be pressed over an empty verdict's place is
 * the hole this closes. Answers which of the two it came to rest in.
 */
async function waitsForTheVerdict(page: Page): Promise<'held' | 'cautioned'> {
	const checking = en('componentsUi.signing.confirmBlock.simChecking');
	const couldNot = en('componentsUi.signing.simUnavailableWarning');
	expect(checking).toBe('Checking what this transaction does…');
	let rested: 'held' | 'cautioned' | null = null;
	const looks: string[] = [];
	await expect
		.poll(
			async () => {
				const now = await verdictPlace(page);
				looks.push(JSON.stringify(now));
				if (now.disabled && now.line === checking && now.said === null) rested = 'held';
				else if (!now.disabled && now.said === couldNot) rested = 'cautioned';
				return rested;
			},
			{ timeout: 25_000, intervals: [100] }
		)
		.not.toBeNull();
	// Open, and nothing in the verdict's place: never, in any look.
	const open = looks.filter((look) => {
		const now = JSON.parse(look) as { disabled: boolean; said: string | null };
		return !now.disabled && now.said !== couldNot;
	});
	expect(open, looks.join('\n')).toEqual([]);
	return rested!;
}

/** The stub's node, as it answers `eth_simulateV1`. */
interface SimulatingNode {
	/** How many times it was asked, and when it last heard anything (an ask, or its release). */
	asked: number;
	lastAskedAt: number;
	/**
	 * Let a held node go: `checked` — it ran, and nothing of the account's
	 * moves; `fail` — every request to it fails from here on, which is the
	 * pool giving up.
	 */
	release(how: 'checked' | 'fail'): void;
	released: boolean;
}

/**
 * The wallet on the stubbed chain, with a node that either HOLDS its answer
 * to the simulation until released, or says at once that it does not offer
 * it — and a page's request on the sheet: a call that carries no value.
 */
async function raiseSimulated(
	page: Page,
	size: { width: number; height: number },
	mode: 'hold' | 'not_offered'
): Promise<SimulatingNode> {
	await page.setViewportSize(size);
	await page.addInitScript(() => localStorage.setItem('vela.dev.console', '1'));
	await stubChain(page);
	await stubRelay(
		page,
		RELAY,
		tieredRelay(() => 'pending')
	);
	let how: 'checked' | 'fail' | null = null;
	let open: () => void = () => {};
	const gate = new Promise<void>((resolve) => (open = resolve));
	const node: SimulatingNode = {
		asked: 0,
		lastAskedAt: 0,
		released: false,
		release: (ending) => {
			how = ending;
			node.released = true;
			node.lastAskedAt = Date.now();
			open();
		}
	};
	// Registered after `stubChain`, so it runs first.
	await page.route(/stub-rpc\.test\/rpc\/1$/, async (route) => {
		const body = route.request().postDataJSON() as { method?: string; id?: number } | null;
		if (body?.method !== 'eth_simulateV1') return route.fallback();
		node.asked += 1;
		node.lastAskedAt = Date.now();
		const reply = (answer: Record<string, unknown>) =>
			route.fulfill({
				contentType: 'application/json',
				body: JSON.stringify({ jsonrpc: '2.0', id: body.id ?? 1, ...answer })
			});
		if (mode === 'not_offered') {
			return reply({
				error: {
					code: -32601,
					message: 'the method eth_simulateV1 does not exist/is not available'
				}
			});
		}
		await gate;
		if (how === 'fail') return route.abort('failed');
		// It ran, and nothing of the account's moves.
		return reply({ result: [{ calls: [{ status: '0x1', logs: [] }] }] });
	});
	await enterWallet(page);
	await page.waitForFunction(
		() => (window as unknown as { vela?: { requester?: unknown } }).vela?.requester !== undefined,
		null,
		{ timeout: 20_000 }
	);
	await page.evaluate((to) => {
		const accounts = JSON.parse(localStorage.getItem('vela.accounts') ?? '[]') as {
			address: string;
		}[];
		const from = accounts[Number(localStorage.getItem('vela.activeAccountIndex') ?? 0)].address;
		const vela = window as unknown as {
			vela: { requester: { fire(method: string, params: unknown[]): Promise<unknown> } };
		};
		void vela.vela.requester
			.fire('eth_sendTransaction', [{ from, to, value: '0x0' }])
			.catch(() => undefined);
	}, RECIPIENT);
	await expect(page.getByTestId('signing-confirm')).toBeVisible({ timeout: 25_000 });
	await expect.poll(() => node.asked, { timeout: 20_000 }).toBeGreaterThanOrEqual(1);
	return node;
}

/** Where the confirm rests: the same place twice running. */
async function restingTop(page: Page): Promise<number> {
	let last = -1;
	await expect
		.poll(
			async () => {
				const now = (await verdictPlace(page)).top ?? -1;
				const still = now === last;
				last = now;
				return still;
			},
			{ timeout: 20_000, intervals: [200] }
		)
		.toBe(true);
	return last;
}

/**
 * Look at the sheet again and again until `over()` — and in every look the
 * confirm can be pressed, no line stands under it, the caution is in the
 * verdict's place, and the confirm is where it was. What must NOT happen has
 * only a watch to stand on; what ends the watch is a condition.
 */
async function cautionStands(page: Page, top: number, over: () => boolean): Promise<number> {
	const couldNot = en('componentsUi.signing.simUnavailableWarning');
	let looks = 0;
	await expect
		.poll(
			async () => {
				const now = await verdictPlace(page);
				looks += 1;
				expect(now, `look ${looks}`).toEqual({
					disabled: false,
					line: null,
					said: couldNot,
					top
				});
				return over();
			},
			{ timeout: 60_000, intervals: [150] }
		)
		.toBe(true);
	return looks;
}

const SCREENS = [
	['the phone sheet', 390, 844],
	['the centred card', 1400, 900]
] as const;

for (const [name, width, height] of SCREENS) {
	for (const ending of ['the pool gives up', 'the node answers after all'] as const) {
		test(`the confirm waits for a node that does not answer in time (${name}), then ${ending}: held, open beside the caution, never moved`, async ({
			page
		}, testInfo) => {
			const node = await raiseSimulated(page, { width, height }, 'hold');
			const confirm = page.getByTestId('signing-confirm');
			const couldNot = en('componentsUi.signing.simUnavailableWarning');
			const card = page.locator('[role="dialog"] section.balances');

			// Held, with the line under it — unless this machine was slow enough
			// for the deadline to pass first, which is the next state, checked below.
			const first = await waitsForTheVerdict(page);
			const linePresent = first === 'held' ? await restingTop(page) : null;
			if (first === 'held') await page.screenshot({ path: testInfo.outputPath('held.png') });

			// The deadline: the caution lands, the confirm opens, the line is gone.
			await expect(card).toContainText(couldNot, { timeout: 20_000 });
			await expect(card.locator('.note')).toHaveAttribute('data-tone', 'caution');
			await expect(card).toHaveAttribute('data-verdict', '');
			await expect(confirm).toBeEnabled();
			expect((await verdictPlace(page)).line).toBeNull();
			const lineAbsent = await restingTop(page);
			await expect(confirm).toBeInViewport({ ratio: 1 });
			await expect(card).toBeInViewport({ ratio: 1 });
			await page.screenshot({ path: testInfo.outputPath('cautioned.png') });

			let after: number;
			if (ending === 'the pool gives up') {
				// Every request to the node fails from here on. The pool tries it
				// again, and then stops asking: it has given up, and the read
				// ends "unreachable". The caution was there in every look on the
				// way, and is there when the asking has been over for a while.
				const askedBefore = node.asked;
				node.release('fail');
				const looks = await cautionStands(
					page,
					lineAbsent,
					() => node.released && Date.now() - node.lastAskedAt > 4_000
				);
				expect(looks).toBeGreaterThan(10);
				expect(node.asked).toBeGreaterThanOrEqual(askedBefore);
				await expect(card).toHaveCount(1);
				await expect(card).toContainText(couldNot);
				await expect(card.locator('.note')).toHaveAttribute('data-tone', 'caution');
				after = await restingTop(page);
				await page.screenshot({ path: testInfo.outputPath('gave-up.png') });
			} else {
				// Its verdict takes the caution's place.
				node.release('checked');
				await expect(card).toContainText(en('componentsUi.signing.simResultNoChange'), {
					timeout: 20_000
				});
				await expect(page.getByText(couldNot)).toHaveCount(0);
				await expect(confirm).toBeEnabled();
				after = await restingTop(page);
				expect((await verdictPlace(page)).line).toBeNull();
				await page.screenshot({ path: testInfo.outputPath('answered.png') });
			}

			testInfo.annotations.push({
				type: 'confirm-top',
				description: `${width}x${height}, ${ending}: line present ${linePresent}, at the deadline ${lineAbsent}, after ${after}`
			});
			if (linePresent !== null) expect(lineAbsent).toBe(linePresent);
			expect(after).toBe(lineAbsent);
		});
	}
}

test('a node that does not offer the simulation: the caution is on the sheet, and stays past the deadline', async ({
	page
}, testInfo) => {
	const node = await raiseSimulated(page, { width: 390, height: 844 }, 'not_offered');
	const firstAsked = node.lastAskedAt;
	const confirm = page.getByTestId('signing-confirm');
	const couldNot = en('componentsUi.signing.simUnavailableWarning');
	const card = page.locator('[role="dialog"] section.balances');

	await expect(card).toContainText(couldNot, { timeout: 20_000 });
	await expect(card.locator('.note')).toHaveAttribute('data-tone', 'caution');
	await expect(card.locator('.row')).toHaveCount(0);
	// The fee has its say, then the confirm opens: nothing else holds it.
	await expect(confirm).toBeEnabled({ timeout: 30_000 });
	const top = await restingTop(page);
	await page.screenshot({ path: testInfo.outputPath('not-offered.png') });

	// The wait's deadline was started with the simulation and is four seconds
	// long. Watched well past it: the caution neither goes nor is drawn anew.
	const looks = await cautionStands(page, top, () => Date.now() - firstAsked > 7_000);
	expect(looks).toBeGreaterThan(5);
	await expect(card).toHaveCount(1);
	await expect(confirm).toBeInViewport({ ratio: 1 });
	testInfo.annotations.push({
		type: 'confirm-top',
		description: `390x844, not offered: ${top} throughout (${looks} looks)`
	});
});
