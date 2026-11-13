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
import { PRESS_IN_DOM } from './stub-chain';
import { createServer, type Server } from 'node:http';
import { readFileSync } from 'node:fs';
import type { AddressInfo } from 'node:net';
import { join } from 'node:path';
import { expect, test, type BrowserContext, type Page } from '@playwright/test';
import {
	extensionBuilt,
	extensionId,
	loadExtension,
	noRequestWindow,
	inSidePanel,
	requestWindow,
	sidePanelShowsRequest,
	sidePanelUp,
	sidePanelView
} from './extension-helpers';

const APP_ROOT = join(import.meta.dirname, '..');
// Ports the system picks when the servers start (`listen(0)`): a fixed pair
// collided whenever two workers ran this describe (a retry, --repeat-each),
// and one EADDRINUSE then failed every test after it.
let DAPP_PORT = 0;
let NODE_PORT = 0;
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
	return new Promise((resolve) =>
		server.listen(0, () => {
			DAPP_PORT = (server.address() as AddressInfo).port;
			resolve(server);
		})
	);
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
	return new Promise((resolve) =>
		server.listen(0, () => {
			NODE_PORT = (server.address() as AddressInfo).port;
			resolve(server);
		})
	);
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
	// both rather than for a duration. Polled from here: a page function that
	// answers a promise is "truthy" to `waitForFunction` at once, so that wait
	// only held while the catalog happened to be published at mount — it now
	// waits for the stored networks first (`publishExtChains`).
	await expect
		.poll(
			() =>
				page.evaluate(async () => {
					const all = await (window as unknown as ChromeStorage).chrome.storage.local.get([
						'vela.ext.cache',
						'vela.ext.chains'
					]);
					return all['vela.ext.cache'] !== undefined && all['vela.ext.chains'] !== undefined;
				}),
			{ timeout: 30_000 }
		)
		.toBe(true);
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
	test.skip(!extensionBuilt(), 'extension/dist is missing — run `bun run build:extension`');
	test.setTimeout(150_000);

	let dapp: Server;
	let node: Server;
	const seen: { method: string; params: unknown }[] = [];
	test.beforeAll(async () => {
		dapp = await serveDApp();
		node = await serveNode(seen);
	});
	test.afterAll(async () => {
		// A server that never started has nothing to close.
		if (dapp) await closeServer(dapp);
		if (node) await closeServer(node);
	});

	test('switches chain per site, and the site hears chainChanged', async () => {
		const context = await loadExtension();
		const id = extensionId();
		await seedWallet(context, id);
		const page = await context.newPage();
		await page.goto(`http://localhost:${DAPP_PORT}/`);

		// Where a new site starts before any pick: the space's first network,
		// nothing having been used in it yet (dapp-17).
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

	test('a site hears its grant move to another account', async () => {
		const context = await loadExtension();
		const id = extensionId();
		const wallet = await seedWallet(context, id);
		const page = await context.newPage();
		await page.goto(`http://localhost:${DAPP_PORT}/`);
		await connect(context, page);

		// A grant moving to another address (the site connected again, on
		// another account — a switch no longer re-pins it, dapp-17), driven
		// directly: the worker tells every tab of the origin. A second address
		// is a fact the test supplies — the fixture keyset has one — so the
		// grant change is exercised without a second passkey.
		const OTHER = '0x24fb1f4e2b9c7a5d8e3f6a1b4c7d9e2f5a8b1d2e';
		await wallet.evaluate(
			async ([origin, other, first]) => {
				const local = (window as unknown as ChromeStorage).chrome.storage.local;
				const all = await local.get('vela.perm.' + origin);
				const grant = all['vela.perm.' + origin] as { address: string; chainId: number };
				// The grant moves the way a reconnect writes it.
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
		// Answered, the CARD goes and the panel stays: the panel is the wallet
		// (spec 077 FR-001), and a sidebar that vanished with the answer is the
		// complaint 077 started from.
		await expect.poll(() => sidePanelShowsRequest(wallet), { timeout: 10_000 }).toBe(false);
		expect(await sidePanelUp(wallet)).toBe(true);

		// A request the page fires on its OWN — no click, the transient
		// activation long spent — shows in the panel that is already open,
		// not in a window beside it (spec 082 RB8, row EX2).
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
		const sheet = await sidePanelView(wallet, 20_000);
		expect(sheet.heading.length).toBeGreaterThan(0);
		expect(context.pages().some((p) => p.url().includes('rid='))).toBe(false);
		await context.close();
	});

	test('a message signed in the panel ends on the Signed tick, which goes by itself (L-PANEL)', async () => {
		const context = await loadExtension({ surface: 'panel' });
		const id = extensionId();
		const wallet = await seedWallet(context, id);
		const page = await context.newPage();
		await page.goto(`http://localhost:${DAPP_PORT}/`);

		// Connected first, in the panel — the product's own surface.
		await page.getByRole('button', { name: 'Connect' }).click();
		await (await sidePanelView(wallet)).click('Connect');
		await page.waitForFunction(
			() => JSON.parse(document.getElementById('out')!.textContent!).results.eth_requestAccounts
		);
		await expect.poll(() => sidePanelShowsRequest(wallet), { timeout: 10_000 }).toBe(false);

		// A trusted click: the sign request rises in the panel.
		await page.getByRole('button', { name: 'Sign' }).click();
		await sidePanelView(wallet, 20_000);

		// Watch the panel for the landing before approving, so a tick that
		// comes and goes between two polls is still seen.
		await inSidePanel(
			wallet,
			`
			const seen = { shownAt: 0, goneAt: 0, title: '' };
			panel.__velaTick = seen;
			const look = () => {
				const receipt = panel.document.querySelector('.landing-over [data-testid="dapp-receipt"]');
				if (receipt && !seen.shownAt) {
					seen.shownAt = Date.now();
					seen.title = receipt.textContent ?? '';
				} else if (!receipt && seen.shownAt && !seen.goneAt) {
					seen.goneAt = Date.now();
				}
			};
			new panel.MutationObserver(look).observe(panel.document.body, { childList: true, subtree: true });
			`
		);
		// The confirm (issue 461: a tap), once the core's gate opens it.
		const CONFIRM = 'button[data-testid="signing-confirm"]';
		await expect
			.poll(
				() =>
					inSidePanel<boolean | null>(
						wallet,
						`return panel.document.querySelector(arg)?.disabled ?? null;`,
						CONFIRM
					),
				{ timeout: 30_000 }
			)
			.toBe(false);
		// R22: a held confirm is pressed twice from a script (the assistive path).
		await inSidePanel(wallet, PRESS_IN_DOM, CONFIRM);

		await page.waitForFunction(
			() => JSON.parse(document.getElementById('out')!.textContent!).results.personal_sign,
			null,
			{ timeout: 30_000 }
		);
		expect((await readState(page)).results.personal_sign.ok).toBe(true);
		await expect
			.poll(
				() =>
					inSidePanel<{ shownAt: number; goneAt: number; title: string }>(
						wallet,
						'return panel.__velaTick;'
					),
				{ timeout: 10_000 }
			)
			.toMatchObject({ title: expect.stringContaining('Signed') });
		const tick = await inSidePanel<{ shownAt: number; goneAt: number }>(
			wallet,
			'return panel.__velaTick;'
		);
		await expect
			.poll(() => inSidePanel<number>(wallet, 'return panel.__velaTick.goneAt;'), {
				timeout: 5_000
			})
			.toBeGreaterThan(0);
		const gone = await inSidePanel<number>(wallet, 'return panel.__velaTick.goneAt;');
		expect(gone - tick.shownAt).toBeLessThan(5_000);
		await context.close();
	});
});

declare global {
	interface Window {
		__ask(method: string, params?: unknown[]): Promise<AskResult>;
	}
}
