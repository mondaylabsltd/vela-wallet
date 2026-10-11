/**
 * The signing sheet draws what the transaction does — on the real machines,
 * against a stubbed chain.
 *
 * The simulation's verdict is the one part of a signing sheet the site being
 * signed for cannot write. The phones and the desktop drew it; the web ran
 * the same `eth_simulateV1` read for its fee and drew two sentences of it.
 * Here the whole path is real — the request, the reading, the fee, the
 * `sign_request` core's wait for the verdict, the core's reading of the node's
 * reply (`simOutcome`), the `token_trust` machine's judgment of a check's
 * moves, the builder and the drawn sheet — and only the chain and the relay
 * are stood in for, per call. The node HOLDS its answer to the simulation
 * until the test lets it go, so what the sheet says before and after are two
 * states a test can look at.
 *
 * What is held:
 *
 * - **what moves is listed** — "−10 USDC", "+0.0031 ETH": the core's figures,
 *   in the core's order; a token nobody vouches for is a direction and never
 *   a figure, with the one warning under the card;
 * - **a call that fails says so** — "Expected to fail", with the reason the
 *   core cleaned, as a danger, once;
 * - **the confirm waits for it** — shut with the core's line while the
 *   verdict's place says "Checking…", open beside the verdict;
 * - **nothing moves when it lands** — the place is on the sheet from the
 *   first frame: the sheet's top, its ✕, the request, the fee row, the signing
 *   account and the confirm are where they were, at 320 × 700, 390 × 844 and
 *   1440 × 900.
 *
 * Nothing leaves the machine, and nothing is signed.
 */
import { expect, test, type Page, type TestInfo } from '@playwright/test';
import { CHAINS } from '../src/lib/services/chains';
import {
	NATIVE_SENTINEL,
	TRANSFER_TOPIC,
	errorString,
	simulatedBlock
} from '../src/lib/signing/sim-replies';
import { en, zh } from './live-helpers';
import {
	abiWord,
	denyOffOrigin,
	happyRelay,
	seedNetworkOverrides,
	stubChainRegistry,
	stubJsonRpc,
	stubRelay
} from './stub-chain';
import { MULTICALL_SEL, abiString, answerAggregate3, isAggregate3 } from './stub-multicall';

test.setTimeout(120_000);

const STUB = 'https://stub-rpc.test/rpc';
const RELAY = /vela-relay(-cf)?\.getvela\.app/;
const ETH = 1_000_000_000_000_000_000n;
const PRICE_8DP = 3000n * 100_000_000n;
/** Mainnet USDC: the core's own registry knows it (six decimals). */
const USDC = '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48';
/** A contract the site controls. Asked for its symbol it answers "USDC" — as any contract can. */
const LURE = '0x' + 'ba'.repeat(20);
const SITE = '0x' + '51'.repeat(20);
const RECIPIENT = '0x' + 'ab'.repeat(20);

const word = (value: bigint | number) => BigInt(value).toString(16).padStart(64, '0');
const topic = (address: string) => '0x' + address.slice(2).toLowerCase().padStart(64, '0');
const transfer = (token: string, from: string, to: string, amount: bigint) => ({
	address: token,
	topics: [TRANSFER_TOPIC, topic(from), topic(to)],
	data: '0x' + word(amount)
});

/** `transfer(to, amount)` — what the page asks the wallet to send. */
function transferCalldata(to: string, amount: bigint): string {
	return '0xa9059cbb' + to.slice(2).toLowerCase().padStart(64, '0') + word(amount);
}

/**
 * A calm mainnet with a deployed account holding 1.5 ETH at $3,000 —
 * `fee-speed`'s chain. Token metadata is answered per call: the lure says
 * "USDC", 18 decimals; nothing else names itself.
 */
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
		if (method === 'eth_maxPriorityFeePerGas') return '0x5f5e100';
		if (method === 'eth_getBlockByNumber') return { baseFeePerGas: '0x3b9aca00' };
		if (method === 'eth_getLogs') return [];
		if (method === 'eth_call') {
			const call = params[0] as { data?: string } | undefined;
			if (!call?.data || !isAggregate3(call.data)) return '0x' + abiWord(0);
			const balance = chainId === 1 ? (ETH * 3n) / 2n : 0n;
			// One blob reads as a balance (word 0) and as a price feed's answer (word 1).
			const blob =
				'0x' + abiWord(balance) + abiWord(PRICE_8DP) + abiWord(0) + abiWord(0) + abiWord(0);
			return answerAggregate3(call.data, (inner) => {
				if (inner.target === LURE) {
					if (inner.selector === MULTICALL_SEL.symbol)
						return { success: true, data: abiString('USDC') };
					if (inner.selector === MULTICALL_SEL.decimals)
						return { success: true, data: '0x' + word(18) };
					return undefined;
				}
				if (inner.selector === MULTICALL_SEL.symbol || inner.selector === MULTICALL_SEL.decimals) {
					return undefined;
				}
				return { success: true, data: blob };
			});
		}
		return undefined;
	});
}

