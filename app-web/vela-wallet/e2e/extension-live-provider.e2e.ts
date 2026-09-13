/**
 * The provider a dApp can actually USE: reads, chain switching, and the events
 * a connected site hears (spec 027's two recorded gaps, closed).
 *
 * 027 shipped connect and sign, and answered every read and every
 * `wallet_switchEthereumChain` with "Vela cannot answer that yet" — which is
 * the request a real dApp makes before it lets anyone swap. It also pushed no
 * `accountsChanged` when the wallet switched accounts or cut a site off. Each
 * test here is one of those, on the real worker, against a node the test
 * serves itself so nothing leaves the machine.
 */
import { createServer, type Server } from 'node:http';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test, type BrowserContext, type Page } from '@playwright/test';
import {
	extensionBuilt,
	extensionId,
	loadExtension,
	noRequestWindow,
	requestWindow,
	sidePanelOpen,
	sidePanelView
} from './extension-helpers';

const APP_ROOT = join(import.meta.dirname, '..');
const DAPP_PORT = 8817;
const NODE_PORT = 8818;
const FIXTURE_ONE = '0xD400866e00B055B20752a826CD5C89b811de130b';

interface AskResult {
	ok: boolean;
	code?: number;
	message?: string;
	result?: unknown;
}
interface DAppState {
	events: { event: string; data: unknown }[];
	results: Record<string, AskResult>;
}

function serveDApp(): Promise<Server> {
	const html = readFileSync(join(APP_ROOT, 'e2e/testdapp/index.html'));
	const server = createServer((_req, res) => {
		res.setHeader('content-type', 'text/html; charset=utf-8');
		res.end(html);
	});
	return new Promise((resolve) => server.listen(DAPP_PORT, () => resolve(server)));
}

/** A JSON-RPC node that remembers what it was asked. */
function serveNode(seen: { method: string; params: unknown }[]): Promise<Server> {
	const server = createServer((req, res) => {
		let body = '';
		req.on('data', (chunk) => (body += chunk));
		req.on('end', () => {
			const call = JSON.parse(body) as { id: number; method: string; params: unknown };
			seen.push({ method: call.method, params: call.params });
			res.setHeader('content-type', 'application/json');
			if (call.method === 'eth_blockNumber') {
				res.end(JSON.stringify({ jsonrpc: '2.0', id: call.id, result: '0x64' }));
			} else if (call.method === 'eth_call') {
				// A revert IS an answer: the page asked to learn exactly this.
				res.end(
					JSON.stringify({
						jsonrpc: '2.0',
						id: call.id,
						error: { code: 3, message: 'execution reverted', data: '0x08c379a0' }
					})
				);
			} else {
				res.end(JSON.stringify({ jsonrpc: '2.0', id: call.id, result: null }));
			}
		});
	});
	return new Promise((resolve) => server.listen(NODE_PORT, () => resolve(server)));
}

async function closeServer(server: Server): Promise<void> {
	server.closeAllConnections();
	await new Promise((resolve) => server.close(() => resolve(null)));
}

type ChromeStorage = {
	chrome: {
		storage: {
			local: {
				get(k: string | string[] | null): Promise<Record<string, unknown>>;
				set(items: Record<string, unknown>): Promise<void>;
				remove(k: string | string[]): Promise<void>;
			};
		};
	};
};

/** A wallet inside the extension, with the fixture keyset. */
async function seedWallet(context: BrowserContext, id: string): Promise<Page> {
	const page = await context.newPage();
	await page.addInitScript(() => {
		localStorage.setItem('vela.intro.seen', String(Date.now()));
		localStorage.setItem('vela.dev.console', '1');
	});
	await page.goto(`chrome-extension://${id}/en/parallel.html`);
	await page.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
	await page.waitForURL(/wallet\.html$/, { timeout: 30_000 });
	// The wallet publishes both the snapshot and the network catalog; wait for
	// both rather than for a duration.
	await page.waitForFunction(
		() =>
			(window as unknown as ChromeStorage).chrome.storage.local
				.get(['vela.ext.cache', 'vela.ext.chains'])
				.then((all) => all['vela.ext.cache'] !== undefined && all['vela.ext.chains'] !== undefined),
		null,
		{ timeout: 30_000 }
	);
	return page;
}

/** Point one chain of the published catalog at the node this test serves. */
async function pointChainAt(wallet: Page, chainId: number, url: string): Promise<void> {
	await wallet.evaluate(
		async ([chainId, url]) => {
			const local = (window as unknown as ChromeStorage).chrome.storage.local;
			const all = await local.get('vela.ext.chains');
			const catalog = all['vela.ext.chains'] as { chains: Record<string, { rpc: string[] }> };
			catalog.chains[String(chainId)].rpc = [url];
			await local.set({ 'vela.ext.chains': catalog });
		},
		[chainId, url] as const
	);
}

