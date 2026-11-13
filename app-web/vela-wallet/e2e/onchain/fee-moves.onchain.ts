/**
 * §39 (R22 follow-up, r22-3 / r22-4): charged = shown on a relay whose gas
 * price MOVES — on a Gnosis fork, a multisig's real execution bundled by the
 * harness's relay (`relay.py`, `MOVING_GAS` / `POST /v1/test/gas`).
 *
 * The relay here publishes no in-band per-gas price, so an execution's fee is
 * the Marked charge (3 × gas × max(the relay's price, the chain's)); the
 * prices set stay above the fork's ~1 gwei and under the core's bound of
 * three times it.
 *
 * - (b) the quote FALLS between the look and the press: it lands, and the
 *   fee recipient receives exactly the figure the sheet showed at the press;
 * - (c) the quote RISES under the press: nothing is signed or sent, the stop
 *   says why ("The network fee went up to ≈ … since you pressed; nothing was
 *   signed. Press again to pay …"), and the next press lands charging exactly
 *   the figure it shows;
 * - (a) the price moves up and down on EVERY quote: every press either lands
 *   at exactly the figure it showed or stops with those words — never a
 *   charge above what was shown, and never a livelock.
 */
import { readFileSync } from 'node:fs';
import { expect, test, type BrowserContext, type Page, type Route } from '@playwright/test';
import { en } from '../live-helpers';
import { RELAY, balanceOf, enterParallel, rpc, setBalance, useHarness } from './harness';

const SHOTS = 'e2e/onchain/.shots';
const RECIPIENT = '0x000000000000000000000000000000000000bEEF';
/** relay.py's FEE_RECIPIENT: where every in-band fee goes. */
const FEE_RECIPIENT = '0x5c3D966577b66a12B815119f1af452Ed7adf260c';
/** The multisig's words ship on demand (`assets/i18n/multisig/`), not in the page corpus. */
const FEE_ROSE = (
	JSON.parse(readFileSync('../../assets/i18n/multisig/en.json', 'utf8')) as {
		home: { multisig: { proposal: { feeRose: string } } };
	}
).home.multisig.proposal.feeRose;

/** Set the relay's gas cycle (gwei), from its start. */
async function gas(gwei: number[]): Promise<void> {
	const r = await fetch(`${RELAY}/v1/test/gas`, {
		method: 'POST',
		headers: { 'content-type': 'application/json' },
		body: JSON.stringify({ gwei })
	});
	expect(r.ok).toBe(true);
}

/** Every price the relay quoted, in order (wei, hex). */
async function quoted(): Promise<bigint[]> {
	const r = await fetch(`${RELAY}/v1/test/gas`);
	const body = (await r.json()) as { quoted: string[] };
	return body.quoted.map((q) => BigInt(q));
}

const gwei = (wei: bigint) => Number(wei) / 1e9;

const ENTRY_POINT = '0x0000000071727De22E5E9d8BAf0edAc6f37da032';
const USER_OP_EVENT = '0x49628fd1471006c1482da88028e9ce4dbb080b815c9b0344d39e5a8e6ec1419f';

/**
 * The gas the EntryPoint paid its beneficiary (relay.py's FEE_RECIPIENT too)
 * for the operations mined since `fromBlock` — `UserOperationEvent`'s
 * `actualGasCost` — so the in-band fee leg is read apart from it.
 */
async function entryPointCompensation(fromBlock: bigint): Promise<bigint> {
	const logs = await rpc<{ data: string }[]>('eth_getLogs', [
		{ address: ENTRY_POINT, topics: [USER_OP_EVENT], fromBlock: `0x${fromBlock.toString(16)}` }
	]);
	// data: nonce, success, actualGasCost, actualGasUsed (32 bytes each).
	return logs.reduce((sum, log) => sum + BigInt(`0x${log.data.slice(2 + 128, 2 + 192)}`), 0n);
}

/** "Network fee ≈ 0.0336 xDAI, paid by …" → its figure, in wei (exactly as written). */
function figureWei(text: string): bigint {
	const figure = /≈\s*([0-9]+(?:\.[0-9]+)?)\s*xDAI/.exec(text)?.[1];
	if (!figure) throw new Error(`no figure in "${text}"`);
	const [whole, fraction = ''] = figure.split('.');
	return BigInt(whole) * 10n ** 18n + BigInt((fraction + '0'.repeat(18)).slice(0, 18));
}

function calls(body: string | null): { method?: string }[] {
	if (!body) return [];
	try {
		const parsed = JSON.parse(body) as { method?: string } | { method?: string }[];
		return Array.isArray(parsed) ? parsed : [parsed];
	} catch {
		return [];
	}
}

