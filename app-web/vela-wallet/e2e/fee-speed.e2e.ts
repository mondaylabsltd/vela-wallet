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
	// beside the fee in force. One refresh, one quote. (The A test below makes
	// the same measurement with a Slow default and gets two, so this count
	// really does see a tier priced alongside.)
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
	// is in force) re-prices with it: two quotes per refresh — the control for
	// the Fast-default count of one above.
	expect(await quotesPerRefresh(page, () => estimates), 'the partner is priced too').toBe(2);
	await expect(speed).toContainText(en('send.gasTier.fast'));

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
