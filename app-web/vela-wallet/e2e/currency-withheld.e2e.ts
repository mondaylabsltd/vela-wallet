/**
 * No fiat figure before the display currency commits — and nothing moves when
 * it does (PR 3 notes 9 and 27; the core's rule in `display_currency.rs`).
 *
 * Until `CurrencyView.committed` the view is the USD/1 placeholder, which is
 * not the person's currency. A wallet with VND stored used to paint "$4,500"
 * and then jump to "₫112,500,000"; the home total has waited since the last
 * round, and the rule now has no surface it skips.
 *
 * Here the real app runs with VND stored and its rate HELD by the test, so
 * the core's view stays uncommitted (`pending: 'VND'`) for as long as the
 * test needs — inside the app's own 8 s limit on that read. Each surface is
 * opened in that state and every drawn text is recorded with its box; then
 * the rate is released and the same is recorded once the figure has landed.
 * Two things are held:
 *
 * 1. the withheld frame carries no fiat figure in any money — and the token
 *    amounts ARE there (Send stays decidable while the currency is on its way);
 * 2. every text that is in both frames is where it was: same x, same y, same
 *    height. The figure lands in room that was already kept for it.
 *
 * Hermetic: the home-truth stubs (1.5 ETH at $3,000), plus the configured
 * rates endpoint answering VND at 25,000 — the one source asked, since VND has
 * no Chainlink feed.
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
const RELAY = /vela-relay(-cf)?\.getvela\.app/;
const ONE_AND_A_HALF_ETH = 1_500_000_000_000_000_000n;
const PRICE_8DP = 3000n * 100_000_000n;
const RECIPIENT = '0x' + 'ab'.repeat(20);

/** A fiat figure in any money this run could draw: dollars or đồng, then a digit. */
const FIAT_FIGURE = /[$₫]\s?\d/;

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
		if (method === 'eth_getLogs') return [];
		if (method === 'eth_getCode') return '0x6080';
		if (method === 'eth_getTransactionCount') return '0x0';
		if (method === 'eth_gasPrice') return '0x3b9aca00';
		if (method === 'eth_maxPriorityFeePerGas') return '0x3b9aca00';
		if (method === 'eth_getBlockByNumber') return { baseFeePerGas: '0x3b9aca00' };
		if (method === 'eth_call') {
			const call = params[0] as { data?: string } | undefined;
			const n = call?.data ? aggregate3CallCount(call.data) : 0;
			const balance = chainId === 1 ? ONE_AND_A_HALF_ETH : 0n;
			const data =
				'0x' + abiWord(balance) + abiWord(PRICE_8DP) + abiWord(0) + abiWord(0) + abiWord(0);
			if (n === 0) return '0x' + abiWord(0);
			return encodeAggregate3Result(Array.from({ length: n }, () => ({ success: true, data })));
		}
		return undefined;
	});
}

/**
 * The rates endpoint, answering VND — after `release()` when held. Registered
 * after the chain stubs, so it wins over the off-origin refusal.
 */
async function rates(page: Page): Promise<{ hold: () => void; release: () => void }> {
	let gate: Promise<void> | null = null;
	let open: (() => void) | null = null;
	await page.route(/vela-currency\.getvela\.app\/v2\/rates/, async (route) => {
		if (gate) await gate;
		await route
			.fulfill({
				contentType: 'application/json',
				body: JSON.stringify([{ base: 'USD', quote: 'VND', rate: 25000 }])
			})
			.catch(() => {});
	});
	return {
		hold: () => {
			gate = new Promise<void>((resolve) => (open = resolve));
		},
		release: () => {
			open?.();
			gate = null;
		}
	};
}

/** The stored display currency, as the core's own write leaves it. */
async function storeVnd(page: Page): Promise<void> {
	await page.evaluate(
		() =>
			new Promise<void>((resolve, reject) => {
				const open = indexedDB.open('vela', 1);
				open.onupgradeneeded = () => open.result.createObjectStore('kv');
				open.onerror = () => reject(open.error);
				open.onsuccess = () => {
					const tx = open.result.transaction('kv', 'readwrite');
					tx.objectStore('kv').put('VND', 'vela.displayCurrency');
					tx.oncomplete = () => resolve();
					tx.onerror = () => reject(tx.error);
				};
			})
	);
}

type Drawn = { text: string; x: number; y: number; w: number; h: number };

