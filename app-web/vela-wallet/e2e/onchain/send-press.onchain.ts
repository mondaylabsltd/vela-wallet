/**
 * §34 (R20 S1): charged = shown, frozen at the press — on a Gnosis fork, with
 * the harness's relay bundling the real user operation.
 *
 * The R20 incident: a never-deployed account's first send (it deploys the
 * account too), Max, and a quote that moved between the press and the
 * signature. The confirm showed one amount and fee; the chain took a smaller
 * amount and a larger fee. Here the network's base fee is raised while the
 * press waits on the relay's float, so the fee card's next quote lands under
 * the press: the press must stop, sign nothing and say so beside the button
 * with the new figures; the next press must charge exactly those, and the
 * account must end at zero (a Max).
 */
import { expect, test, type Route } from '@playwright/test';
import { en } from '../live-helpers';
import {
	FIXTURES,
	FORK_RPC,
	RELAY,
	balanceOf,
	enterParallel,
	rpc,
	seedNetwork,
	setBalance,
	useHarness
} from './harness';

const SHOTS = 'e2e/onchain/.shots';
const RECIPIENT = '0x000000000000000000000000000000000000c0DE';
const FUNDED = 5n * 10n ** 17n; // 0.5 xDAI
/** The base fee the network moves to under the press: 20 gwei. */
const RAISED_BASE_FEE = 20_000_000_000n;

type RpcCall = { method?: string };

function calls(body: string | null): RpcCall[] {
	if (!body) return [];
	try {
		const parsed = JSON.parse(body) as RpcCall | RpcCall[];
		return Array.isArray(parsed) ? parsed : [parsed];
	} catch {
		return [];
	}
}

