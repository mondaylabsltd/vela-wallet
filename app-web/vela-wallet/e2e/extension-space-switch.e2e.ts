/**
 * A network-space switch and the sites already connected (r10 follow-up).
 *
 * Before: a site kept its network when the person switched spaces, so a
 * session in the mainnet space could keep a site on a testnet. The owner's
 * spec-105 line — test value is never mistaken for real — decides it: on a
 * space switch every site whose network is outside the new space moves to
 * that space's start network (`network_space::sites_moved_by_space_switch`,
 * twinned in the worker as `spaceSwitchMoves`, one set of vectors). The page
 * hears `chainChanged`, and the site's next request says so in one line.
 *
 * Here on the real extension: the fixture wallet in the testnet space, a site
 * connected on Arc Testnet, then the person switches to the mainnet space.
 */
import { createServer, type Server } from 'node:http';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test, type BrowserContext, type Page, type Route } from '@playwright/test';
import { CHAINS } from '../src/lib/services/chains';
import { extensionBuilt, extensionId, loadExtension, requestWindow } from './extension-helpers';
import { seedNetworkOverrides } from './stub-chain';
import { answerAggregate3, isAggregate3, MULTICALL_SEL as SEL, word } from './stub-multicall';

const APP_ROOT = join(import.meta.dirname, '..');
const PORT = 8849;
const STUB = 'https://stub-rpc.test/rpc';
const ARC = 5042002;
const ARC_HEX = '0x4cef52';
/** 0.4 of Arc's coin. */
const HELD = 400_000_000_000_000_000n;

interface AskResult {
	ok: boolean;
	code?: number;
	message?: string;
	result?: unknown;
}

interface DAppState {
	events: { event: string; data: unknown }[];
}

function serveDApp(): Promise<Server> {
	const html = readFileSync(join(APP_ROOT, 'e2e/testdapp/index.html'));
	const server = createServer((_req, res) => {
		res.setHeader('content-type', 'text/html; charset=utf-8');
		res.end(html);
	});
	return new Promise((resolve) => server.listen(PORT, () => resolve(server)));
}

/** A node on every chain: the account holds Arc's coin, and nothing else anywhere. */
function answer(chainId: number, method: string, params: unknown[]): unknown {
	switch (method) {
		case 'eth_chainId':
			return '0x' + chainId.toString(16);
		case 'eth_blockNumber':
			return '0x10';
		case 'eth_getCode':
			return '0x';
		case 'eth_getTransactionCount':
			return '0x0';
		case 'eth_gasPrice':
		case 'eth_maxPriorityFeePerGas':
			return '0x3b9aca00';
		case 'eth_getBlockByNumber':
			return { number: '0x10', timestamp: '0x66000000', baseFeePerGas: '0x3b9aca00' };
		case 'eth_getBalance':
			return chainId === ARC ? '0x' + HELD.toString(16) : '0x0';
		case 'eth_getLogs':
			return [];
		case 'eth_call': {
			const call = params[0] as { data?: string } | undefined;
			if (!call?.data || !isAggregate3(call.data)) return '0x' + word(0);
			return answerAggregate3(call.data, (inner) => {
				if (inner.selector === SEL.getEthBalance)
					return { success: true, data: '0x' + word(chainId === ARC ? HELD : 0n) };
				if (inner.selector === SEL.balanceOf) return { success: true, data: '0x' + word(0) };
				return undefined;
			});
		}
		default:
			return undefined;
	}
}

async function hermeticNet(context: BrowserContext): Promise<void> {
	await context.route(/^https?:\/\/(?!localhost|127\.0\.0\.1)/, (route: Route) =>
		route.abort('failed')
	);
	await context.route(/stub-rpc\.test\/rpc\/(\d+)/, async (route: Route) => {
		const request = route.request();
		if (request.method() !== 'POST') return route.abort('failed');
		const chainId = Number(/\/rpc\/(\d+)/.exec(request.url())?.[1]);
		const body = JSON.parse(request.postData() ?? '{}') as {
			id?: number;
			method?: string;
			params?: unknown[];
		};
		const result = answer(chainId, body.method ?? '', body.params ?? []);
		return route.fulfill({
			contentType: 'application/json',
			body: JSON.stringify(
				result === undefined
					? { jsonrpc: '2.0', id: body.id ?? 1, error: { code: -32601, message: 'no' } }
					: { jsonrpc: '2.0', id: body.id ?? 1, result }
			)
		});
	});
}

type Local = { chrome: { storage: { local: { get(k: null): Promise<Record<string, unknown>> } } } };

/** The catalog's space, as the extension holds it. */
const publishedSpace = (page: Page) =>
	page.evaluate(() =>
		(window as unknown as Local).chrome.storage.local
			.get(null)
			.then((all) => (all['vela.ext.chains'] as { space?: string } | undefined)?.space)
	);

