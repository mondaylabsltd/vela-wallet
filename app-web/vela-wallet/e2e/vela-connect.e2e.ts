/**
 * Vela Connect (spec 109), end to end: a site on another origin opens the web
 * wallet's window and asks it — through the protocol's own reference client
 * (`docs/vela-connect/examples/`), served as-is, so the example a developer
 * copies is the one this suite proves.
 *
 * Everything after the site's click is the real wallet: the core's verdicts,
 * the consent card, 026's sheet, the fixture key where the passkey is (the
 * parallel space), the relay and the chain stubbed — on the CONTEXT, so the
 * window the site opens is covered too.
 */
import { createServer, type Server } from 'node:http';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test, type BrowserContext, type Page } from '@playwright/test';
import { CHAINS } from '../src/lib/services/chains';
import { en } from './live-helpers';
import {
	abiWord,
	aggregate3CallCount,
	denyOffOrigin,
	encodeAggregate3Result,
	happyRelay,
	holdPress,
	seedNetworkOverrides,
	stubChainRegistry,
	stubJsonRpc,
	stubRelay
} from './stub-chain';

test.setTimeout(240_000);

const EXAMPLES = join(import.meta.dirname, '..', '..', '..', 'docs', 'vela-connect', 'examples');
/** The site's port: a free one, so engines running side by side never share it. */
let DAPP_PORT = 0;
let DAPP = '';
const FIXTURE_ONE = '0xD400866e00B055B20752a826CD5C89b811de130b';
const STUB = 'https://stub-rpc.test/rpc';
const RELAY = /vela-relay(-cf)?\.getvela\.app/;
const ETH = 1_000_000_000_000_000_000n;
const PRICE_8DP = 3000n * 100_000_000n;
const OP_HASH = '0x' + '5e'.repeat(32);
const TX_HASH = '0x' + '7a'.repeat(32);

function serveExamples(): Promise<Server> {
	const files: Record<string, [string, string]> = {
		'/': ['index.html', 'text/html; charset=utf-8'],
		'/index.html': ['index.html', 'text/html; charset=utf-8'],
		'/vela-connect.js': ['vela-connect.js', 'text/javascript; charset=utf-8']
	};
	const server = createServer((req, res) => {
		const path = new URL(req.url ?? '/', 'http://localhost').pathname;
		const file = files[path];
		if (!file) {
			res.statusCode = 404;
			res.end('not found');
			return;
		}
		res.setHeader('content-type', file[1]);
		res.end(readFileSync(join(EXAMPLES, file[0])));
	});
	return new Promise((resolve) =>
		server.listen(0, () => {
			DAPP_PORT = (server.address() as { port: number }).port;
			DAPP = `http://localhost:${DAPP_PORT}`;
			resolve(server);
		})
	);
}