test('a deploying account’s Max: the quote moves between the press and the signature — nothing is charged, the stop is said, the next press charges what it shows', async ({
	page,
	context
}) => {
	await useHarness(context);
	// Gnosis is the fork, and only the fork: the wallet's node pool would
	// otherwise fail over from a cold fork to Gnosis's public nodes and read
	// the account's mainnet balance.
	await context.route(
		(url) => url.hostname !== '127.0.0.1' && /gnosis/i.test(url.href),
		(route) => route.abort()
	);

	// The relay, as the wallet reaches it: its float (the press's own read)
	// held on demand, every submission counted.
	let submitted = 0;
	let holdFloat = false;
	let releaseFloat: () => void = () => {};
	const floatHeld = new Promise<void>((resolve) => (releaseFloat = resolve));
	let floatAsked = 0;
	await context.route(`${RELAY}/**`, async (route: Route) => {
		const request = route.request();
		if (request.method() === 'GET' && request.url().includes('/v1/treasury/')) {
			if (holdFloat) {
				floatAsked += 1;
				await floatHeld;
			}
			return route.continue();
		}
		if (calls(request.postData()).some((c) => c.method === 'eth_sendUserOperation')) {
			submitted += 1;
		}
		return route.continue();
	});
	// The wallet's own reads of the network's price, counted; its balance reads
	// kept, to warm a cold fork with (it fetches every token contract the read
	// touches from upstream, slower than the wallet waits for an answer).
	let priceReads = 0;
	const reads = new Set<string>();
	context.on('request', (request) => {
		if (!request.url().startsWith(FORK_RPC)) return;
		const body = request.postData();
		if (calls(body).some((c) => c.method === 'eth_getBlockByNumber')) priceReads += 1;
		if (body && calls(body).some((c) => c.method === 'eth_call')) reads.add(body);
	});
	const warm = async () => {
		for (const body of reads) {
			await fetch(FORK_RPC, {
				method: 'POST',
				headers: { 'content-type': 'application/json' },
				body
			}).catch(() => undefined);
		}
	};

	// Parallel Three: the fixture account no operation has deployed on Gnosis
	// (One and Two are deployed there, and so on the fork).
	const account = FIXTURES.three;
	await enterParallel(page, [account]);
	// The R20 shape: an account no operation has deployed yet.
	expect(await rpc<string>('eth_getCode', [account, 'latest'])).toBe('0x');
	await setBalance(account, FUNDED);

	// The form, on the fork's balance: 0.5 xDAI.
	for (let attempt = 0; ; attempt++) {
		// The fork as Gnosis's node, written again once the wallet is up: the
		// parallel page's own first write of the network list can land after
		// the harness's.
		await seedNetwork(page);
		await warm();
		await page.reload();
		await page
			.getByRole('button', { name: en('componentsUi.dock.send'), exact: true })
			.first()
			.click();
		await expect(page.getByRole('heading', { name: en('send.selectTokenTitle') })).toBeVisible();
		const row = page.getByRole('button', { name: /^xDAI Gnosis/ }).last();
		const listed = await row
			.waitFor({ timeout: 30_000 })
			.then(() => true)
			.catch(() => false);
		if (!listed) {
			if (attempt >= 5) await expect(row).toBeVisible();
			continue;
		}
		await row.click();
		const funded = page.getByText(/Balance\s0\.5(?!\d)/);
		const seen = await funded
			.waitFor({ timeout: 30_000 })
			.then(() => true)
			.catch(() => false);
		if (seen) break;
		if (attempt >= 5) await expect(funded).toBeVisible();
	}
	await page.getByRole('textbox', { name: en('send.recipientLabel') }).fill(RECIPIENT);
	await page.getByRole('button', { name: en('send.maxBtn') }).click();
	await expect(page.getByRole('textbox', { name: 'xDAI' })).toHaveValue(/^0\.\d+$/, {
		timeout: 60_000
	});
	const advance = page.getByRole('button', { name: en('send.continueBtn') });
	await expect(advance).toBeEnabled({ timeout: 60_000 });
	await advance.click();
	await expect(page.getByRole('heading', { name: en('send.confirmTitle') })).toBeVisible();
	const confirm = page.getByRole('button', { name: en('send.confirmSendBtn') });
	await expect(confirm).toBeEnabled({ timeout: 60_000 });
	const feeLabel = page.getByText(en('send.estFeeLabel'), { exact: true }).first();
	const feeRow = feeLabel.locator('xpath=..');
	const pressedFee = (await feeRow.innerText()).trim();
	await page.screenshot({ path: `${SHOTS}/s34-1-confirm.png` });

	// The press. Its read of the relay's float is held; the network's base fee
	// rises meanwhile, and the fee card's next quote reads it under the press.
	holdFloat = true;
	await confirm.click();
	await expect.poll(() => floatAsked, { timeout: 30_000 }).toBeGreaterThan(0);
	await expect(page.getByText(en('send.feeSigning'))).toBeVisible();
	await rpc('anvil_setNextBlockBaseFeePerGas', [`0x${RAISED_BASE_FEE.toString(16)}`]);
	await rpc('evm_mine', []);
	const readsAtRise = priceReads;
	await expect.poll(() => priceReads, { timeout: 60_000 }).toBeGreaterThan(readsAtRise);
	// The new quote is in the core now; the row still draws the fee pressed.
	await page.waitForTimeout(3_000);
	expect((await feeRow.innerText()).trim()).toBe(pressedFee);
	await page.screenshot({ path: `${SHOTS}/s34-2-under-press.png` });

	// The signature's moment: the quote moved. Nothing signed, nothing sent,
	// nothing charged — and said beside the button, the new figures drawn.
	releaseFloat();
	holdFloat = false;
	const stop = en('send.feeMovedMax').split('{{')[0].replace(/\s+$/, '');
	await expect(page.getByText(stop, { exact: false })).toBeVisible({ timeout: 30_000 });
	await page.screenshot({ path: `${SHOTS}/s34-3-stopped.png` });
	expect(submitted).toBe(0);
	expect(await balanceOf(account)).toBe(FUNDED);
	expect(await rpc<string>('eth_getCode', [account, 'latest'])).toBe('0x');
	const movedFee = (await feeRow.innerText()).trim();
	expect(movedFee).not.toBe(pressedFee);

	// The next press charges exactly what is drawn: the whole balance, split
	// between the payee and the fee now shown — and deploys the account.
	const before = await balanceOf(RECIPIENT);
	await expect(confirm).toBeEnabled({ timeout: 30_000 });
	await confirm.click();
	await expect
		.poll(async () => rpc<string>('eth_getCode', [account, 'latest']), { timeout: 120_000 })
		.not.toBe('0x');
	expect(submitted).toBe(1);
	await expect.poll(async () => balanceOf(account), { timeout: 120_000 }).toBe(0n);
	const received = (await balanceOf(RECIPIENT)) - before;
	const charged = FUNDED - received;
	await page.screenshot({ path: `${SHOTS}/s34-4-sent.png` });
	console.log(
		`[s34] pressed "${pressedFee}" → stopped; drawn after the stop "${movedFee}"; received ${received} wei, fee leg ${charged} wei`
	);
	// The fee charged is the one drawn after the stop (the row's figure, as
	// the row rounds it), never the one first pressed.
	const figure = (text: string) => Number(/([0-9]+(?:\.[0-9]+)?)\s*xDAI/.exec(text)?.[1] ?? 'NaN');
	expect(Math.abs(Number(charged) / 1e18 - figure(movedFee))).toBeLessThan(figure(movedFee) * 0.01);
});
