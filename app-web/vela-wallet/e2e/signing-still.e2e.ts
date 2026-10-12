/**
 * The signing sheet does not jump (device walk of 2026-10-11, item 2).
 *
 * On Android the sheet stepped upward twice as its fee landed — 77 px each,
 * the ✕ moving under a finger from y 337 to 183. The confirm stayed put: it is
 * pinned in the sheet's foot. Everything above it moved, because the sheet is
 * anchored to the bottom of the screen and grew by every line the fee card
 * gained after it opened: the speed's note, "No token can pay this fee", why a
 * fee could not be worked out.
 *
 * The rule: from its first settled frame the sheet has its final geometry for
 * everything the fee card can say. Its top, its header and its ✕ never move.
 *
 * This walks the real sheet on the real machines — the wallet page, the dev
 * requester, a stubbed node and relay — and reads EVERY painted frame from the
 * moment the sheet has come to rest with its fee still being worked out until
 * the fee has said its last word. Four endings (priced; priced, and no token
 * can pay it; the relay answers with an error; the relay cannot be reached),
 * on the phone sheet at two sizes, at the side panel's width, and as the wide
 * layout's centred card. The relay is held shut until the sheet is at rest, so
 * "the first settled frame" is a frame this test saw — with the request read
 * and its simulation's verdict already on the sheet, which leaves the fee's
 * landing as the only thing that happens while it watches.
 */
import { writeFileSync } from 'node:fs';
import { expect, test, type Page } from '@playwright/test';
import { CHAINS } from '../src/lib/services/chains';
import { en } from './live-helpers';
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

test.setTimeout(150_000);

const STUB = 'https://stub-rpc.test/rpc';
const RELAY = /vela-relay(-cf)?\.getvela\.app/;
const ETH = 1_000_000_000_000_000_000n;
const PRICE_8DP = 3000n * 100_000_000n;
const RECIPIENT = '0x' + 'ab'.repeat(20);

const LAYOUTS = [
	['the phone sheet, 390 × 844', { width: 390, height: 844 }],
	['the phone sheet, 360 × 640', { width: 360, height: 640 }],
	['the side panel’s width, 320 × 600', { width: 320, height: 600 }],
	['the wide layout’s centred card, 1440 × 900', { width: 1440, height: 900 }]
] as const;

type Ending = 'priced' | 'no token can pay' | 'refused' | 'unreachable';
const ENDINGS: Ending[] = ['priced', 'no token can pay', 'refused', 'unreachable'];

/** One painted frame of the sheet: where its parts are, and what its fee says. */
interface Frame {
	t: number;
	sheetTop: number;
	header: number;
	close: number;
	blocks: number;
	fee: number;
	feeHeight: number;
	speed: number;
	signer: number;
	confirm: number;
	/** How far the sheet's body is scrolled. */
	scrolled: number;
	said: string;
}
type Part = Exclude<keyof Frame, 't' | 'said'>;

/** Deployed Safe, 1.5 ETH at $3,000, a calm gas market — and a node that simulates. */
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
		if (method === 'eth_estimateGas') return '0x5208';
		if (method === 'eth_getLogs') return [];
		// The sheet's own simulation answers at once: its card is on the sheet
		// before the fee is, so the fee's landing is all that is left to watch.
		if (method === 'eth_simulateV1') return [{ calls: [{ status: '0x1', logs: [] }] }];
		if (method === 'eth_call') {
			const call = params[0] as { data?: string } | undefined;
			const n = call?.data ? aggregate3CallCount(call.data) : 0;
			const balance = chainId === 1 ? (ETH * 3n) / 2n : 0n;
			const data =
				'0x' + abiWord(balance) + abiWord(PRICE_8DP) + abiWord(0) + abiWord(0) + abiWord(0);
			if (n === 0) return '0x' + abiWord(0);
			return encodeAggregate3Result(Array.from({ length: n }, () => ({ success: true, data })));
		}
		return undefined;
	});
}