/** The stub's node, as it answers `eth_simulateV1`: held until the test lets it go. */
interface SimulatingNode {
	asked: number;
	/** The account that signs, as the wallet named it to the node. */
	account: string;
	/** Answer every held (and later) ask with this JSON-RPC `result`, built for the account. */
	release(result: (account: string) => object[]): void;
}

/**
 * The parallel space's wallet on the stubbed chain, in `locale`, with a
 * page's `eth_sendTransaction` on the sheet and the node holding its answer
 * to the simulation.
 */
async function raise(
	page: Page,
	size: { width: number; height: number },
	locale: 'en' | 'zh',
	tx: { to: string; data?: string; value?: string }
): Promise<SimulatingNode> {
	await page.setViewportSize(size);
	await page.addInitScript(() => localStorage.setItem('vela.dev.console', '1'));
	await stubChain(page);
	await stubRelay(page, RELAY, happyRelay('0x' + 'a1'.repeat(32), '0x' + 'b2'.repeat(32)));
	let result: ((account: string) => object[]) | null = null;
	let open: () => void = () => {};
	const gate = new Promise<void>((resolve) => (open = resolve));
	const node: SimulatingNode = {
		asked: 0,
		account: '',
		release: (reply) => {
			result = reply;
			open();
		}
	};
	// Registered after `stubChain`, so it runs first.
	await page.route(/stub-rpc\.test\/rpc\/1$/, async (route) => {
		const body = route.request().postDataJSON() as {
			method?: string;
			id?: number;
			params?: [{ blockStateCalls?: { calls?: { from?: string }[] }[] }];
		} | null;
		if (body?.method !== 'eth_simulateV1') return route.fallback();
		node.asked += 1;
		node.account = body.params?.[0]?.blockStateCalls?.[0]?.calls?.[0]?.from ?? '';
		await gate;
		return route.fulfill({
			contentType: 'application/json',
			body: JSON.stringify({ jsonrpc: '2.0', id: body.id ?? 1, result: result!(node.account) })
		});
	});

	await page.goto('/en/parallel');
	// The button is in the server's HTML before the page hydrates; the fixture
	// list fills on mount — a row in it means the handlers are on.
	await page.locator('li code').first().waitFor({ timeout: 60_000 });
	await page.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
	await page.waitForURL(/\/en\/wallet$/);
	await seedNetworkOverrides(
		page,
		CHAINS.map((c) => ({ chainId: c.chainId, rpcURL: `${STUB}/${c.chainId}` }))
	);
	await page.goto(`/${locale}/wallet`);
	await page.waitForFunction(
		() => (window as unknown as { vela?: { requester?: unknown } }).vela?.requester !== undefined,
		null,
		{ timeout: 30_000 }
	);
	await page.evaluate((request) => {
		const accounts = JSON.parse(localStorage.getItem('vela.accounts') ?? '[]') as {
			address: string;
		}[];
		const from = accounts[Number(localStorage.getItem('vela.activeAccountIndex') ?? 0)].address;
		const vela = window as unknown as {
			vela: { requester: { fire(method: string, params: unknown[]): Promise<unknown> } };
		};
		void vela.vela.requester
			.fire('eth_sendTransaction', [{ from, value: '0x0', ...request }])
			.catch(() => undefined);
	}, tx);
	await expect(page.getByTestId('signing-confirm')).toBeVisible({ timeout: 30_000 });
	await expect.poll(() => node.asked, { timeout: 30_000 }).toBeGreaterThanOrEqual(1);
	return node;
}