async function shot(page: Page, name: string): Promise<void> {
	await page.screenshot({ path: `${SHOTS}/${name}.png`, fullPage: false });
}

/** Send → token, recipient, amount → Continue → Propose → the proposer's review. */
async function proposeSend(page: Page, amount: string): Promise<void> {
	await page
		.getByRole('button', { name: en('componentsUi.dock.send'), exact: true })
		.first()
		.click();
	await expect(page.getByRole('heading', { name: en('send.selectTokenTitle') })).toBeVisible();
	await page
		.getByRole('button', { name: /^xDAI Gnosis/ })
		.last()
		.click({ timeout: 60_000 });
	await page.getByRole('textbox', { name: en('send.recipientLabel') }).fill(RECIPIENT);
	await page.getByRole('textbox', { name: 'xDAI' }).fill(amount);
	const advance = page.getByRole('button', { name: en('send.continueBtn') });
	await expect(advance).toBeEnabled({ timeout: 30_000 });
	await advance.click();
	await expect(page.getByRole('heading', { name: en('send.confirmTitle') })).toBeVisible();
	await page.getByRole('button', { name: /^Propose( and approve)?$/ }).click();
	const hashes = page.getByTestId('review-hashes');
	await expect(hashes).toBeVisible({ timeout: 60_000 });
	await hashes
		.locator('xpath=ancestor::*[@role="dialog"][1]')
		.getByRole('button', { name: /^Propose( and approve)?$/ })
		.click();
	await expect(page.getByText('1/2 approved').first()).toBeVisible({ timeout: 60_000 });
}

/** The execute button (a held confirm when the core holds it: held, else pressed). */
async function press(page: Page): Promise<void> {
	const button = page.getByRole('button', { name: /^(Approve and execute|Execute)/ }).first();
	await expect(button).toBeEnabled({ timeout: 60_000 });
	const held = await button.evaluate(
		(el) => Number(el.closest('[data-hold-ms]')?.getAttribute('data-hold-ms') ?? '0') || 0
	);
	if (held <= 0) return button.click();
	const box = (await button.boundingBox())!;
	await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
	await page.mouse.down();
	await page.waitForTimeout(held + 300);
	await page.mouse.up();
}

/** The fee line as drawn now. */
async function feeLine(page: Page): Promise<string> {
	const line = page.getByTestId('execute-fee').locator('p').first();
	await expect(line).toBeVisible({ timeout: 60_000 });
	return (await line.innerText()).trim();
}

/**
 * The harness pointed at, with the relay's quotes watched: every operation
 * sent counted, every gas price the wallet was quoted kept (ms since the
 * start, gwei). The relay is made one that reports its network fee per gas
 * (`networkFeePerGas`, as vela-relay's quote does): relay.py's moving price
 * is then the network price the Marked charge is based on — without it the
 * core bases the charge on the chain's own price, and the relay's move moves
 * nothing.
 */
async function movingRelay(context: BrowserContext) {
	await useHarness(context);
	// Gnosis is the fork, and only the fork.
	await context.route(
		(url) => url.hostname !== '127.0.0.1' && /gnosis/i.test(url.href),
		(route) => route.abort()
	);
	const relay = {
		submitted: 0,
		t0: Date.now(),
		seen: [] as { at: number; gwei: number }[],
		/** Send's press reads the relay's float first: held here while `holdFloat`. */
		holdFloat: false,
		floatAsked: 0,
		release: () => {}
	};
	await context.route(`${RELAY}/**`, async (route: Route) => {
		if (
			relay.holdFloat &&
			route.request().method() === 'GET' &&
			route.request().url().includes('/v1/treasury/')
		) {
			relay.floatAsked += 1;
			await new Promise<void>((resolve) => (relay.release = resolve));
			return route.continue();
		}
		const asked = calls(route.request().postData());
		if (asked.some((c) => c.method === 'eth_sendUserOperation')) relay.submitted += 1;
		if (!asked.some((c) => c.method === 'pimlico_getUserOperationGasPrice'))
			return route.continue();
		const response = await route.fetch();
		const body = (await response.json()) as unknown;
		const rows = (Array.isArray(body) ? body : [body]) as {
			result?: Record<string, { maxFeePerGas: string; networkFeePerGas?: string }>;
		}[];
		for (const row of rows) {
			for (const tier of Object.values(row.result ?? {})) {
				tier.networkFeePerGas = tier.maxFeePerGas;
			}
			const standard = row.result?.standard?.maxFeePerGas;
			if (standard) relay.seen.push({ at: Date.now() - relay.t0, gwei: gwei(BigInt(standard)) });
		}
		return route.fulfill({ response, json: body });
	});
	return relay;
}