/** The relay, as each ending has it — and held shut until `release`. */
async function stubHeldRelay(page: Page, ending: Ending): Promise<() => void> {
	const happy = happyRelay('0x' + 'a1'.repeat(32), '0x' + 'b2'.repeat(32), () => 'pending');
	await stubRelay(page, RELAY, (method, params) => {
		if (ending === 'refused') return { error: { code: -32603, message: 'internal error' } };
		if (ending === 'no token can pay' && method === 'vela_getInBandGasQuote') {
			// The relay prices in the native coin and reports the Safe's own
			// balance of it: nothing.
			return [
				{
					recipient: '0x' + 'fe'.repeat(20),
					asset: 'native',
					feeToken: null,
					balance: '0x0',
					decimals: 18,
					symbol: 'ETH',
					usdBalance: '0',
					usdPrice: '3000'
				}
			];
		}
		return happy(method, params);
	});
	// Registered last, so it is asked first: every JSON-RPC call to the relay
	// waits here until the sheet is at rest, then goes on — or, for a relay
	// that cannot be reached, fails the way a dead connection does.
	let release: () => void = () => {};
	const held = new Promise<void>((resolve) => (release = resolve));
	await page.route(RELAY, async (route) => {
		if (route.request().method() !== 'POST') return route.fallback();
		await held;
		if (ending === 'unreachable') return route.abort('internetdisconnected');
		return route.fallback();
	});
	return release;
}

/** Into the parallel space, on the stubbed chain, with the balance showing. */
async function enterWallet(page: Page): Promise<void> {
	await page.addInitScript(() => localStorage.setItem('vela.dev.console', '1'));
	await page.goto('/en/parallel');
	// The fixture list fills on mount — a row in it means the handlers are on.
	await page.locator('li code').first().waitFor({ timeout: 60_000 });
	await page.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
	await page.waitForURL(/\/en\/wallet$/);
	await seedNetworkOverrides(
		page,
		CHAINS.map((c) => ({ chainId: c.chainId, rpcURL: `${STUB}/${c.chainId}` }))
	);
	await page.reload();
	await expect(page.getByText('$4,500', { exact: true }).first()).toBeVisible({ timeout: 40_000 });
	await page.waitForFunction(
		() => (window as unknown as { vela?: { requester?: unknown } }).vela?.requester !== undefined,
		null,
		{ timeout: 20_000 }
	);
}

/** A page asks for a plain transfer, from the account on screen. */
async function ask(page: Page): Promise<void> {
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
}

/**
 * A frame sampler, in the page: every painted frame of the sheet, from the
 * moment it exists.
 */
async function sample(page: Page): Promise<void> {
	await page.evaluate(() => {
		const still = { frames: [] as unknown[] };
		(window as unknown as { __still: typeof still }).__still = still;
		const SHEET = '[role="dialog"]';
		const box = (selector: string) =>
			document.querySelector(`${SHEET} ${selector}`)?.getBoundingClientRect();
		const loop = () => {
			const sheet = document.querySelector(SHEET);
			if (sheet !== null) {
				const fee = box('.footer > .line');
				still.frames.push({
					t: performance.now(),
					sheetTop: sheet.getBoundingClientRect().top,
					header: box('[data-signing-top]')?.top ?? NaN,
					close: box('header.header .close')?.top ?? NaN,
					blocks: box('.blocks')?.top ?? NaN,
					fee: fee?.top ?? NaN,
					feeHeight: fee?.height ?? NaN,
					speed: box('.footer > .speed')?.top ?? NaN,
					signer: box('.footer > .signer')?.top ?? NaN,
					confirm: box('[data-testid="signing-confirm"]')?.top ?? NaN,
					scrolled: sheet.querySelector('.content')?.scrollTop ?? 0,
					// The fee row, the line said under it, and the speed's note —
					// what is drawn, not the rooms held for what is not.
					said: [...sheet.querySelectorAll('.footer > .line, .footer .warning, .footer .free')]
						.filter((el) => getComputedStyle(el).visibility !== 'hidden')
						.map((el) => el.textContent?.replace(/\s+/g, ' ').trim())
						.join(' | ')
				});
			}
			requestAnimationFrame(loop);
		};
		requestAnimationFrame(loop);
	});
}

const framesSince = (page: Page, mark: number): Promise<Frame[]> =>
	page.evaluate(
		(since) =>
			(window as unknown as { __still: { frames: Frame[] } }).__still.frames.filter(
				(frame) => frame.t >= since
			),
		mark
	);

/**
 * Wait until the sheet's parts have been in one place over a run of painted
 * frames; answers the time of the first frame of that run.
 */
async function atRest(page: Page): Promise<number> {
	return page.evaluate(
		() =>
			new Promise<number>((resolve) => {
				const still = (window as unknown as { __still: { frames: Frame[] } }).__still;
				const RUN = 30;
				const look = () => {
					const last = still.frames.slice(-RUN);
					const same =
						last.length === RUN &&
						last.every(
							(frame) =>
								Number.isFinite(frame.confirm) &&
								frame.sheetTop === last[0].sheetTop &&
								frame.close === last[0].close &&
								frame.fee === last[0].fee &&
								frame.confirm === last[0].confirm &&
								frame.said === last[0].said
						);
					if (same) resolve(last[0].t);
					else requestAnimationFrame(look);
				};
				look();
			})
	);
}