/** Every element with text of its own that is drawn, and where. */
async function drawn(page: Page): Promise<Drawn[]> {
	return page.evaluate(() => {
		const out: { text: string; x: number; y: number; w: number; h: number }[] = [];
		const tenth = (n: number) => Math.round(n * 10) / 10;
		const walk = (el: Element) => {
			const style = getComputedStyle(el);
			if (style.display === 'none' || style.visibility === 'hidden') return;
			const own = [...el.childNodes]
				.filter((node) => node.nodeType === 3)
				.map((node) => (node.nodeValue ?? '').trim())
				.join(' ')
				.trim();
			const box = el.getBoundingClientRect();
			if (own !== '' && box.width > 0 && box.height > 0) {
				out.push({
					text: own,
					x: tenth(box.x),
					y: tenth(box.y),
					w: tenth(box.width),
					h: tenth(box.height)
				});
			}
			for (const child of el.children) walk(child);
		};
		walk(document.body);
		return out;
	});
}

/**
 * The texts drawn exactly once in both frames whose box moved: another x,
 * another y or another height. A figure landing changes ITS text, so it is
 * not among them — anything here is something else that shifted because of it.
 */
function moved(before: Drawn[], after: Drawn[]): string[] {
	const once = (list: Drawn[]) => {
		const count = new Map<string, number>();
		for (const item of list) count.set(item.text, (count.get(item.text) ?? 0) + 1);
		return new Map(list.filter((item) => count.get(item.text) === 1).map((d) => [d.text, d]));
	};
	const then = once(before);
	const now = once(after);
	const out: string[] = [];
	for (const [text, from] of then) {
		const to = now.get(text);
		if (to === undefined) continue;
		if (from.x !== to.x || from.y !== to.y || from.h !== to.h) {
			out.push(`${text}: (${from.x}, ${from.y}, h ${from.h}) → (${to.x}, ${to.y}, h ${to.h})`);
		}
	}
	return out;
}

/**
 * One surface, withheld and then landed.
 *
 * Reload with the rate held, `open` the surface, record it; release, wait for
 * `landed`, record it again. The app gives that rate 8 s before it commits the
 * currency without one — a frame taken later than that would be a committed
 * frame in dollars, so a slow run is tried again rather than believed.
 */
async function withheldThenLanded(
	page: Page,
	rate: { hold: () => void; release: () => void },
	open: (page: Page) => Promise<void>,
	landed: (page: Page) => Promise<void>
): Promise<{ before: Drawn[]; after: Drawn[] }> {
	for (let attempt = 0; ; attempt += 1) {
		rate.hold();
		await page.reload();
		const started = Date.now();
		await expect(page.getByText('ETH', { exact: true }).first()).toBeVisible({ timeout: 20_000 });
		await open(page);
		// Entrances and sheet slides finish before the frame is worth reading.
		await page.waitForTimeout(700);
		const before = await drawn(page);
		const inTime = Date.now() - started < 7_000;
		rate.release();
		if (!inTime && attempt < 2) continue;
		expect(inTime, 'the withheld frame was read inside the rate’s own time limit').toBe(true);
		await landed(page);
		await page.waitForTimeout(500);
		return { before, after: await drawn(page) };
	}
}

const texts = (frame: Drawn[]) => frame.map((item) => item.text).join('\n');

async function openHome(page: Page): Promise<void> {
	await page.goto('/en/wallet');
	await expect(page.getByTestId('balance-refresh')).toBeVisible();
	await seedNetworkOverrides(
		page,
		CHAINS.map((c) => ({ chainId: c.chainId, rpcURL: `${STUB}/${c.chainId}` }))
	);
	await storeVnd(page);
	await page.reload();
	// The figure, in the person's money: the starting point every case leaves.
	await expect(page.getByText(/₫112,500,000/).first()).toBeVisible({ timeout: 30_000 });
}