/** A deployed Safe on Ethereum holding 3 ETH at $3,000 — every window of the context sees it. */
async function stubChain(context: BrowserContext): Promise<void> {
	await denyOffOrigin(context);
	await stubChainRegistry(context, {
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
	await stubJsonRpc(context, /stub-rpc\.test\/rpc\/(\d+)/, (method, params, url) => {
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
			const balance = chainId === 1 ? ETH * 3n : 0n;
			const data =
				'0x' + abiWord(balance) + abiWord(PRICE_8DP) + abiWord(0) + abiWord(0) + abiWord(0);
			if (n === 0) return '0x' + abiWord(0);
			return encodeAggregate3Result(Array.from({ length: n }, () => ({ success: true, data })));
		}
		if (method === 'eth_getLogs') return [];
		return undefined;
	});
}

/** The relay: quotes, takes the operation, and lands it when the test says so. */
async function stubLandingRelay(context: BrowserContext) {
	let landed = false;
	const sent: unknown[] = [];
	const happy = happyRelay(OP_HASH, TX_HASH, () => (landed ? 'landed' : 'pending'));
	await stubRelay(context, RELAY, (method, params) => {
		if (method === 'eth_sendUserOperation') sent.push(params[0]);
		const answer = happy(method, params);
		// The receipt is of the operation asked about: the wallet hashes the
		// operation itself, and the batch id is that hash.
		if (method === 'eth_getUserOperationReceipt' && answer && typeof answer === 'object') {
			return { ...(answer as object), userOpHash: params[0] };
		}
		return answer;
	});
	return { sent, land: () => (landed = true) };
}

/** The fixture wallet, signed in, its networks on the stub. */
async function enterWallet(context: BrowserContext): Promise<Page> {
	await context.addInitScript(() => {
		localStorage.setItem('vela.intro.seen', String(Date.now()));
		localStorage.setItem('vela.dev.console', '1');
	});
	const wallet = await context.newPage();
	await wallet.goto('/en/parallel');
	await wallet.locator('li code').first().waitFor({ timeout: 60_000 });
	await wallet.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
	await wallet.waitForURL(/\/en\/wallet$/);
	await seedNetworkOverrides(
		wallet,
		CHAINS.map((c) => ({ chainId: c.chainId, rpcURL: `${STUB}/${c.chainId}` }))
	);
	await wallet.reload();
	await expect(wallet.getByText(/^\$9[.,]000$/)).toBeVisible({ timeout: 30_000 });
	return wallet;
}

async function openSite(context: BrowserContext, baseURL: string): Promise<Page> {
	const site = await context.newPage();
	await site.goto(`${DAPP}/?wallet=${encodeURIComponent(baseURL)}`);
	return site;
}

type Logged = { label: string; result?: unknown; code?: number; message?: string };

/** What the site's log says for `label`, once it says anything. */
async function answerFor(site: Page, label: string, after = 0): Promise<Logged> {
	let found: Logged | undefined;
	await expect
		.poll(
			async () => {
				const results = (await site.evaluate(
					() => (window as unknown as { demo: { results: Logged[] } }).demo.results
				)) as Logged[];
				found = results.filter((r) => r.label === label)[after];
				return found !== undefined;
			},
			{ timeout: 60_000 }
		)
		.toBe(true);
	return found!;
}

/** Click a site button that opens Vela; the window it opened (or the one already open). */
async function clickOpening(context: BrowserContext, site: Page, id: string): Promise<Page> {
	const opened = context.waitForEvent('page', { timeout: 15_000 });
	await site.click(`#${id}`);
	const popup = await opened;
	await popup.waitForLoadState();
	return popup;
}

test.describe('Vela Connect', () => {
	let server: Server;
	test.beforeAll(async () => {
		server = await serveExamples();
	});
	test.afterAll(async () => {
		server.closeAllConnections();
		await new Promise((resolve) => server.close(() => resolve(null)));
	});

	test('a site connects, signs, pays and follows the payment — one window, the person deciding each step', async ({
		browser,
		baseURL
	}) => {
		const context = await browser.newContext({ viewport: { width: 420, height: 800 } });
		await stubChain(context);
		const relay = await stubLandingRelay(context);
		await enterWallet(context);
		const site = await openSite(context, baseURL!);

		// Connect: the consent card names the site — the browser's fact — and
		// what it would share.
		const popup = await clickOpening(context, site, 'connect');
		await expect(
			popup.getByRole('heading', { name: `Connect to localhost:${DAPP_PORT}` })
		).toBeVisible({ timeout: 60_000 });
		await expect(popup.getByTestId('consent-facts')).toContainText('Ethereum');
		await popup.getByRole('button', { name: 'Connect', exact: true }).click();
		expect((await answerFor(site, 'connect')).result).toEqual([FIXTURE_ONE]);
		await expect(popup.getByTestId('connect-window')).toHaveAttribute('data-stage', 'idle');
		await expect(popup.getByText(`Back to localhost:${DAPP_PORT}`)).toBeVisible();

		// The same window takes the next request: no second click needed.
		await site.click('#sign');
		await expect(popup.getByText('Hello, Vela')).toBeVisible({ timeout: 60_000 });
		await expect(popup.getByText(`localhost:${DAPP_PORT}`).first()).toBeVisible();
		await holdPress(popup.getByTestId('signing-confirm'));
		const signed = await answerFor(site, 'sign');
		expect(signed.code).toBeUndefined();
		expect(String(signed.result)).toMatch(/^0x[0-9a-f]{130,}$/i);

		// A batch: one approval, answered with its id; the site follows it on
		// the same window until it lands, then closes it.
		await site.click('#send');
		const confirm = popup.getByTestId('signing-confirm');
		await confirm.waitFor({ state: 'visible', timeout: 60_000 });
		await holdPress(confirm);
		const sent = await answerFor(site, 'send');
		// EIP-5792 2.0.0: `{ id }`, the operation's hash as the wallet computed it.
		expect(sent.result).toEqual({ id: expect.stringMatching(/^0x[0-9a-f]{64}$/) });
		const batch = (sent.result as { id: string }).id;
		// Answered as the operation goes out; the relay has it a moment later.
		await expect.poll(() => relay.sent.length, { timeout: 30_000 }).toBe(1);
		relay.land();
		const status = await answerFor(site, 'status');
		expect(status.result).toMatchObject({
			id: batch,
			chainId: '0x1',
			status: 200,
			atomic: true
		});
		await expect.poll(() => popup.isClosed(), { timeout: 15_000 }).toBe(true);

		// The connection is the wallet's to list and revoke: Settings → Advanced
		// → Device storage, as in the extension.
		const settings = await context.newPage();
		await settings.goto('/en/settings');
		await settings.getByText('Advanced', { exact: true }).first().click();
		await settings.getByText('Device storage', { exact: true }).first().click();
		const row = settings.getByText(`localhost:${DAPP_PORT}`, { exact: true });
		await expect(row).toBeVisible({ timeout: 20_000 });
		await expect(settings.getByText('0xD40086…de130b')).toBeVisible();
		await settings.getByRole('button', { name: 'Disconnect', exact: true }).first().click();
		const sheet = settings.locator('[role=dialog], dialog[open]');
		await expect(sheet).toContainText(`localhost:${DAPP_PORT}`);
		await sheet.getByRole('button', { name: 'Disconnect', exact: true }).click();
		await expect(row).toHaveCount(0, { timeout: 10_000 });
		expect(
			await settings.evaluate(
				(origin) => localStorage.getItem(`vela.area.vela.perm.${origin}`),
				DAPP
			)
		).toBeNull();
		await context.close();
	});

	test('a connected site is answered without a card; reads that need nobody never ask', async ({
		browser,
		baseURL
	}) => {
		const context = await browser.newContext({ viewport: { width: 420, height: 800 } });
		await stubChain(context);
		await enterWallet(context);
		const site = await openSite(context, baseURL!);

		const popup = await clickOpening(context, site, 'connect');
		await popup.getByRole('button', { name: 'Connect', exact: true }).click();
		await answerFor(site, 'connect');

		// Asked again: answered at once, nothing shown.
		await site.click('#connect');
		expect((await answerFor(site, 'connect', 1)).result).toEqual([FIXTURE_ONE]);
		await expect(popup.getByRole('heading', { name: /Connect to/ })).toHaveCount(0);

		// Reads a site may make freely.
		const read = (method: string, params: unknown[] = []) =>
			site.evaluate(
				([m, p]) =>
					(
						window as unknown as {
							demo: { vela: { request(m: string, p: unknown[]): Promise<unknown> } };
						}
					).demo.vela
						.request(m as string, p as unknown[])
						.then((result) => ({ result }))
						.catch((e: { code: number; message: string }) => ({
							code: e.code,
							message: e.message
						})),
				[method, params] as const
			);
		expect(await read('eth_accounts')).toEqual({ result: [FIXTURE_ONE] });
		expect(await read('eth_chainId')).toEqual({ result: '0x1' });
		expect(await read('wallet_getCapabilities', [FIXTURE_ONE, ['0x1']])).toEqual({
			result: { '0x1': { atomic: { status: 'supported' } } }
		});
		// Not a node, and never a blind signature.
		expect(await read('eth_getBalance', [FIXTURE_ONE, 'latest'])).toMatchObject({ code: 4200 });
		expect(await read('eth_sign', [FIXTURE_ONE, '0x00'])).toMatchObject({ code: 4200 });
		// A chain Vela does not have.
		expect(await read('wallet_switchEthereumChain', [{ chainId: '0x7a69a' }])).toMatchObject({
			code: 4902
		});
		// A batch it never sent for this site.
		expect(await read('wallet_getCallsStatus', ['0x' + '11'.repeat(32)])).toMatchObject({
			code: 5730
		});
		// The site disconnects itself: it sees no account until it connects again.
		expect(await read('wallet_revokePermissions', [{ eth_accounts: {} }])).toEqual({
			result: null
		});
		expect(await read('eth_accounts')).toEqual({ result: [] });
		expect(await read('personal_sign', ['0x00', FIXTURE_ONE])).toMatchObject({ code: 4100 });
		await context.close();
	});

	test('the person closing the window before deciding is a decline (4001)', async ({
		browser,
		baseURL
	}) => {
		const context = await browser.newContext({ viewport: { width: 420, height: 800 } });
		await stubChain(context);
		await enterWallet(context);
		const site = await openSite(context, baseURL!);
		const popup = await clickOpening(context, site, 'connect');
		await expect(popup.getByRole('heading', { name: /Connect to/ })).toBeVisible({
			timeout: 60_000
		});
		await popup.close({ runBeforeUnload: true });
		expect(await answerFor(site, 'connect')).toMatchObject({ code: 4001 });
		await context.close();
	});

	test('Cancel on the card is a decline the site hears at once', async ({ browser, baseURL }) => {
		const context = await browser.newContext({ viewport: { width: 420, height: 800 } });
		await stubChain(context);
		await enterWallet(context);
		const site = await openSite(context, baseURL!);
		const popup = await clickOpening(context, site, 'connect');
		await popup.getByRole('button', { name: en('connect.browser.cancel') }).click();
		expect(await answerFor(site, 'connect')).toMatchObject({ code: 4001 });
		await context.close();
	});

	test('redirect mode: the warning first, then the card; the answer comes back with the state', async ({
		browser,
		baseURL
	}) => {
		const context = await browser.newContext({ viewport: { width: 420, height: 800 } });
		await stubChain(context);
		await enterWallet(context);
		const site = await openSite(context, baseURL!);
		await site.click('#redirect-connect');
		await site.waitForURL(/\/dapp/);
		// The request travelled in the fragment, and is no longer in the address.
		await expect(site.getByTestId('connect-unverified')).toBeVisible({ timeout: 60_000 });
		expect(new URL(site.url()).hash).toBe('');
		await expect(site.getByTestId('connect-unverified')).toContainText(`localhost:${DAPP_PORT}`);
		await site.getByRole('button', { name: en('connect.velaConnect.continueLabel') }).click();
		await site.getByRole('button', { name: 'Connect', exact: true }).click();
		await site.waitForURL(new RegExp(`^${DAPP}`));
		expect((await answerFor(site, 'redirect')).result).toEqual([FIXTURE_ONE]);
		// The fragment is read and cleared.
		expect(new URL(site.url()).hash).toBe('');
		await context.close();
	});

	test('with no wallet yet, the request waits; once a wallet is here it is asked', async ({
		browser,
		baseURL
	}) => {
		const context = await browser.newContext({ viewport: { width: 420, height: 800 } });
		await stubChain(context);
		await context.addInitScript(() => {
			localStorage.setItem('vela.intro.seen', String(Date.now()));
			localStorage.setItem('vela.dev.console', '1');
		});
		const site = await openSite(context, baseURL!);
		const popup = await clickOpening(context, site, 'connect');
		await expect(popup.getByTestId('connect-no-wallet')).toBeVisible({ timeout: 60_000 });
		await expect(
			popup.getByRole('button', { name: en('onboarding.welcome.createWallet') })
		).toBeVisible();

		// The person signs in elsewhere (another tab of this wallet): the window
		// follows, and the request it held is the core's to rule on now.
		const wallet = await context.newPage();
		await wallet.goto('/en/parallel');
		await wallet.locator('li code').first().waitFor({ timeout: 60_000 });
		await wallet.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
		await wallet.waitForURL(/\/en\/wallet$/);
		await expect(
			popup.getByRole('heading', { name: `Connect to localhost:${DAPP_PORT}` })
		).toBeVisible({ timeout: 60_000 });
		await popup.getByRole('button', { name: 'Connect', exact: true }).click();
		expect((await answerFor(site, 'connect')).result).toEqual([FIXTURE_ONE]);
		await context.close();
	});

	test('redirect mode, a site already connected: answered at once, no warning to read', async ({
		browser,
		baseURL
	}) => {
		const context = await browser.newContext({ viewport: { width: 420, height: 800 } });
		await stubChain(context);
		await enterWallet(context);
		const site = await openSite(context, baseURL!);
		const popup = await clickOpening(context, site, 'connect');
		await popup.getByRole('button', { name: 'Connect', exact: true }).click();
		await answerFor(site, 'connect');
		await popup.close();

		await site.click('#redirect-connect');
		await site.waitForURL(new RegExp(`^${DAPP}`), { timeout: 60_000 });
		expect((await answerFor(site, 'redirect')).result).toEqual([FIXTURE_ONE]);
		await context.close();
	});

	test('a link that is not a Vela Connect request says so and sends nothing', async ({ page }) => {
		await denyOffOrigin(page);
		await page.goto(
			'/en/dapp#request=%%%&return=https%3A%2F%2Fexample.com%2F&state=abcdefghijklmnop'
		);
		await expect(page.getByText(en('connect.velaConnect.badLinkTitle'))).toBeVisible({
			timeout: 30_000
		});
		expect(page.url()).toContain('/en/dapp');
	});

	test('opened directly, with no site behind it, the window points to the wallet', async ({
		page
	}) => {
		await denyOffOrigin(page);
		await page.goto('/dapp');
		await expect(page).toHaveURL(/\/en\/dapp$/);
		await expect(page.getByText(en('connect.velaConnect.directTitle'))).toBeVisible({
			timeout: 30_000
		});
	});
});
