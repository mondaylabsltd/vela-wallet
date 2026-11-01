/**
 * Where a new site starts, for an account that lives on one network (r9 S4,
 * r10 — unfixed for two rounds).
 *
 * The pass: a plain account holding only Arc Testnet USDC, the wallet in the
 * testnet space, a site it had never seen — and the connect request said
 * "Network Sepolia". The rule was right (`network_space::site_start_chain_for`,
 * twinned in the worker): the INPUT was wrong. The account's networks lived in
 * the catalog every extension page publishes, and only the wallet page knew
 * them; the request window published the catalog again the moment it opened,
 * with no networks for the account, and the space's first won.
 *
 * Here that account, on the real extension: the testnet space, every node a
 * stub that says the account holds the Arc coin and nothing anywhere else.
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
const PORT = 8848;
const STUB = 'https://stub-rpc.test/rpc';
const ARC = 5042002;
const ARC_HEX = '0x4cef52';
/** 0.4 of Arc's coin (USDC, 18 decimals as the native coin). */
const HELD = 400_000_000_000_000_000n;

interface AskResult {
	ok: boolean;
	code?: number;
	result?: unknown;
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

/**
 * Every page of this browser: the stub nodes answer, the dApp and the
 * extension are reached, and anything else fails as a dropped connection does.
 */
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

/** The fixture wallet, in the testnet space, on the stub nodes — once it has read its holdings. */
async function seedArcOnlyWallet(context: BrowserContext, id: string): Promise<Page> {
	const page = await context.newPage();
	await page.addInitScript(() => {
		localStorage.setItem('vela.intro.seen', String(Date.now()));
		localStorage.setItem('vela.dev.console', '1');
		localStorage.setItem('vela.networkSpace', 'testnet');
	});
	await page.goto(`chrome-extension://${id}/en/parallel.html`);
	await page.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
	await page.waitForURL(/wallet\.html$/, { timeout: 30_000 });
	await seedNetworkOverrides(
		page,
		CHAINS.map((c) => ({ chainId: c.chainId, rpcURL: `${STUB}/${c.chainId}` }))
	);
	await page.reload();
	// The wallet published who is signed in, read the holdings and told the
	// extension where the account lives — wherever the build keeps that.
	// Polled from here: `waitForFunction` takes the promise a storage read
	// returns for a truthy answer, and would not wait at all.
	await expect
		.poll(
			() =>
				page.evaluate(
					(arc) =>
						(window as unknown as Local).chrome.storage.local.get(null).then((all) => {
							if (all['vela.ext.cache'] === undefined) return false;
							const catalog = all['vela.ext.chains'] as { accountChains?: number[] } | undefined;
							const byAccount = (all['vela.ext.accountChains'] ?? {}) as Record<string, number[]>;
							return [catalog?.accountChains ?? [], ...Object.values(byAccount)].some((ids) =>
								ids.includes(arc)
							);
						}),
					ARC
				),
			{ timeout: 60_000 }
		)
		.toBe(true);
	return page;
}

test.describe('a new site, for an account that holds only Arc USDC', () => {
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

	test('starts on Arc Testnet — before, during and after the connect', async () => {
		const context = await loadExtension();
		await hermeticNet(context);
		const id = extensionId();
		const wallet = await seedArcOnlyWallet(context, id);
		// The page that read the holdings is gone: what the worker and the
		// request window know about the account must not depend on it.
		await wallet.close();

		const site = await context.newPage();
		await site.goto(`http://localhost:${PORT}/`);
		const chainOf = async (page: Page) =>
			((await page.evaluate(() => window.__ask('eth_chainId'))) as AskResult).result;
		expect(await chainOf(site)).toBe(ARC_HEX);

		// The connect: the request window opens (and publishes what it knows) —
		// the network it asks on is still where the account lives.
		const asked = site.evaluate(() => window.__ask('eth_requestAccounts')) as Promise<AskResult>;
		const win = await requestWindow(context);
		const facts = win.getByTestId('consent-facts');
		await expect(facts).toContainText('Network');
		await expect(facts).toContainText('Arc Testnet');
		await win.getByRole('button', { name: 'Connect' }).click();
		expect((await asked).ok).toBe(true);
		expect(await chainOf(site)).toBe(ARC_HEX);

		// Another new site, after the window published: the same start.
		const other = await context.newPage();
		await other.goto(`http://127.0.0.1:${PORT}/`);
		expect(await chainOf(other)).toBe(ARC_HEX);

		// r10 (Safari): a site may still move to a network of the space NOT in
		// front — Vela has it; "not in Vela's networks" was untrue.
		const moved = (await other.evaluate(() =>
			window.__ask('wallet_switchEthereumChain', [{ chainId: '0x2105' }])
		)) as AskResult;
		expect(moved).toMatchObject({ ok: true, result: null });
		expect(await chainOf(other)).toBe('0x2105');
		// A chain Vela does not have is 4902, as before.
		const unknown = (await other.evaluate(() =>
			window.__ask('wallet_switchEthereumChain', [{ chainId: '0x67932' }])
		)) as AskResult;
		expect(unknown).toMatchObject({ ok: false, code: 4902 });

		await context.close();
	});
});

declare global {
	interface Window {
		__ask(method: string, params?: unknown[]): Promise<AskResult>;
	}
}