/** Everything a person could be reading, or reaching for — and what the sheet says. */
async function look(page: Page) {
	return page.evaluate(() => {
		const sheet = document.querySelector<HTMLElement>('[role="dialog"]')!;
		const at = (selector: string) => {
			const el = sheet.querySelector(selector);
			return el ? Math.round(el.getBoundingClientRect().top * 10) / 10 : null;
		};
		const place = sheet.querySelector<HTMLElement>('[data-testid="signing-verdict"]');
		const said = place?.querySelector<HTMLElement>('.said') ?? null;
		const confirm = sheet.querySelector<HTMLButtonElement>('[data-testid="signing-confirm"]');
		const line = sheet.querySelector<HTMLElement>('.confirm-note');
		const scroller = sheet.querySelector<HTMLElement>('.content')!;
		return {
			where: {
				sheet: Math.round(sheet.getBoundingClientRect().top * 10) / 10,
				height: Math.round(sheet.getBoundingClientRect().height * 10) / 10,
				close: at('header.header .close'),
				request: at('.blocks > :first-child'),
				place: at('[data-testid="signing-verdict"]'),
				placeHeight: place ? Math.round(place.getBoundingClientRect().height * 10) / 10 : null,
				details: at('section.tech'),
				fee: at('.footer > .line'),
				signer: at('.footer > .signer'),
				confirm: at('[data-testid="signing-confirm"]'),
				scrolled: Math.round(scroller.scrollTop)
			},
			/** The rows of the balance card in the place: "USDC −10". */
			rows: [...(said?.querySelectorAll('section.balances .row') ?? [])].map((row) =>
				(row.textContent ?? '').replace(/\s+/g, ' ').trim()
			),
			/** The one line said in the place: the card's note, or the notice. */
			line:
				(said?.querySelector('.banner p') ?? said?.querySelector('.note'))?.textContent?.trim() ??
				null,
			tone:
				said?.querySelector<HTMLElement>('.banner')?.dataset.tone ??
				said?.querySelector<HTMLElement>('.note')?.dataset.tone ??
				null,
			stage: place?.dataset.stage ?? null,
			disabled: confirm ? confirm.disabled : true,
			held:
				line && getComputedStyle(line).visibility !== 'hidden'
					? (line.textContent?.trim() ?? '')
					: null
		};
	});
}

/** Where everything RESTS: the same figures twice running, past the sheet's entrance. */
async function resting(page: Page) {
	let last = '';
	let now = await look(page);
	await expect
		.poll(
			async () => {
				now = await look(page);
				const key = JSON.stringify(now.where);
				const still = key === last;
				last = key;
				return still;
			},
			{ timeout: 30_000, intervals: [250] }
		)
		.toBe(true);
	return now;
}

/**
 * The sheet before the verdict: the place is there, and either the confirm is
 * held with the core's line over "Checking…", or — on a machine slow enough
 * for the core's four seconds to pass first — open beside the caution that
 * nothing was checked. Never a confirm that can be pressed over "Checking…".
 */
async function beforeTheVerdict(page: Page, words: (key: string) => string) {
	const holds = words('componentsUi.signing.confirmBlock.simChecking');
	const checking = words('componentsUi.funding.checking');
	const couldNot = words('componentsUi.signing.simUnavailableWarning');
	const looks: Awaited<ReturnType<typeof look>>[] = [];
	// The fee has had its say too: the simulation is all that holds the confirm.
	await expect
		.poll(
			async () => {
				const now = await look(page);
				looks.push(now);
				if (now.disabled && now.held === holds && now.line === checking) return 'held';
				if (!now.disabled && now.line === couldNot) return 'cautioned';
				return null;
			},
			{ timeout: 40_000, intervals: [100] }
		)
		.not.toBeNull();
	for (const now of looks) {
		expect(now.where.place, 'the place is there in every look').not.toBeNull();
		expect(!now.disabled && now.line === checking, JSON.stringify(now)).toBe(false);
	}
	return resting(page);
}

const SIZES = [
	['a small phone', 320, 700],
	['the phone sheet', 390, 844],
	['the centred card', 1440, 900]
] as const;

/** The measurements, for the record of the run. */
function record(testInfo: TestInfo, what: string, before: object, after: object): void {
	testInfo.annotations.push({
		type: 'geometry',
		description: `${what}: before ${JSON.stringify(before)} — after ${JSON.stringify(after)}`
	});
}