test('home, the token page and the account switcher: no figure until the currency commits, and nothing moves when it does', async ({
	page
}) => {
	await seedSignedIn(page);
	await stubChain(page);
	const rate = await rates(page);
	await openHome(page);
	const figure = (p: Page) =>
		expect(p.getByText(/₫[\d,]+/).first()).toBeVisible({ timeout: 30_000 });

	// --- the home: the total and a holding's worth ---------------------------
	const home = await withheldThenLanded(page, rate, async () => {}, figure);
	expect(texts(home.before)).not.toMatch(FIAT_FIGURE);
	// The currency on its way is named; the placeholder's "USD" never is.
	expect(texts(home.before)).toContain(`${en('home.totalBalance')} · VND`);
	// The token amount is not fiat: it does not wait.
	expect(home.before.map((item) => item.text)).toContain('1.5');
	expect(texts(home.after)).toMatch(/₫112,500,000/);
	expect(moved(home.before, home.after), 'home').toEqual([]);

	// --- the token page: its worth and its price -----------------------------
	const token = await withheldThenLanded(
		page,
		rate,
		async (p) => {
			await p
				.getByRole('button', { name: /^ETH Ethereum/ })
				.first()
				.click();
			await expect(p.getByText(/^1 ETH = /)).toBeVisible();
		},
		async (p) => expect(p.getByText(/^1 ETH = ₫/)).toBeVisible({ timeout: 30_000 })
	);
	expect(texts(token.before)).not.toMatch(FIAT_FIGURE);
	expect(texts(token.before)).toContain('1.5 ETH');
	expect(texts(token.after)).toContain('1 ETH = ₫75,000,000.00');
	expect(moved(token.before, token.after), 'the token page').toEqual([]);

	// --- the account switcher: each account's total, and their sum ------------
	const switcher = await withheldThenLanded(
		page,
		rate,
		async (p) => {
			await p.getByText('E2E Wallet').first().click();
			await expect(p.getByRole('dialog')).toBeVisible();
		},
		async (p) =>
			expect(
				p
					.getByRole('dialog')
					.getByText(/₫112,500,000/)
					.first()
			).toBeVisible()
	);
	expect(texts(switcher.before)).not.toMatch(FIAT_FIGURE);
	expect(moved(switcher.before, switcher.after), 'the account switcher').toEqual([]);
});

test('the send form and its review: the "≈" lines and the fee’s money wait, the coin figures do not, and nothing moves', async ({
	page
}) => {
	await stubChain(page);
	await stubRelay(page, RELAY, happyRelay('0x' + 'a1'.repeat(32), '0x' + 'b2'.repeat(32)));
	const rate = await rates(page);
	// The parallel space's fixture wallet: it can price a send.
	await page.goto('/en/parallel');
	await page.locator('li code').first().waitFor({ timeout: 60_000 });
	await page.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
	await page.waitForURL(/\/en\/wallet$/);
	await openHome(page);

	const feeRow = page.getByRole('button', { name: en('send.feeTokenLabel') });
	const compose = async (p: Page) => {
		await p
			.getByRole('button', { name: en('componentsUi.dock.send') })
			.first()
			.click();
		await expect(p.getByRole('heading', { name: en('send.selectTokenTitle') })).toBeVisible();
		await p
			.getByRole('button', { name: /^ETH Ethereum/ })
			.last()
			.click();
		await p.getByRole('textbox', { name: en('send.recipientLabel') }).fill(RECIPIENT);
		await p.getByRole('textbox', { name: 'ETH' }).fill('0.1');
		// The fee is quoted in its coin whatever the display currency is doing.
		await expect(feeRow).toContainText(/[\d.]+ ETH/, { timeout: 6_000 });
	};

	// --- the form -------------------------------------------------------------
	const form = await withheldThenLanded(page, rate, compose, async () =>
		expect(feeRow).toContainText(/₫[\d,]+/, { timeout: 30_000 })
	);
	expect(texts(form.before)).not.toMatch(FIAT_FIGURE);
	// What is sent and what it costs in the coin: there to decide with.
	expect(texts(form.before)).toMatch(/[\d.]+ ETH/);
	// "No VND rate right now" is a refusal, and this is a wait: not said.
	expect(texts(form.before)).not.toContain('VND rate');
	expect(texts(form.after)).toMatch(/≈ ₫7,500,000/);
	expect(moved(form.before, form.after), 'the send form').toEqual([]);

	// The pair has landed while the form was open: the ⇄ row enters fiat in
	// the person's money — the machine was told (`display_changed`), where it
	// used to keep the placeholder's dollars for as long as the send stayed open.
	// (It is named by the line it shows — the unit it switches to.)
	await page.getByRole('button', { name: /≈ ₫7,500,000/ }).click();
	await expect(page.getByRole('textbox', { name: 'VND' })).toBeVisible();

	// --- the review -----------------------------------------------------------
	const review = await withheldThenLanded(
		page,
		rate,
		async (p) => {
			await compose(p);
			await p.getByRole('button', { name: en('send.continueBtn') }).click();
			await expect(p.getByText(en('send.estFeeLabel'))).toBeVisible({ timeout: 6_000 });
		},
		async (p) => expect(p.getByText(/· ≈₫[\d,]+/).first()).toBeVisible({ timeout: 30_000 })
	);
	expect(texts(review.before)).not.toMatch(FIAT_FIGURE);
	expect(texts(review.before)).toMatch(/[\d.]+ ETH · ≈…/);
	expect(moved(review.before, review.after), 'the review').toEqual([]);
});