/** What the fee has said once it has said its last word. */
const LAST_WORD: Record<Ending, RegExp> = {
	priced: /\d ETH/,
	'no token can pay': new RegExp(en('componentsUi.gas.noCoinPays')),
	refused: /Network fee (—|Tap to retry)/,
	unreachable: /Network fee (—|Tap to retry)/
};

const round = (n: number) => Math.round(n * 100) / 100;
const spread = (frames: Frame[], part: Part): number => {
	const values = frames.map((frame) => frame[part]).filter(Number.isFinite);
	return values.length === 0 ? 0 : round(Math.max(...values) - Math.min(...values));
};

for (const [layout, viewport] of LAYOUTS) {
	test.describe(layout, () => {
		test.use({ viewport });

		for (const ending of ENDINGS) {
			test(`the fee lands — ${ending} — and the sheet’s top, header, ✕ and confirm stay where they were`, async ({
				page
			}, testInfo) => {
				await stubChain(page);
				const release = await stubHeldRelay(page, ending);
				await enterWallet(page);
				await ask(page);

				const dialog = page.locator('[role="dialog"]');
				const confirm = page.getByTestId('signing-confirm');
				await expect(confirm).toBeVisible({ timeout: 25_000 });
				await sample(page);

				// The first settled frame: the request is read, its simulation
				// has said its piece, and the fee is still being worked out.
				await expect(dialog.locator('.blocks .sentence')).toHaveCount(0, { timeout: 30_000 });
				await expect(dialog.getByTestId('signing-verdict')).toHaveAttribute(
					'data-stage',
					'landed',
					{
						timeout: 30_000
					}
				);
				const feeLine = dialog.locator('.footer > .line');
				await expect(feeLine).toContainText(en('componentsUi.gas.estimating'));
				const settled = await atRest(page);
				const first = (await framesSince(page, settled)).at(0)!;
				expect(first.said).toContain(en('componentsUi.gas.estimating'));
				await page.screenshot({ path: testInfo.outputPath('first.png') });

				// The relay answers, and the fee says its last word.
				release();
				await expect
					.poll(async () => (await framesSince(page, settled)).at(-1)?.said ?? '', {
						timeout: 60_000
					})
					.toMatch(LAST_WORD[ending]);
				await atRest(page);
				// A beat more, for anything that lands a moment after the figure
				// (a failed fee is asked again by itself after 3 s).
				await page.waitForTimeout(4_000);
				await atRest(page);

				const frames = await framesSince(page, settled);
				const last = frames.at(-1)!;
				const PARTS: Part[] = [
					'sheetTop',
					'header',
					'close',
					'blocks',
					'fee',
					'feeHeight',
					'speed',
					'signer',
					'confirm',
					'scrolled'
				];
				const moved = Object.fromEntries(PARTS.map((part) => [part, spread(frames, part)]));
				const places = (frame: Frame) =>
					Object.fromEntries(PARTS.map((part) => [part, round(frame[part])]));
				const report = {
					layout,
					ending,
					frames: frames.length,
					first: { ...places(first), said: first.said },
					last: { ...places(last), said: last.said },
					// Every different thing the fee said on the way, in order.
					saidOnTheWay: [...new Set(frames.map((frame) => frame.said))],
					moved
				};
				writeFileSync(testInfo.outputPath('still.json'), JSON.stringify(report, null, 1));
				await page.screenshot({ path: testInfo.outputPath('last.png') });
				console.log(`STILL ${JSON.stringify(report)}`);

				// Not a pixel, from the first settled frame to the last: the sheet's
				// top, its header, its ✕ — and its confirm.
				expect(
					{
						sheetTop: moved.sheetTop,
						header: moved.header,
						close: moved.close,
						confirm: moved.confirm
					},
					`${layout} · ${ending}`
				).toEqual({ sheetTop: 0, header: 0, close: 0, confirm: 0 });
				// Nor anything of the body: what the request says, the fee row, the
				// speed control and the signing account stand where they stood, and
				// the body was not scrolled under a reader.
				expect(
					{
						blocks: moved.blocks,
						fee: moved.fee,
						speed: moved.speed,
						signer: moved.signer,
						scrolled: moved.scrolled
					},
					`${layout} · ${ending}`
				).toEqual({ blocks: 0, fee: 0, speed: 0, signer: 0, scrolled: 0 });
				// And the ✕ and the confirm are whole, on the screen.
				await expect(dialog.locator('header.header .close')).toBeInViewport({ ratio: 1 });
				await expect(confirm).toBeInViewport({ ratio: 1 });
			});
		}
	});
}