for (const [name, width, height] of SIZES) {
	test(`a transaction that moves assets lists what moves, and its landing moves nothing — ${name} (${width}×${height})`, async ({
		page
	}, testInfo) => {
		// The page asks to send 10 USDC; run on the chain, 10 USDC leaves and
		// 0.0031 ETH comes back.
		const node = await raise(page, { width, height }, 'en', {
			to: USDC,
			data: transferCalldata(RECIPIENT, 10_000_000n)
		});
		const before = await beforeTheVerdict(page, en);
		expect(before.rows).toEqual([]);
		expect(before.stage === 'out' || before.line !== en('componentsUi.funding.checking')).toBe(
			true
		);
		await page.screenshot({ path: testInfo.outputPath(`moves-before-en-${width}x${height}.png`) });

		node.release((me) =>
			simulatedBlock({
				status: '0x1',
				logs: [
					transfer(USDC, me, RECIPIENT, 10_000_000n),
					transfer(NATIVE_SENTINEL, SITE, me, 3_100_000_000_000_000n)
				]
			})
		);
		await expect
			.poll(async () => (await look(page)).rows, { timeout: 30_000 })
			.toEqual(['USDC −10', 'ETH +0.0031']);
		const after = await resting(page);

		// The card, in the place: its title, its two rows, no caution, no notice.
		const place = page.getByTestId('signing-verdict');
		await expect(place.locator('.said h3')).toHaveText(
			en('componentsUi.signing.balanceChangesTitle')
		);
		expect(after.line).toBeNull();
		expect(after.stage).toBe('landed');
		await expect(place.locator('.said .banner')).toHaveCount(0);
		// What leaves is plain ink; what arrives is a gain.
		await expect(place.locator('.said .row .delta').nth(0)).toHaveAttribute('data-tone', 'neutral');
		await expect(place.locator('.said .row .delta').nth(1)).toHaveAttribute('data-tone', 'success');
		// The confirm waited for it, and is open beside it — with no line under it.
		expect(after.disabled).toBe(false);
		expect(after.held).toBeNull();
		// Whole, and in sight, with the confirm whole too.
		await expect(place.locator('.said')).toBeInViewport({ ratio: 1 });
		await expect(page.getByTestId('signing-confirm')).toBeInViewport({ ratio: 1 });

		// NOTHING MOVED: the sheet's top and height, its ✕, the request, the
		// place and its height, the details, the fee row, the account, the confirm.
		record(testInfo, `${width}x${height} moves assets`, before.where, after.where);
		expect(after.where).toEqual(before.where);
		// One read served the fee and the sheet.
		expect(node.asked).toBe(1);
		await page.screenshot({ path: testInfo.outputPath(`moves-after-en-${width}x${height}.png`) });
	});

	test(`a transaction that is expected to fail says so, with its reason, and its landing moves nothing — ${name} (${width}×${height})`, async ({
		page
	}, testInfo) => {
		const node = await raise(page, { width, height }, 'en', {
			to: USDC,
			data: transferCalldata(RECIPIENT, 10_000_000n)
		});
		const before = await beforeTheVerdict(page, en);
		await page.screenshot({ path: testInfo.outputPath(`fails-before-en-${width}x${height}.png`) });

		const reason = 'ERC20: transfer amount exceeds balance';
		node.release(() =>
			simulatedBlock({
				status: '0x0',
				returnData: errorString(reason),
				error: { code: 3, message: 'execution reverted' }
			})
		);
		const sentence = en('componentsUi.signing.simWillFailReason').replace('{{reason}}', reason);
		expect(sentence).toBe(`Expected to fail: ${reason} — you’d still pay gas.`);
		await expect.poll(async () => (await look(page)).line, { timeout: 30_000 }).toBe(sentence);
		const after = await resting(page);

		// The danger, as an alert, in the place — and no balance card: a call
		// that fails moves nothing, and "nothing moves" is not what it says.
		const place = page.getByTestId('signing-verdict');
		expect(after.tone).toBe('danger');
		expect(after.rows).toEqual([]);
		await expect(place.locator('.said .banner')).toHaveAttribute('role', 'alert');
		await expect(place.locator('.said section.balances')).toHaveCount(0);
		const sheet = page.getByRole('dialog');
		await expect(sheet.getByText(en('componentsUi.signing.simResultNoChange'))).toHaveCount(0);
		await expect(sheet.getByText(en('componentsUi.signing.simUnavailableWarning'))).toHaveCount(0);
		// Said once on the sheet.
		await expect(sheet.getByRole('alert')).toHaveCount(1);
		// A warning informs, it never blocks: the confirm is open beside it.
		expect(after.disabled).toBe(false);
		expect(after.held).toBeNull();
		await expect(place.locator('.said')).toBeInViewport({ ratio: 1 });
		await expect(page.getByTestId('signing-confirm')).toBeInViewport({ ratio: 1 });

		record(testInfo, `${width}x${height} expected to fail`, before.where, after.where);
		expect(after.where).toEqual(before.where);
		await page.screenshot({ path: testInfo.outputPath(`fails-after-en-${width}x${height}.png`) });
	});
}