/** The fixture wallet in `space`, on the stub nodes. */
async function openWallet(context: BrowserContext, id: string, space: string): Promise<Page> {
	const page = await context.newPage();
	await page.addInitScript((s) => {
		localStorage.setItem('vela.intro.seen', String(Date.now()));
		localStorage.setItem('vela.dev.console', '1');
		localStorage.setItem('vela.networkSpace', s);
	}, space);
	await page.goto(`chrome-extension://${id}/en/parallel.html`);
	await page.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
	await page.waitForURL(/wallet\.html$/, { timeout: 30_000 });
	await seedNetworkOverrides(
		page,
		CHAINS.map((c) => ({ chainId: c.chainId, rpcURL: `${STUB}/${c.chainId}` }))
	);
	await page.reload();
	await expect.poll(() => publishedSpace(page), { timeout: 60_000 }).toBe(space);
	return page;
}

const readState = (page: Page) =>
	page.evaluate(() => JSON.parse(document.getElementById('out')!.textContent!) as DAppState);

test.describe('a network-space switch and the connected sites', () => {
	test.skip(!extensionBuilt(), 'extension/dist is missing — run `pnpm build:extension`');
	test.setTimeout(180_000);

	let server: Server;
	test.beforeAll(async () => {
		server = await serveDApp();
	});
	test.afterAll(async () => {
		server.closeAllConnections();
		await new Promise((resolve) => server.close(() => resolve(null)));
	});

	test('a site on a testnet moves to the mainnet space’s network, hears it, and its next request says so', async () => {
		const context = await loadExtension();
		await hermeticNet(context);
		const id = extensionId();
		const wallet = await openWallet(context, id, 'testnet');

		// A site connected on Arc Testnet, in the testnet space.
		const site = await context.newPage();
		await site.goto(`http://localhost:${PORT}/`);
		const chainOf = async () =>
			((await site.evaluate(() => window.__ask('eth_chainId'))) as AskResult).result as string;
		const connect = site.evaluate(() => window.__ask('eth_requestAccounts')) as Promise<AskResult>;
		const consent = await requestWindow(context);
		await consent.getByRole('button', { name: 'Connect' }).click();
		expect((await connect).ok).toBe(true);
		const switched = (await site.evaluate(
			(arc) => window.__ask('wallet_switchEthereumChain', [{ chainId: arc }]),
			ARC_HEX
		)) as AskResult;
		expect(switched.ok).toBe(true);
		expect(await chainOf()).toBe(ARC_HEX);

		// The person switches to the mainnet space in Vela: the wallet, opened
		// in it, publishes the mainnet space's catalog.
		await wallet.close();
		const mainnet = await context.newPage();
		await mainnet.addInitScript(() => localStorage.setItem('vela.networkSpace', 'mainnet'));
		await mainnet.goto(`chrome-extension://${id}/en/wallet.html`);
		await expect.poll(() => publishedSpace(mainnet), { timeout: 60_000 }).toBe('mainnet');
		await mainnet.close();

		// The site is on a network of the mainnet space now — never Arc — and
		// its page was told.
		await expect.poll(chainOf, { timeout: 15_000 }).not.toBe(ARC_HEX);
		const now = await chainOf();
		const network = CHAINS.find((c) => c.chainId === parseInt(now, 16));
		expect(network, `a network Vela has: ${now}`).toBeDefined();
		expect(network?.testnet === true).toBe(false);
		await expect
			.poll(async () =>
				(await readState(site)).events.some((e) => e.event === 'chainChanged' && e.data === now)
			)
			.toBe(true);

		// A page asking for Arc again is told it is the testnet space's network
		// — the same sentence the in-app browsers say (the shared vectors).
		const back = (await site.evaluate(
			(arc) => window.__ask('wallet_switchEthereumChain', [{ chainId: arc }]),
			ARC_HEX
		)) as AskResult;
		expect(back).toMatchObject({
			ok: false,
			code: 4902,
			message:
				"Chain 5042002 is one of Vela's testnet networks; Vela is showing its mainnet networks. Switch to the testnet space in Vela to use it."
		});

		// Its next request that opens the sheet says, once, that the network
		// changed with the space.
		const name = CHAINS.find((c) => c.chainId === parseInt(now, 16))?.name ?? '';
		const signed = site.evaluate(() =>
			window.__ask('personal_sign', [
				'0x48656c6c6f',
				(window as unknown as { ethereum: { selectedAddress: string } }).ethereum.selectedAddress
			])
		) as Promise<AskResult>;
		const sheet = await requestWindow(context);
		const line = sheet.getByTestId('signing-space-note');
		await expect(line).toContainText('The network changed with the space', { timeout: 30_000 });
		await expect(line).toContainText(name);
		await sheet.close();
		expect((await signed).ok).toBe(false);

		await context.close();
	});
});

declare global {
	interface Window {
		__ask(method: string, params?: unknown[]): Promise<AskResult>;
	}
}