/** The Marked charge at `g` gwei for 5.6 M gas (the relay's estimate for a deployed account): 3 × gas × price. */
const atPrice = (g: number) => 3n * 5_600_000n * BigInt(Math.round(g * 1e9));

// The relay's own fixed price again for whatever runs next on this harness.
test.afterEach(async () => {
	await gas([]);
});

test('a multisig execution on a moving relay price: charged = shown; a fall lands at the shown figure; a rise stops, said why, and the next press pays its shown figure', async ({
	page,
	context
}) => {
	test.setTimeout(900_000);
	const relay = await movingRelay(context);
	const { seen, t0 } = relay;
	page.on('pageerror', (e) => console.log(`[pageerror] ${e.message.slice(0, 300)}`));

	await gas([2]);
	await enterParallel(page);

	await test.step('a 2-of-3 multisig, funded', async () => {
		await page
			.getByRole('button', { name: /Parallel One/ })
			.first()
			.click();
		await page.getByRole('button', { name: 'Add account' }).click();
		await page.getByRole('button', { name: /^Create multisig/ }).click();
		await page.getByRole('textbox', { name: /^Name/ }).fill('Moving fee');
		await page.getByText('Parallel Two', { exact: true }).click();
		await page.getByText('Parallel Three', { exact: true }).click();
		await page.getByRole('button', { name: 'Next' }).click();
		await expect(page.getByText('2 of 3')).toBeVisible();
		await page.getByRole('button', { name: 'Next' }).click();
		const group = page.getByRole('radiogroup', { name: 'First network' });
		const radio = group.getByRole('radio', { name: 'Gnosis', exact: true });
		if (!(await radio.isVisible()))
			await group.getByRole('button', { name: /^More networks/ }).click();
		await radio.click();
		await page.getByRole('button', { name: 'Create', exact: true }).click();
		await expect(page.getByText('Multisig created')).toBeVisible();
		await page.getByRole('button', { name: 'Done' }).click();
	});
	const multisig = (await page.evaluate(() => localStorage.getItem('vela.activeMultisig')))!;
	expect(multisig).toMatch(/^0x[0-9a-fA-F]{40}$/);
	await setBalance(multisig, 5n * 10n ** 18n);

	const report: string[] = [];
	const say = (line: string) => {
		report.push(line);
		console.log(line);
	};
	/**
	 * One execution: `look` is the price the sheet draws its figure at,
	 * `pressAt` the price(s) the press meets. Every press either lands at
	 * exactly the figure it showed, or stops with the core's words and sends
	 * nothing; the next press goes on. Returns the stops seen.
	 */
	async function execute(
		name: string,
		look: number[],
		pressAt: number[],
		/** Wait out the quote cache before every press, so each meets a fresh quote. */
		fresh = false,
		maxPresses = 8
	) {
		await gas(look);
		await page.reload();
		await proposeSend(page, '0.01');
		// The look, on `look`'s price — then past the wallet's 15 s quote cache,
		// so the press's own fresh look asks the relay again, and meets
		// `pressAt`'s price.
		await feeLine(page);
		await page.waitForTimeout(16_000);
		let shown = await feeLine(page);
		say(`[${name}] look: "${shown}" (quoted so far: ${seen.map((s) => s.gwei).join(', ')})`);
		await shot(page, `fee-moves-${name}-1-look`);
		await gas(pressAt);
		const stops: string[] = [];
		const pressedAt: number[] = [];
		for (let n = 1; n <= maxPresses; n++) {
			if (fresh && n > 1) await page.waitForTimeout(16_000);
			shown = await feeLine(page);
			const before = await balanceOf(FEE_RECIPIENT);
			const fromBlock = BigInt(await rpc<string>('eth_blockNumber', [])) + 1n;
			const sentBefore = relay.submitted;
			const stop = page.getByTestId('proposal-failed');
			pressedAt.push(Date.now() - t0);
			say(`[${name}] press ${n} at ${pressedAt[n - 1]} ms on "${shown}"`);
			await press(page);
			// The last stop (if any) goes with the press; then the press lands —
			// the fee recipient is paid — or stops again.
			await expect(stop)
				.toBeHidden({ timeout: 15_000 })
				.catch(() => undefined);
			let landed = false;
			await expect
				.poll(
					async () => {
						landed = (await balanceOf(FEE_RECIPIENT)) > before;
						return landed || (await stop.isVisible());
					},
					{ timeout: 180_000, intervals: [500] }
				)
				.toBe(true);
			if (!landed) {
				const said = (await stop.innerText()).trim();
				stops.push(said);
				await shot(page, `fee-moves-${name}-stop-${n}`);
				// Said why, with the new figure — never "That didn't go through".
				expect(said.startsWith(FEE_ROSE.split('{{fee}}')[0])).toBe(true);
				expect(said).toContain('nothing was signed');
				expect(said).not.toContain("didn't go through");
				// Nothing signed, nothing sent, nothing charged.
				expect(relay.submitted).toBe(sentBefore);
				expect(await balanceOf(FEE_RECIPIENT)).toBe(before);
				say(`[${name}] press ${n}: shown "${shown}" → stopped: "${said}"`);
				// The next press is on the figure the line draws now.
				await expect.poll(async () => feeLine(page), { timeout: 60_000 }).not.toBe(shown);
				continue;
			}
			await expect(page.getByText('Executed', { exact: true }).first()).toBeVisible({
				timeout: 120_000
			});
			await shot(page, `fee-moves-${name}-executed`);
			const received = (await balanceOf(FEE_RECIPIENT)) - before;
			// The EntryPoint's gas compensation to the same address (its
			// beneficiary), apart from the in-band fee leg the sheet showed.
			const compensation = await entryPointCompensation(fromBlock);
			const charged = received - compensation;
			say(
				`[${name}] press ${n}: shown "${shown}" (${figureWei(shown)} wei) → landed; fee recipient received ${received} wei (EntryPoint compensation ${compensation}), fee leg ${charged} wei`
			);
			// Charged = shown, to the wei.
			expect(charged).toBe(figureWei(shown));
			return { stops, shown, charged, presses: n, pressedAt };
		}
		throw new Error(`[${name}] no press landed in ${maxPresses}`);
	}

	// (b) the price falls between the look and the press: lands at the figure shown.
	const fell = await execute('falls', [2], [1.5]);
	expect(fell.stops).toEqual([]);
	expect(fell.charged).toBe(atPrice(2));
	// The press's own look met the lower price, and signed the figure shown.
	expect(seen.some((s) => s.at >= fell.pressedAt[0] && s.gwei === 1.5)).toBe(true);

	// (c) the price rises under the press: stopped, said why; the next press pays it.
	const rose = await execute('rises', [1.5], [2.5]);
	expect(rose.stops).toHaveLength(1);
	expect(rose.stops[0]).toBe(FEE_ROSE.replaceAll('{{fee}}', '0.042 xDAI'));
	expect(rose.presses).toBe(2);
	expect(rose.charged).toBe(atPrice(2.5));

	// (a) the price moves up and down on every quote; each press waits out the
	// quote cache, so each meets a new price: up — stopped, said why — or down
	// — signed at the figure shown. It lands, at exactly the figure it showed.
	const from = seen.length;
	const moving = await execute('moves', [2], [2.75, 1.25, 2.5, 1.5, 2.25, 1.75], true);
	const during = seen.slice(from).map((s) => s.gwei);
	const ups = during.slice(1).filter((p, i) => p > during[i]).length;
	const downs = during.slice(1).filter((p, i) => p < during[i]).length;
	say(`relay quoted (gwei): ${(await quoted()).map(gwei).join(', ')}`);
	say(`the wallet was quoted: ${seen.map((s) => `${s.at}ms:${s.gwei}`).join(', ')}`);
	expect(moving.stops.length).toBeGreaterThan(0);
	expect(ups).toBeGreaterThan(0);
	expect(downs).toBeGreaterThan(0);
	expect(moving.charged).toBe(figureWei(moving.shown));
});