test('an unverified token is a direction and never a figure — whole, in sight, the confirm pinned where it was', async ({
	page
}, testInfo) => {
	const node = await raise(page, { width: 390, height: 844 }, 'en', {
		to: USDC,
		data: transferCalldata(RECIPIENT, 10_000_000n)
	});
	const before = await beforeTheVerdict(page, en);
	// 10 USDC leaves; and a contract of the site's own emits "5,000 of me to
	// you" and answers "USDC" when asked its name.
	node.release((me) =>
		simulatedBlock({
			status: '0x1',
			logs: [transfer(USDC, me, RECIPIENT, 10_000_000n), transfer(LURE, SITE, me, 5_000n * ETH)]
		})
	);
	const label = en('componentsUi.signing.balanceUnverifiedToken');
	await expect
		.poll(async () => (await look(page)).rows, { timeout: 30_000 })
		.toEqual(['USDC −10', `${label} +`]);
	const after = await resting(page);

	// One caution under the card, in the caution's ink — and so is the row's sign.
	const place = page.getByTestId('signing-verdict');
	expect(after.line).toBe(en('componentsUi.signing.unverifiedWarning'));
	expect(after.tone).toBe('caution');
	await expect(place.locator('.said .row .delta').nth(1)).toHaveAttribute('data-tone', 'caution');
	// The site's number is nowhere on the sheet, and its "USDC" names no second row.
	const text = (await page.getByRole('dialog').innerText()).replace(/\s+/g, ' ');
	expect(text).not.toMatch(/5[,.]?000/);
	expect(await place.locator('.said .row').filter({ hasText: 'USDC' }).count()).toBe(1);

	// Taller than the room the place keeps: it grew, drawn whole — and the
	// confirm did not move, nor did anything under the place.
	expect(after.where.placeHeight!).toBeGreaterThan(before.where.placeHeight!);
	await expect(place.locator('.said')).toBeInViewport({ ratio: 1 });
	await expect(page.getByTestId('signing-confirm')).toBeInViewport({ ratio: 1 });
	expect(after.where.confirm).toBe(before.where.confirm);
	expect(after.disabled).toBe(false);
	record(testInfo, '390x844 unverified token', before.where, after.where);
	await page.screenshot({ path: testInfo.outputPath('unverified-after-en-390x844.png') });
});

test('in Chinese: 余额变化, the figures, and 未验证代币 as a direction', async ({
	page
}, testInfo) => {
	const node = await raise(page, { width: 390, height: 844 }, 'zh', {
		to: USDC,
		data: transferCalldata(RECIPIENT, 10_000_000n)
	});
	const before = await beforeTheVerdict(page, zh);
	await page.screenshot({ path: testInfo.outputPath('moves-before-zh-390x844.png') });
	node.release((me) =>
		simulatedBlock({
			status: '0x1',
			logs: [
				transfer(USDC, me, RECIPIENT, 10_000_000n),
				transfer(NATIVE_SENTINEL, SITE, me, 3_100_000_000_000_000n),
				transfer(LURE, SITE, me, 5_000n * ETH)
			]
		})
	);
	const label = zh('componentsUi.signing.balanceUnverifiedToken');
	expect(label).toBe('未验证代币');
	await expect
		.poll(async () => (await look(page)).rows, { timeout: 30_000 })
		.toEqual(['USDC −10', 'ETH +0.0031', `${label} +`]);
	const after = await resting(page);
	const place = page.getByTestId('signing-verdict');
	await expect(place.locator('.said h3')).toHaveText(
		zh('componentsUi.signing.balanceChangesTitle')
	);
	expect(after.line).toBe(zh('componentsUi.signing.unverifiedWarning'));
	expect(after.where.confirm).toBe(before.where.confirm);
	await expect(place.locator('.said')).toBeInViewport({ ratio: 1 });
	await page.screenshot({ path: testInfo.outputPath('moves-after-zh-390x844.png') });
});