const readState = (page: Page) =>
	page.evaluate(() => JSON.parse(document.getElementById('out')!.textContent!) as DAppState);

/** Connect through the fallback window (no gesture: a page-fired request). */
async function connect(context: BrowserContext, page: Page): Promise<void> {
	const asked = page.evaluate(() => window.__ask('eth_requestAccounts')) as Promise<AskResult>;
	const win = await requestWindow(context);
	await win.getByRole('button', { name: 'Connect' }).click();
	await noRequestWindow(context);
	expect((await asked).result).toEqual([FIXTURE_ONE]);
}

test.describe('a provider a dApp can use', () => {
	test.skip(!extensionBuilt(), 'extension/dist is missing — run `pnpm build:extension`');
	test.setTimeout(150_000);

	let dapp: Server;
	let node: Server;
	const seen: { method: string; params: unknown }[] = [];
	test.beforeAll(async () => {
		dapp = await serveDApp();
		node = await serveNode(seen);
	});
	test.afterAll(async () => {
		await closeServer(dapp);
		await closeServer(node);
	});

	test('switches chain per site, and the site hears chainChanged', async () => {
		const context = await loadExtension();
		const id = extensionId();
		await seedWallet(context, id);
		const page = await context.newPage();
		await page.goto(`http://localhost:${DAPP_PORT}/`);

		// The snapshot's default chain, before any pick.
		expect(((await page.evaluate(() => window.__ask('eth_chainId'))) as AskResult).result).toBe(
			'0x1'
		);

		// EIP-3326: a known chain answers null…
		const switched = (await page.evaluate(() =>
			window.__ask('wallet_switchEthereumChain', [{ chainId: '0x64' }])
		)) as AskResult;
		expect(switched.ok).toBe(true);
		expect(switched.result).toBeNull();

		// …the site is on it from then on, and hears about it exactly once.
		expect(((await page.evaluate(() => window.__ask('eth_chainId'))) as AskResult).result).toBe(
			'0x64'
		);
		await page.waitForFunction(() =>
			JSON.parse(document.getElementById('out')!.textContent!).events.some(
				(e: { event: string }) => e.event === 'chainChanged'
			)
		);
		const events = (await readState(page)).events.filter((e) => e.event === 'chainChanged');
		expect(events).toEqual([{ event: 'chainChanged', data: '0x64' }]);

		// A chain the wallet does not have is 4902 — "offer to add it", never a guess.
		const unknown = (await page.evaluate(() =>
			window.__ask('wallet_switchEthereumChain', [{ chainId: '0x7a69' }])
		)) as AskResult;
		expect(unknown.ok).toBe(false);
		expect(unknown.code).toBe(4902);
		expect(((await page.evaluate(() => window.__ask('eth_chainId'))) as AskResult).result).toBe(
			'0x64'
		);

		// The pick is the SITE's: a fresh load of the same origin is still on it.
		await page.reload();
		expect(((await page.evaluate(() => window.__ask('eth_chainId'))) as AskResult).result).toBe(
			'0x64'
		);
		await context.close();
	});

	test('forwards reads to the node the wallet named for the site chain, verbatim', async () => {
		const context = await loadExtension();
		const id = extensionId();
		const wallet = await seedWallet(context, id);
		await pointChainAt(wallet, 100, `http://localhost:${NODE_PORT}/`);
		const page = await context.newPage();
		await page.goto(`http://localhost:${DAPP_PORT}/`);
		await page.evaluate(() => window.__ask('wallet_switchEthereumChain', [{ chainId: '0x64' }]));

		seen.length = 0;
		const block = (await page.evaluate(() => window.__ask('eth_blockNumber'))) as AskResult;
		expect(block.ok).toBe(true);
		expect(block.result).toBe('0x64');
		expect(seen).toEqual([{ method: 'eth_blockNumber', params: [] }]);

		// The node's own error is the page's answer — a revert is what an
		// estimate exists to find out, and inventing a result would hide it.
		const call = (await page.evaluate(() =>
			window.__ask('eth_call', [
				{ to: '0x0000000000000000000000000000000000000001', data: '0x' },
				'latest'
			])
		)) as AskResult;
		expect(call.ok).toBe(false);
		expect(call.code).toBe(3);
		expect(call.message).toBe('execution reverted');

		// An allowlist: a method outside it never reaches the node.
		const refused = (await page.evaluate(() =>
			window.__ask('eth_signTransaction', [])
		)) as AskResult;
		expect(refused.code).toBe(4200);
		expect(seen.map((s) => s.method)).toEqual(['eth_blockNumber', 'eth_call']);
		await context.close();
	});

	test('a site cut off in the wallet hears accountsChanged([]) and disconnect', async () => {
		const context = await loadExtension();
		const id = extensionId();
		const wallet = await seedWallet(context, id);
		const page = await context.newPage();
		await page.goto(`http://localhost:${DAPP_PORT}/`);
		await connect(context, page);

		// The grant is the core's; revoking is its absence. The Settings screen
		// removes the key exactly like this, and the worker announces the change.
		await wallet.evaluate(async (origin) => {
			await (window as unknown as ChromeStorage).chrome.storage.local.remove('vela.perm.' + origin);
		}, `http://localhost:${DAPP_PORT}`);

		await page.waitForFunction(() =>
			JSON.parse(document.getElementById('out')!.textContent!).events.some(
				(e: { event: string }) => e.event === 'disconnect'
			)
		);
		const events = (await readState(page)).events.map((e) => e.event);
		expect(events.indexOf('accountsChanged')).toBeLessThan(events.indexOf('disconnect'));
		const cleared = (await readState(page)).events
			.filter((e) => e.event === 'accountsChanged')
			.at(-1);
		expect(cleared?.data).toEqual([]);
		expect(((await page.evaluate(() => window.__ask('eth_accounts'))) as AskResult).result).toEqual(
			[]
		);
		await context.close();
	});

	test('a site follows the account the wallet switches to', async () => {
		const context = await loadExtension();
		const id = extensionId();
		const wallet = await seedWallet(context, id);
		const page = await context.newPage();
		await page.goto(`http://localhost:${DAPP_PORT}/`);
		await connect(context, page);

		// What the wallet page does on a switch, driven directly: the core
		// re-pins the grant to the new address (`follow.ts`), and the worker
		// tells every tab of the origin. A second address is a fact the test
		// supplies — the fixture keyset has one — so the re-pin is exercised
		// without a second passkey.
		const OTHER = '0x24fb1f4e2b9c7a5d8e3f6a1b4c7d9e2f5a8b1d2e';
		await wallet.evaluate(
			async ([origin, other, first]) => {
				const local = (window as unknown as ChromeStorage).chrome.storage.local;
				const all = await local.get('vela.perm.' + origin);
				const grant = all['vela.perm.' + origin] as { address: string; chainId: number };
				// The grant is re-pinned the way `followActiveAccount` writes it.
				await local.set({
					['vela.perm.' + origin]: { ...grant, address: other, grantedAt: Date.now() }
				});
				void first;
			},
			[`http://localhost:${DAPP_PORT}`, OTHER, FIXTURE_ONE] as const
		);
		await page.waitForFunction(
			(other) =>
				JSON.parse(document.getElementById('out')!.textContent!).events.some(
					(e: { event: string; data: unknown }) =>
						e.event === 'accountsChanged' &&
						Array.isArray(e.data) &&
						e.data[0] === other.toLowerCase()
				),
			OTHER
		);
		await context.close();
	});

	test('a request a person CLICKED for opens the side panel, and it answers', async () => {
		// The product's surface, not the harness's: no window preference.
		const context = await loadExtension({ surface: 'panel' });
		const id = extensionId();
		const wallet = await seedWallet(context, id);
		const page = await context.newPage();
		await page.goto(`http://localhost:${DAPP_PORT}/`);

		// A trusted click carries the user gesture the side panel needs. The
		// panel is not a Page the harness can hold, so it is read and driven
		// through the wallet page's own view of it.
		await page.getByRole('button', { name: 'Connect' }).click();
		const panel = await sidePanelView(wallet);
		expect(panel.heading).toContain(`localhost:${DAPP_PORT}`);
		expect(context.pages().some((p) => p.url().includes('rid='))).toBe(false);
		await panel.click('Connect');

		await page.waitForFunction(
			() => JSON.parse(document.getElementById('out')!.textContent!).results.eth_requestAccounts
		);
		const state = await readState(page);
		expect(state.results.eth_requestAccounts.result).toEqual([FIXTURE_ONE]);
		// Answered, the panel goes away — a sidebar that stays is a sidebar in
		// the way.
		await expect.poll(() => sidePanelOpen(wallet), { timeout: 10_000 }).toBe(false);

		// And a request the page fires on its OWN — no click, the transient
		// activation long spent — opens the window instead. Either way, answered.
		await page.evaluate(() =>
			setTimeout(
				() =>
					window.__ask('personal_sign', [
						'0x48656c6c6f2c2056656c61',
						'0xD400866e00B055B20752a826CD5C89b811de130b'
					]),
				6000
			)
		);
		const win = await requestWindow(context, 20_000);
		expect(win.url()).toContain('rid=');
		await context.close();
	});
});

declare global {
	interface Window {
		__ask(method: string, params?: unknown[]): Promise<AskResult>;
	}
}