/**
 * The plain Send, on the same moving price: a fall between the look and the
 * press signs the figure shown; a rise stops — "The network fee went up since
 * you pressed, so nothing was signed…" — sends nothing, and the next press
 * pays the figure then shown.
 */
test('a Send on a moving relay price: a fall lands at the figure shown; a rise stops, said why, and the next press pays its shown figure', async ({
	page,
	context
}) => {
	test.setTimeout(600_000);
	const relay = await movingRelay(context);
	page.on('pageerror', (e) => console.log(`[pageerror] ${e.message.slice(0, 300)}`));
	const FROM = '0xD400866e00B055B20752a826CD5C89b811de130b'; // Parallel One (deployed on Gnosis)
	await gas([2]);
	await enterParallel(page, [FROM]);
	await setBalance(FROM, 2n * 10n ** 18n);
	const AMOUNT = 10n ** 16n; // 0.01 xDAI
	const FEE_MOVED = en('send.feeMoved');

	const feeRow = () =>
		page.getByText(en('send.estFeeLabel'), { exact: true }).first().locator('xpath=..');
	const rowText = async () => (await feeRow().innerText()).replace(/\s+/g, ' ').trim();
	const confirm = () => page.getByRole('button', { name: en('send.confirmSendBtn') });

	async function send(name: string, look: number, pressAt: number) {
		await gas([look]);
		await page.reload();
		await page
			.getByRole('button', { name: en('componentsUi.dock.send'), exact: true })
			.first()
			.click();
		await page
			.getByRole('button', { name: /^xDAI Gnosis/ })
			.last()
			.click({ timeout: 60_000 });
		await page.getByRole('textbox', { name: en('send.recipientLabel') }).fill(RECIPIENT);
		await page.getByRole('textbox', { name: 'xDAI' }).fill('0.01');
		const advance = page.getByRole('button', { name: en('send.continueBtn') });
		await expect(advance).toBeEnabled({ timeout: 60_000 });
		await advance.click();
		await expect(confirm()).toBeEnabled({ timeout: 60_000 });
		const lookFigure = String(Number(atPrice(look)) / 1e18);
		await expect.poll(rowText, { timeout: 60_000 }).toContain(lookFigure);
		const stops: string[] = [];
		for (let n = 1; n <= 4; n++) {
			const shown = await rowText();
			const before = await balanceOf(FEE_RECIPIENT);
			const paidBefore = await balanceOf(RECIPIENT);
			const sentBefore = relay.submitted;
			const stop = page.getByText(FEE_MOVED, { exact: true });
			console.log(`[send ${name}] press ${n} at ${Date.now() - relay.t0} ms on "${shown}"`);
			await expect(confirm()).toBeEnabled({ timeout: 60_000 });
			if (n === 1) {
				// Send freezes its figures at the press; the price moves UNDER it:
				// the press's read of the relay's float is held while the fee card
				// asks again and meets `pressAt` — then the signature's moment comes.
				relay.holdFloat = true;
				const asked = relay.floatAsked;
				await confirm().click();
				await expect.poll(() => relay.floatAsked, { timeout: 30_000 }).toBeGreaterThan(asked);
				const quotes = relay.seen.length;
				await gas([pressAt]);
				await expect
					.poll(() => relay.seen.slice(quotes).some((s) => s.gwei === pressAt), {
						timeout: 120_000
					})
					.toBe(true);
				await page.waitForTimeout(2_000);
				// The row still draws the figure pressed.
				expect(await rowText()).toBe(shown);
				relay.holdFloat = false;
				relay.release();
			} else {
				await confirm().click();
			}
			let landed = false;
			await expect
				.poll(
					async () => {
						landed = (await balanceOf(FEE_RECIPIENT)) > before;
						return landed || (await stop.isVisible());
					},
					{ timeout: 180_000, intervals: [500] }
				)
				.toBe(true);
			if (!landed) {
				stops.push(FEE_MOVED);
				await shot(page, `fee-moves-send-${name}-stop-${n}`);
				// Nothing signed, nothing sent, nothing charged.
				expect(relay.submitted).toBe(sentBefore);
				expect(await balanceOf(FEE_RECIPIENT)).toBe(before);
				console.log(`[send ${name}] press ${n}: stopped: "${FEE_MOVED}"`);
				await expect.poll(rowText, { timeout: 60_000 }).not.toBe(shown);
				continue;
			}
			await expect.poll(async () => (await balanceOf(RECIPIENT)) - paidBefore).toBe(AMOUNT);
			const charged = (await balanceOf(FEE_RECIPIENT)) - before;
			await shot(page, `fee-moves-send-${name}-sent`);
			console.log(
				`[send ${name}] press ${n}: shown "${shown}" → landed; fee recipient received ${charged} wei`
			);
			return { stops, shown, charged };
		}
		throw new Error(`[send ${name}] no press landed`);
	}

	// A fall between the look and the press: the figure shown is signed.
	const fell = await send('falls', 2, 1.5);
	expect(fell.stops).toEqual([]);
	expect(fell.shown).toContain('0.0336');
	expect(fell.charged).toBe(atPrice(2));
	// A rise under the press: stopped, said why; the next press pays the new figure.
	const rose = await send('rises', 1.5, 2.5);
	expect(rose.stops).toHaveLength(1);
	expect(rose.shown).toContain('0.042');
	expect(rose.charged).toBe(atPrice(2.5));
	console.log(
		`[send] the wallet was quoted: ${relay.seen.map((s) => `${s.at}ms:${s.gwei}`).join(', ')}`
	);
});
