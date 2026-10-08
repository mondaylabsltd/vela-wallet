/**
 * A request's life in the packaged extension (spec 082 T100, RH5) — the
 * device rows EX4, EX4b, EX5, EX6 and EX8, on the real worker and the real
 * side panel; and since round 2 (T216) the two the post-fix pass found
 * untested: a panel that goes after the submit claim (G35, RJ2) and a request
 * that arrives while the panel shows Settings (G55, RB9).
 *
 * Each of these failed on 079's build: a reload left the old sheet up (G17),
 * a second tab's request never showed (G18), a stopped worker lost a live
 * request and the page read Chrome's "message channel closed" (G19), the
 * panel's ✕ answered nothing (EX6), and the panel stopped being the panel
 * after a visit to Settings (G23c). Every case asserts the same three things:
 * one answer per request, 4900 with plain words when nobody decided, and no
 * `vela.req.*` left behind.
 *
 * Its own ports (8827/8828, and 8829 for the stand-in chain and relay), so it
 * never shares a server with the other extension suites (8817/8818) or
 * another session's run.
 */
import { createServer, type Server } from 'node:http';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test, type BrowserContext, type Page } from '@playwright/test';
import { CHAINS } from '../src/lib/services/chains';
import { NOT_CONFIRMED_MESSAGE } from '../extension/lib/protocol.js';
import {
	extensionBuilt,
	extensionId,
	inSidePanel,
	loadExtension,
	noRequestWindow,
	requestWindow,
	sidePanelShowsRequest,
	sidePanelUp,
	sidePanelView,
	slideToConfirm
} from './extension-helpers';
import { abiWord, aggregate3CallCount, encodeAggregate3Result, happyRelay } from './stub-chain';

const APP_ROOT = join(import.meta.dirname, '..');
const DAPP_PORT = 8827;
const DAPP_B_PORT = 8828;
const NET_PORT = 8829;
const FIXTURE_ONE = '0xD400866e00B055B20752a826CD5C89b811de130b';

interface AskResult {
	ok: boolean;
	code?: number;
	message?: string;
	result?: unknown;
}

function serveDApp(port: number): Promise<Server> {
	const html = readFileSync(join(APP_ROOT, 'e2e/testdapp/index.html'));
	const server = createServer((_req, res) => {
		res.setHeader('content-type', 'text/html; charset=utf-8');
		res.end(html);
	});
	return new Promise((resolve) => server.listen(port, () => resolve(server)));
}

async function closeServer(server: Server): Promise<void> {
	server.closeAllConnections();
	await new Promise((resolve) => server.close(() => resolve(null)));
}

type ChromeStorage = {
	chrome: {
		storage: {
			local: { get(k: string | string[] | null): Promise<Record<string, unknown>> };
			session: { get(k: string | string[] | null): Promise<Record<string, unknown>> };
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

/** The worker's ledger: every `vela.req.*` key, in session AND local storage. */
const ledger = (wallet: Page) =>
	wallet.evaluate(async () => {
		const { storage } = (window as unknown as ChromeStorage).chrome;
		const [session, local] = await Promise.all([
			storage.session.get(null),
			storage.local.get(null)
		]);
		const keys = (all: Record<string, unknown>) =>
			Object.keys(all).filter((k) => k.startsWith('vela.req.'));
		return { session: keys(session), local: keys(local) };
	});

const workerLog = (wallet: Page) =>
	wallet.evaluate(async () => {
		const all = await (window as unknown as ChromeStorage).chrome.storage.session.get(
			'vela.sw.log'
		);
		return ((all['vela.sw.log'] as string[] | undefined) ?? []).join('\n');
	});

interface HistoryRow {
	userOpHash?: string;
	status?: string;
	chainId?: number;
}

/**
 * The wallet's own records of an op hash (`vela.transactionHistory`, IndexedDB
 * — the store the panel writes and the tracker resumes from). Every extension
 * page shares it with the panel.
 */
const historyOf = (wallet: Page, opHash: string) =>
	wallet.evaluate(
		(hash) =>
			new Promise<HistoryRow[]>((resolve, reject) => {
				const open = indexedDB.open('vela', 1);
				open.onerror = () => reject(open.error);
				open.onsuccess = () => {
					const read = open.result
						.transaction('kv', 'readonly')
						.objectStore('kv')
						.get('vela.transactionHistory');
					read.onerror = () => reject(read.error);
					read.onsuccess = () => {
						const rows: HistoryRow[] =
							typeof read.result === 'string' ? JSON.parse(read.result) : [];
						resolve(rows.filter((r) => r.userOpHash?.toLowerCase() === hash.toLowerCase()));
					};
				};
			}),
		opHash
	);

/**
 * What a page is told when a submit that may have been sent loses its surface
 * (RJ2): -32603 "not confirmed yet", naming the operation — never 4900, which
 * a dApp reads as "not sent" and pays again, and never the op hash as a
 * result, which no node the site asks knows (083, owner ruling 2026-10-01).
 * One answer: the page's own record of it is the same.
 */
function expectNotConfirmed(answer: AskResult, opHash: string): void {
	expect(answer).toEqual({
		ok: false,
		code: -32603,
		message: `${NOT_CONFIRMED_MESSAGE} (user operation ${opHash})`
	});
}

/** What the page's provider answered, per method (the test dApp's own record). */
const results = (page: Page) =>
	page.evaluate(
		() =>
			JSON.parse(document.getElementById('out')!.textContent!).results as Record<string, AskResult>
	);

/** Connect in the panel, the product's way: a trusted click, then Connect. */
async function connectInPanel(page: Page, wallet: Page): Promise<void> {
	await page.getByRole('button', { name: 'Connect' }).click();
	await (await sidePanelView(wallet)).click('Connect');
	await page.waitForFunction(
		() => JSON.parse(document.getElementById('out')!.textContent!).results.eth_requestAccounts
	);
	expect((await results(page)).eth_requestAccounts.result).toEqual([FIXTURE_ONE]);
	await expect.poll(() => sidePanelShowsRequest(wallet), { timeout: 10_000 }).toBe(false);
}

/**
 * The side panel's path, wherever it is — `sidePanelView` finds the panel
 * only on the wallet, where requests rise; this finds it on Settings too, by
 * the mark the first `?panel` load left (RB9).
 */
const panelPath = (wallet: Page) =>
	wallet.evaluate(() => {
		const views = (
			window as unknown as { chrome: { extension: { getViews(): Window[] } } }
		).chrome.extension.getViews();
		const panel = views.find((w) => {
			try {
				return w.sessionStorage.getItem('vela.surface.panel') === '1';
			} catch {
				return false;
			}
		});
		return panel?.location.pathname ?? null;
	});

// ---------------------------------------------------------------------------
// A chain and a relay the side panel can reach (G35's dust send, T216)
// ---------------------------------------------------------------------------

/**
 * The side panel is not a Playwright page, so `context.route` never sees its
 * traffic — which is why the dust send waited for a device (post2-E1). It
 * needs no route: the wallet reads its relay and its nodes from settings the
 * person can edit, so this stands a relay and every chain's node up on
 * 127.0.0.1 and points those settings at it (`pointAtStubNet`), and
 * `hermeticPanel` refuses anything else the panel asks for.
 *
 * The relay quotes and takes the op; its reply to `eth_sendUserOperation` is
 * held and never comes — the op "may have been sent", the one moment G35 is
 * about. The only relay the wallet knows from here on is this one, so nothing
 * can be sent anywhere real.
 */
const NET = `http://127.0.0.1:${NET_PORT}`;
const DUST_TO = '0x' + '4d'.repeat(20);
/** 0.000001 of the chain's coin — a dust send. */
const DUST_WEI = '0xe8d4a51000';

interface StubNet {
	server: Server;
	/** Every request, `time METHOD /path rpc_method`, in arrival order. */
	seen: string[];
	/** Each `eth_sendUserOperation` the relay was handed (never answered), and when. */
	posted: { at: number; params: unknown[] }[];
}

/** A deployed Safe holding 1.5 of the coin at $3,000, on every chain. */
function chainAnswer(chainId: number, method: string, params: unknown[]): unknown {
	switch (method) {
		case 'eth_chainId':
			return '0x' + chainId.toString(16);
		case 'eth_blockNumber':
			return '0x10';
		case 'eth_getCode':
			// The Safe and the EntryPoint are deployed (no initCode, a ready
			// network); the recipient is a person.
			return String(params[0]).toLowerCase() === DUST_TO ? '0x' : '0x6080';
		case 'eth_getTransactionCount':
			return '0x0';
		case 'eth_gasPrice':
		case 'eth_maxPriorityFeePerGas':
			return '0x3b9aca00';
		case 'eth_getBlockByNumber':
			return { number: '0x10', timestamp: '0x66000000', baseFeePerGas: '0x3b9aca00' };
		case 'eth_getBalance':
			return '0x14d1120d7b160000';
		case 'eth_estimateGas':
			return '0x5208';
		case 'eth_getLogs':
			return [];
		case 'eth_call': {
			const call = params[0] as { data?: string } | undefined;
			const n = call?.data?.startsWith('0x82ad56cb') ? aggregate3CallCount(call.data) : 0;
			if (n === 0) return '0x' + abiWord(0);
			const data =
				'0x' +
				abiWord(1_500_000_000_000_000_000n) +
				abiWord(3000n * 100_000_000n) +
				abiWord(0) +
				abiWord(0) +
				abiWord(0);
			return encodeAggregate3Result(Array.from({ length: n }, () => ({ success: true, data })));
		}
		default:
			return undefined;
	}
}

/** The op hash and the transaction the stand-in relay answers with, when it answers. */
const RELAY_OP = '0x' + 'e1'.repeat(32);
const RELAY_TX = '0x' + 'e2'.repeat(32);

/**
 * `answerSends`: the relay takes the op AND answers — its hash, and a landed
 * receipt (spec 094 S12: a send that succeeds end to end). Without it the
 * reply to `eth_sendUserOperation` is held, G35's moment.
 *
 * `refusesAfter`: the relay takes the op and answers its hash, then refuses
 * it — no receipt ever, and its status says `rejected`, naming no bundle
 * transaction (spec 097 N4: the pass's Aave withdraw, "Submitted", then
 * "the network refused this transaction; nothing was sent").
 */
function serveStubNet(
	options: { answerSends?: boolean; refusesAfter?: boolean } = {}
): Promise<StubNet> {
	const seen: string[] = [];
	const posted: StubNet['posted'] = [];
	const answers = options.answerSends || options.refusesAfter;
	const relay = happyRelay(RELAY_OP, RELAY_TX, () =>
		options.refusesAfter ? 'pending' : options.answerSends ? 'landed' : 'pending'
	);
	const cors = {
		'access-control-allow-origin': '*',
		'access-control-allow-headers': '*',
		'access-control-allow-methods': 'GET, POST, OPTIONS',
		'access-control-allow-private-network': 'true'
	};
	const server = createServer((req, res) => {
		if (req.method === 'OPTIONS') {
			res.writeHead(204, cors);
			res.end();
			return;
		}
		let raw = '';
		req.on('data', (chunk) => (raw += chunk));
		req.on('end', () => {
			const path = (req.url ?? '/').split('?')[0];
			let rpc: { id?: number; method?: string; params?: unknown[] } | null = null;
			if (req.method === 'POST') {
				try {
					rpc = JSON.parse(raw);
				} catch {
					rpc = null;
				}
			}
			seen.push(`${new Date().toISOString()} ${req.method} ${path} ${rpc?.method ?? ''}`);
			const send = (status: number, body: unknown) => {
				res.writeHead(status, { ...cors, 'content-type': 'application/json' });
				res.end(JSON.stringify(body));
			};
			const id = rpc?.id ?? 1;
			const node = /^\/rpc\/(\d+)$/.exec(path);
			if (node && rpc?.method) {
				const result = chainAnswer(Number(node[1]), rpc.method, rpc.params ?? []);
				// A method this node does not serve is an answer, not a sick node:
				// the pool must not cool the only endpoint it has for it.
				return send(
					200,
					result === undefined
						? { jsonrpc: '2.0', id, error: { code: -32601, message: 'the method does not exist' } }
						: { jsonrpc: '2.0', id, result }
				);
			}
			if (/^\/relay\/v1\/account\//.test(path)) {
				return send(200, {
					activeDepositAddress: '0x' + 'a1'.repeat(20),
					onchainBalance: '0x2386f26fc10000',
					spendableBalance: '0x2386f26fc10000',
					status: 'ACTIVE'
				});
			}
			if (/^\/relay\/v1\/treasury\//.test(path)) {
				return send(200, {
					address: '0x' + 'b2'.repeat(20),
					asset: 'native',
					balance: '0x8ac7230489e80000',
					floor: '0x2386f26fc10000',
					bootstrapNeeded: false
				});
			}
			if (/^\/relay\/v1\/sponsor\//.test(path)) return send(200, { sponsored: true });
			if (/^\/relay\/\d+$/.test(path) && rpc?.method) {
				if (rpc.method === 'eth_sendUserOperation') {
					posted.push({ at: Date.now(), params: rpc.params ?? [] });
					// Taken, and the reply held: the op may have been sent.
					if (!answers) return;
				}
				if (options.refusesAfter && rpc.method === 'pimlico_getUserOperationStatus') {
					return send(200, { jsonrpc: '2.0', id, result: { status: 'rejected' } });
				}
				const answer = relay(rpc.method, rpc.params ?? []);
				return send(
					200,
					answer !== null && typeof answer === 'object' && 'error' in answer
						? { jsonrpc: '2.0', id, error: (answer as { error: unknown }).error }
						: { jsonrpc: '2.0', id, result: answer ?? null }
				);
			}
			send(404, { error: 'not served here' });
		});
	});
	return new Promise((resolve) =>
		server.listen(NET_PORT, '127.0.0.1', () => resolve({ server, seen, posted }))
	);
}

/**
 * Point the wallet's relay, data service and every chain's node at the stub
 * — the settings a person edits under Service nodes and Networks, written
 * where the wallet keeps them. Before the panel opens, so it boots on them.
 */
async function pointAtStubNet(wallet: Page): Promise<void> {
	await wallet.evaluate(
		([net, chainIds]) => {
			localStorage.setItem(
				'vela.serviceEndpoints',
				JSON.stringify({
					ethereumDataURL: `${net}/data`,
					passkeyIndexURL: `${net}/index`,
					bundlerServiceURL: `${net}/relay`,
					fiatRatesURL: `${net}/fx`,
					aaguidDirectoryURL: `${net}/aaguid`
				})
			);
			const rows = chainIds.map((chainId) => ({ chainId, rpcURL: `${net}/rpc/${chainId}` }));
			return new Promise<void>((resolve, reject) => {
				const open = indexedDB.open('vela', 1);
				open.onupgradeneeded = () => open.result.createObjectStore('kv');
				open.onerror = () => reject(open.error);
				open.onsuccess = () => {
					const tx = open.result.transaction('kv', 'readwrite');
					tx.objectStore('kv').put(JSON.stringify(rows), 'vela.networkConfig');
					tx.oncomplete = () => resolve();
					tx.onerror = () => reject(tx.error);
				};
			});
		},
		[NET, CHAINS.map((c) => c.chainId)] as const
	);
}

/**
 * From here on the side panel's `fetch` reaches only 127.0.0.1, localhost and
 * the extension itself; anything else fails the way a dropped connection
 * does, and is listed on the panel as `__velaRefused`.
 */
async function hermeticPanel(wallet: Page): Promise<void> {
	await inSidePanel(
		wallet,
		`
		if (panel.__velaRefused) return;
		const real = panel.fetch.bind(panel);
		const refused = [];
		panel.fetch = (input, init) => {
			const url = typeof input === 'string' ? input : (input && input.url) || String(input);
			let local = true;
			try {
				const parsed = new panel.URL(url, panel.location.href);
				if (parsed.protocol === 'http:' || parsed.protocol === 'https:') {
					local = parsed.hostname === '127.0.0.1' || parsed.hostname === 'localhost';
				}
			} catch {
				local = false;
			}
			if (local) return real(input, init);
			refused.push(url.slice(0, 120));
			return panel.Promise.reject(new panel.TypeError('Failed to fetch'));
		};
		panel.__velaRefused = refused;
		`
	);
}

/** Is the side panel still the one `hermeticPanel` fenced (no reload since)? */
const panelFenced = (wallet: Page) =>
	inSidePanel<boolean>(wallet, 'return Array.isArray(panel.__velaRefused);');

/** Slide the panel's sheet — by keyboard, the same `onconfirm` — once it arms. */
async function slideInPanel(wallet: Page): Promise<void> {
	const TRACK = `[role="dialog"] [data-testid="signing-confirm"]`;
	await expect
		.poll(
			() =>
				inSidePanel<string | null>(
					wallet,
					`return panel.document.querySelector(arg)?.getAttribute('aria-disabled') ?? null;`,
					TRACK
				),
			{ timeout: 60_000 }
		)
		.toBe('false');
	await inSidePanel(
		wallet,
		`
		const track = panel.document.querySelector(arg);
		track.focus();
		track.dispatchEvent(new panel.KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
		`,
		TRACK
	);
}

/** Refuse whatever the panel's dialog shows, with its ✕ (the core's 4001). */
async function cancelInPanel(wallet: Page): Promise<void> {
	await inSidePanel(
		wallet,
		`
		const dialog = panel.document.querySelector('[role="dialog"]');
		const close = dialog && dialog.querySelector('button[aria-label]');
		if (!close) throw new Error('no close button in the panel dialog');
		close.click();
		`
	);
}

test.describe('a request’s life in the extension (spec 082)', () => {
	test.skip(!extensionBuilt(), 'extension/dist is missing — run `pnpm build:extension`');
	test.setTimeout(180_000);

	let dapp: Server;
	let dappB: Server;
	test.beforeAll(async () => {
		dapp = await serveDApp(DAPP_PORT);
		dappB = await serveDApp(DAPP_B_PORT);
	});
	test.afterAll(async () => {
		await closeServer(dapp);
		await closeServer(dappB);
	});

	test('EX5 (G17): a reload during a sheet settles the old request; only the new one shows', async () => {
		const context = await loadExtension({ surface: 'panel' });
		const wallet = await seedWallet(context, extensionId());
		const page = await context.newPage();
		await page.goto(`http://localhost:${DAPP_PORT}/`);
		await connectInPanel(page, wallet);

		await page.getByRole('button', { name: 'Sign' }).click();
		await sidePanelView(wallet, 20_000);
		expect((await ledger(wallet)).session).toHaveLength(1);

		// The page leaves without deciding.
		await page.reload();
		await expect.poll(() => sidePanelShowsRequest(wallet), { timeout: 5_000 }).toBe(false);
		await expect.poll(async () => (await ledger(wallet)).session, { timeout: 5_000 }).toEqual([]);
		expect(await workerLog(wallet)).toMatch(/req\.settled cause=page_left/);

		// Asked again, it shows once, and is answered once.
		await page.getByRole('button', { name: 'Sign' }).click();
		await sidePanelView(wallet, 20_000);
		await cancelInPanel(wallet);
		await page.waitForFunction(
			() => JSON.parse(document.getElementById('out')!.textContent!).results.personal_sign
		);
		expect((await results(page)).personal_sign).toMatchObject({ ok: false, code: 4001 });
		await expect.poll(async () => (await ledger(wallet)).session, { timeout: 5_000 }).toEqual([]);
		expect((await ledger(wallet)).local).toEqual([]);
		await context.close();
	});

	test('EX4 (G18): a second tab’s request queues in the same panel and is answered next', async () => {
		const context = await loadExtension({ surface: 'panel' });
		const wallet = await seedWallet(context, extensionId());
		const a = await context.newPage();
		await a.goto(`http://localhost:${DAPP_PORT}/`);
		const b = await context.newPage();
		await b.goto(`http://localhost:${DAPP_B_PORT}/`);

		await a.bringToFront();
		await a.getByRole('button', { name: 'Connect' }).click();
		const first = await sidePanelView(wallet);
		expect(first.heading).toContain(`localhost:${DAPP_PORT}`);

		await b.bringToFront();
		await b.getByRole('button', { name: 'Connect' }).click();
		// A answers first; B's card follows in the same panel.
		await first.click('Cancel');
		await a.waitForFunction(
			() => JSON.parse(document.getElementById('out')!.textContent!).results.eth_requestAccounts
		);
		expect((await results(a)).eth_requestAccounts).toMatchObject({ ok: false, code: 4001 });

		await expect
			.poll(async () => (await sidePanelView(wallet, 5_000)).heading, { timeout: 15_000 })
			.toContain(`localhost:${DAPP_B_PORT}`);
		await (await sidePanelView(wallet)).click('Connect');
		await b.waitForFunction(
			() => JSON.parse(document.getElementById('out')!.textContent!).results.eth_requestAccounts
		);
		expect((await results(b)).eth_requestAccounts.result).toEqual([FIXTURE_ONE]);
		// Each tab heard exactly its own one answer.
		expect((await results(a)).eth_requestAccounts).toMatchObject({ ok: false, code: 4001 });
		await expect.poll(async () => (await ledger(wallet)).session, { timeout: 5_000 }).toEqual([]);
		await context.close();
	});

	test('EX6: closing the side panel answers 4900 "The browser closed…" at once', async () => {
		const context = await loadExtension({ surface: 'panel' });
		const wallet = await seedWallet(context, extensionId());
		const page = await context.newPage();
		await page.goto(`http://localhost:${DAPP_PORT}/`);
		await connectInPanel(page, wallet);

		await page.getByRole('button', { name: 'Sign' }).click();
		await sidePanelView(wallet, 20_000);
		// What Chrome's ✕ does to the panel: its document goes, and its port with it.
		await inSidePanel(wallet, 'panel.close();').catch(() => {});
		await page.waitForFunction(
			() => JSON.parse(document.getElementById('out')!.textContent!).results.personal_sign,
			null,
			{ timeout: 10_000 }
		);
		expect((await results(page)).personal_sign).toEqual({
			ok: false,
			code: 4900,
			message: 'The browser closed before the request finished'
		});
		await expect.poll(async () => (await ledger(wallet)).session, { timeout: 5_000 }).toEqual([]);
		expect(await workerLog(wallet)).toMatch(/req\.settled cause=surface_closed/);
		await context.close();
	});

	/**
	 * G35 (P0, RJ2): the device pass closed the side panel after the slide and
	 * the page was told 4900 "The browser closed…" while the op landed 13 s
	 * later — a dApp that reads 4900 as "not sent" pays again. Once the submit
	 * claim carries the operation hash, the page is told it is not confirmed
	 * yet, naming that hash (083: never the hash as if it were a transaction).
	 *
	 * The claim here is made on the worker's own `vela.surface` port, by a
	 * stand-in for the panel in the wallet tab: the worker, the page, the
	 * provider and content.js are the real ones, and nothing is signed or
	 * sent. The next case makes the same claim the product's way — a dust send
	 * slid in the panel itself.
	 */
	test('G35 (RJ2): a claimed submit whose panel goes is answered "not confirmed yet" with its op hash, never 4900', async () => {
		const context = await loadExtension({ surface: 'panel' });
		const wallet = await seedWallet(context, extensionId());
		const page = await context.newPage();
		await page.goto(`http://localhost:${DAPP_PORT}/`);
		await connectInPanel(page, wallet);

		const OP = `0x${'ab'.repeat(32)}`;
		const asked = page.evaluate(() =>
			window.__ask('eth_sendTransaction', [
				{
					from: window.ethereum.selectedAddress,
					to: '0x3333333333333333333333333333333333333333',
					value: '0x0'
				}
			])
		);
		await expect
			.poll(async () => (await ledger(wallet)).session, { timeout: 10_000 })
			.toHaveLength(1);

		// The surface's half of the protocol, from an extension page, for the
		// window the request is owed in.
		const outcome = await wallet.evaluate(
			async ([port, op]) => {
				type Msg = { type: string; nonce?: number; live?: boolean; request?: { rid: string } };
				type Port = {
					postMessage(m: unknown): void;
					disconnect(): void;
					onMessage: { addListener(fn: (m: Msg) => void): void };
				};
				const chrome = (
					window as unknown as {
						chrome: {
							tabs: { query(q: { url: string }): Promise<{ windowId: number }[]> };
							runtime: { connect(info: { name: string }): Port };
						};
					}
				).chrome;
				const [tab] = await chrome.tabs.query({ url: `http://localhost:${port}/*` });
				const surface = chrome.runtime.connect({ name: 'vela.surface' });
				const heard: Msg[] = [];
				surface.onMessage.addListener((m: Msg) => heard.push(m));
				const next = async (match: (m: Msg) => boolean) => {
					for (let i = 0; i < 100; i += 1) {
						const found = heard.find(match);
						if (found) return found;
						await new Promise((r) => setTimeout(r, 50));
					}
					throw new Error('the worker said nothing');
				};
				surface.postMessage({ type: 'hello', kind: 'panel', windowId: tab.windowId });
				const owed = await next((m) => m.type === 'owed');
				const rid = owed.request!.rid;
				surface.postMessage({ type: 'claim', rid, phase: 'sign', nonce: 1 });
				const signed = await next((m) => m.type === 'claimResult' && m.nonce === 1);
				surface.postMessage({
					type: 'claim',
					rid,
					phase: 'submit',
					nonce: 2,
					opHash: op,
					chainId: 100
				});
				const submitted = await next((m) => m.type === 'claimResult' && m.nonce === 2);
				// Chrome's ✕: the surface's port goes.
				surface.disconnect();
				return { sign: signed.live, submit: submitted.live };
			},
			[DAPP_PORT, OP] as const
		);
		expect(outcome).toEqual({ sign: true, submit: true });

		expectNotConfirmed(await asked, OP);
		await expect.poll(async () => (await ledger(wallet)).session, { timeout: 5_000 }).toEqual([]);
		// Logged after the page took the answer: waited for, not read once.
		await expect
			.poll(() => workerLog(wallet), { timeout: 5_000 })
			.toMatch(/req\.answered cause=surface_closed maybe_sent=1/);
		expect(await workerLog(wallet)).not.toMatch(/req\.settled cause=surface_closed/);
		// The page's receipt reads for that hash are translated from now on (RF3).
		const remembered = await wallet.evaluate(
			(op) => (window as unknown as ChromeStorage).chrome.storage.local.get(`vela.ext.op.${op}`),
			OP
		);
		expect(remembered[`vela.ext.op.${OP}`]).toMatchObject({ chainId: 100 });
		await context.close();
	});

	/**
	 * G35 as the device pass met it (post2-E1): a send slid in the side panel,
	 * the relay has the op, its reply is still out — and the person closes the
	 * panel. The panel's own submit claim carries the op's hash (T228), made
	 * after the write-ahead and before the POST, so the page is told it is not
	 * confirmed yet, naming that hash: one answer, never 4900 (which a dApp
	 * reads as "not sent" and pays again).
	 *
	 * Signed for real by the parallel space's fixture key; the relay and the
	 * chain are the stand-ins above, so nothing leaves the machine.
	 *
	 * The device met G35 twice (`evidence/extension/post-EX-W1-T182.txt`): a
	 * RELOAD of the panel while the relay's reply was lost (19:27:13 —
	 * `panel.down`, 4900, `panel.up` 55 ms later), and a close (19:29:02).
	 * Both run here. The reloaded panel comes back owing nothing: no sheet for
	 * the answered request, no second answer.
	 */
	for (const how of ['close', 'reload'] as const) {
		const title =
			how === 'close'
				? 'G35 (RJ2): a dust send slid in the panel, the panel closed after its submit claim → one "not confirmed yet" naming its op hash'
				: 'G35 (RJ2): a dust send slid in the panel, the panel reloaded after its submit claim → one "not confirmed yet" naming its op hash, and the new panel owes nothing';
		test(title, async () => {
			await dustSendThenLosePanel(how);
		});
	}

	async function dustSendThenLosePanel(how: 'close' | 'reload'): Promise<void> {
		const net = await serveStubNet();
		const context = await loadExtension({ surface: 'panel' });
		try {
			const wallet = await seedWallet(context, extensionId());
			await pointAtStubNet(wallet);
			// The pages the harness CAN route reach nothing off this machine either.
			await context.route(/^https?:\/\/(?!localhost[:/]|127\.0\.0\.1[:/])/, (route) =>
				route.abort('blockedbyclient')
			);
			const page = await context.newPage();
			await page.goto(`http://localhost:${DAPP_PORT}/`);
			await connectInPanel(page, wallet);
			await hermeticPanel(wallet);

			const asked = page.evaluate(
				([to, value]) =>
					window.__ask('eth_sendTransaction', [
						{ from: window.ethereum.selectedAddress, to, value }
					]),
				[DUST_TO, DUST_WEI] as const
			);
			// Awaited below; a failure before then must be reported as itself,
			// not as this promise dying with the context.
			asked.catch(() => {});
			await sidePanelView(wallet, 30_000);
			expect(await panelFenced(wallet)).toBe(true);
			await slideInPanel(wallet);

			// The fixture key signed, the record was written, the claim went, and
			// the relay has the op — this dust send, from this Safe. Its reply
			// never comes.
			await expect.poll(() => net.posted.length, { timeout: 60_000 }).toBeGreaterThan(0);
			const sent = net.posted[0];
			const [op] = sent.params as [{ sender?: string; callData?: string }];
			expect(op.sender?.toLowerCase()).toBe(FIXTURE_ONE.toLowerCase());
			expect(op.callData?.toLowerCase()).toContain(DUST_TO.slice(2));

			// The claim carried the op's hash and chain, and came before the POST.
			const claimed = await wallet.evaluate(async () => {
				const all = await (window as unknown as ChromeStorage).chrome.storage.session.get(null);
				return Object.entries(all)
					.filter(([k]) => k.startsWith('vela.req.'))
					.map(
						([, v]) => v as { state?: string; phase?: string; opHash?: string; chainId?: number }
					);
			});
			expect(claimed).toHaveLength(1);
			expect(claimed[0]).toMatchObject({ state: 'claimed', phase: 'submit' });
			expect(claimed[0].opHash).toMatch(/^0x[0-9a-f]{64}$/i);
			expect(claimed[0].chainId).toBeGreaterThan(0);
			const opHash = claimed[0].opHash!;
			const claimLine = /(\S+) req\.claim phase=submit live=true/.exec(await workerLog(wallet));
			expect(claimLine).not.toBeNull();
			expect(Date.parse(claimLine![1])).toBeLessThanOrEqual(sent.at);
			// RJ1, which the hash rests on: the wallet wrote the op down under that
			// hash before the bytes left — pending, for the tracker to follow. A
			// page told a hash the wallet holds no record of would wait on an op
			// nothing ever closes.
			const ahead = await historyOf(wallet, opHash);
			expect(ahead).toHaveLength(1);
			expect(ahead[0]).toMatchObject({ status: 'pending', chainId: claimed[0].chainId });

			// Chrome's ✕, or a reload of the panel, while the op's fate is unknown.
			await inSidePanel(
				wallet,
				how === 'close' ? 'panel.close();' : 'panel.location.reload();'
			).catch(() => {});
			expectNotConfirmed(await asked, opHash);
			await expect.poll(async () => (await ledger(wallet)).session, { timeout: 5_000 }).toEqual([]);

			// One answer: no settlement and no second answer, however it was
			// reached — the port going (`surface_closed`) or the panel's own
			// teardown (`surface_settled`). The worker logs it after the page took
			// it, so the line is waited for, not read once.
			await expect
				.poll(() => workerLog(wallet), { timeout: 5_000 })
				.toMatch(/req\.answered cause=surface_(closed|settled) maybe_sent=1/);
			if (how === 'reload') {
				// The reloaded panel says hello again — and is handed nothing: the
				// answered request is not owed, so no sheet rises for it.
				await expect
					.poll(
						async () =>
							/req\.answered[^\n]*maybe_sent=1[\s\S]*panel\.up/.test(await workerLog(wallet)),
						{
							timeout: 15_000
						}
					)
					.toBe(true);
				await page.waitForTimeout(1_000);
				expect(await sidePanelUp(wallet)).toBe(true);
				expect(await sidePanelShowsRequest(wallet)).toBe(false);
				expect((await ledger(wallet)).session).toEqual([]);
			}
			const log = await workerLog(wallet);
			expect(log.match(/maybe_sent=1/g)).toHaveLength(1);
			expect(log).not.toMatch(/req\.settled/);
			await page.waitForTimeout(1_000);
			expectNotConfirmed((await results(page)).eth_sendTransaction, opHash);
			// Its receipt reads are translated from now on (RF3).
			const remembered = await wallet.evaluate(
				(hash) =>
					(window as unknown as ChromeStorage).chrome.storage.local.get(`vela.ext.op.${hash}`),
				opHash
			);
			expect(remembered[`vela.ext.op.${opHash}`]).toMatchObject({ chainId: claimed[0].chainId });
			// The record outlives the panel as it was: still pending, neither failed
			// nor dropped as "never sent" — the tracker alone closes it.
			expect(await historyOf(wallet, opHash)).toEqual([
				expect.objectContaining({ status: 'pending' })
			]);
			// The panel's teardown aborts the POST in flight, and the pool may
			// fail it over — the SAME signed op again, which the relay knows by
			// its hash. Never a second op: that would be a second payment.
			for (const again of net.posted) expect(again.params).toEqual(sent.params);
		} finally {
			await test
				.info()
				.attach('stub-net', { body: net.seen.join('\n'), contentType: 'text/plain' });
			await context.close();
			await closeServer(net.server);
		}
	}

	/**
	 * Spec 094 S12 / S6: what the suites never had — a dApp transaction that
	 * SUCCEEDS end to end in the extension, and a batch whose status the dApp
	 * asks afterwards. Signed for real by the parallel space's fixture key; the
	 * relay and every chain are the stand-ins above, which answer this time.
	 */
	async function withAnsweringNet(
		body: (wallet: Page, page: Page, net: StubNet) => Promise<void>
	): Promise<void> {
		const net = await serveStubNet({ answerSends: true });
		const context = await loadExtension({ surface: 'panel' });
		try {
			const wallet = await seedWallet(context, extensionId());
			await pointAtStubNet(wallet);
			await context.route(/^https?:\/\/(?!localhost[:/]|127\.0\.0\.1[:/])/, (route) =>
				route.abort('blockedbyclient')
			);
			const page = await context.newPage();
			await page.goto(`http://localhost:${DAPP_PORT}/`);
			await connectInPanel(page, wallet);
			await hermeticPanel(wallet);
			await body(wallet, page, net);
		} finally {
			await test
				.info()
				.attach('stub-net', { body: net.seen.join('\n'), contentType: 'text/plain' });
			await context.close();
			await closeServer(net.server);
		}
	}

	test('eth_sendTransaction: slid in the panel, sent to the relay, landed — the page gets the transaction', async () => {
		await withAnsweringNet(async (wallet, page, net) => {
			const asked = page.evaluate(
				([to, value]) =>
					window.__ask('eth_sendTransaction', [
						{ from: window.ethereum.selectedAddress, to, value }
					]),
				[DUST_TO, DUST_WEI] as const
			);
			asked.catch(() => {});
			await sidePanelView(wallet, 30_000);
			await slideInPanel(wallet);
			await expect.poll(() => net.posted.length, { timeout: 60_000 }).toBeGreaterThan(0);
			const answer = (await asked) as AskResult;
			expect(answer.ok).toBe(true);
			// The landed transaction — or, if the receipt came late, the op that
			// is still confirming (079); never a failure, never 4900.
			expect([RELAY_TX, RELAY_OP]).toContain(String(answer.result).toLowerCase());
			await expect.poll(async () => (await ledger(wallet)).session, { timeout: 5_000 }).toEqual([]);
		});
	});

	/**
	 * Spec 097 G: the request declares EIP-5792 2.0.0, so the answer is the
	 * object `{ id }`. The page reads `.id` off it exactly as Uniswap (and
	 * viem) do — Uniswap read `.id` off the bare string it got, asked
	 * `wallet_getCallsStatus([null])` and called a landed swap failed.
	 */
	test('wallet_sendCalls: the page gets the batch id, and wallet_getCallsStatus reads it as confirmed', async () => {
		await withAnsweringNet(async (wallet, page, net) => {
			const asked = page.evaluate(
				([to, value]) =>
					window.__ask('wallet_sendCalls', [
						{
							version: '2.0.0',
							from: window.ethereum.selectedAddress,
							atomicRequired: true,
							calls: [{ to, value }]
						}
					]),
				[DUST_TO, DUST_WEI] as const
			);
			asked.catch(() => {});
			await sidePanelView(wallet, 30_000);
			await slideInPanel(wallet);
			await expect.poll(() => net.posted.length, { timeout: 60_000 }).toBeGreaterThan(0);
			const sent = (await asked) as AskResult;
			expect(sent.ok).toBe(true);
			expect(sent.result).toEqual({ id: expect.any(String) });
			const id = (sent.result as { id: string }).id;
			expect(id.toLowerCase()).toBe(RELAY_OP);

			// What viem's `waitForCallsStatus` asks next — 4200 before 094 (089 F04).
			const capabilities = (await page.evaluate(() =>
				window.__ask('wallet_getCapabilities', [window.ethereum.selectedAddress])
			)) as AskResult;
			expect(capabilities.ok).toBe(true);
			expect(Object.values(capabilities.result as object)).toContainEqual({
				atomic: { status: 'supported' }
			});
			await expect
				.poll(
					async () =>
						(
							(await page.evaluate(
								(batch) => window.__ask('wallet_getCallsStatus', [batch]),
								id
							)) as {
								result?: { status?: number };
							}
						).result?.status,
					{ timeout: 30_000 }
				)
				.toBe(200);
			const status = (await page.evaluate(
				(batch) => window.__ask('wallet_getCallsStatus', [batch]),
				id
			)) as AskResult;
			expect(status.result).toMatchObject({
				version: '2.0.0',
				id: RELAY_OP,
				atomic: true,
				status: 200,
				receipts: [{ transactionHash: RELAY_TX, status: '0x1' }]
			});
		});
	});

	/**
	 * Spec 097 N4, as the pass met it in a request WINDOW: the relay took the
	 * op (the window said "Submitted"), then refused it — and the window
	 * closed ~12 s later with no words, the page told -32603 by nobody the
	 * person could see. The refusal now stays in the window, in its own
	 * words, with Done and no "Try again"; the page is not answered under it;
	 * Done answers it, once, and only then does the window go.
	 *
	 * Signed for real by the parallel space's fixture key; the relay and the
	 * chain are the stand-ins above, so nothing leaves the machine.
	 */
	test('097 N4: a refusal after "Submitted" stays in the request window until Done, which answers the page once', async () => {
		const net = await serveStubNet({ refusesAfter: true });
		const context = await loadExtension({ surface: 'window' });
		try {
			const wallet = await seedWallet(context, extensionId());
			await pointAtStubNet(wallet);
			await context.route(/^https?:\/\/(?!localhost[:/]|127\.0\.0\.1[:/])/, (route) =>
				route.abort('blockedbyclient')
			);
			const page = await context.newPage();
			await page.goto(`http://localhost:${DAPP_PORT}/`);
			const connected = page.evaluate(() => window.__ask('eth_requestAccounts'));
			const consent = await requestWindow(context);
			await consent.getByRole('button', { name: 'Connect' }).click();
			expect((await connected).result).toEqual([FIXTURE_ONE]);
			await noRequestWindow(context);

			const asked = page.evaluate(
				([to, value]) =>
					window.__ask('eth_sendTransaction', [
						{ from: window.ethereum.selectedAddress, to, value }
					]),
				[DUST_TO, DUST_WEI] as const
			);
			asked.catch(() => {});
			const win = await requestWindow(context, 30_000);
			// Armed first (the fee quoted): a slide made before it does nothing.
			await expect(win.getByTestId('signing-confirm')).toHaveAttribute('aria-disabled', 'false', {
				timeout: 60_000
			});
			await slideToConfirm(win);
			await expect.poll(() => net.posted.length, { timeout: 60_000 }).toBeGreaterThan(0);

			// The relay's status poll says rejected (the tracker asks every ~12 s).
			await expect(win.getByText('The network refused it — nothing was sent.')).toBeVisible({
				timeout: 60_000
			});
			await expect(win.getByText('Failed', { exact: true })).toBeVisible();
			await expect(win.getByRole('button', { name: 'Try Again' })).toHaveCount(0);
			// The window's own size (`openRequestWindow`: 420 × 760), for the picture.
			await win.setViewportSize({ width: 420, height: 760 });
			await win.screenshot({ path: test.info().outputPath('097-n4-refusal-window.png') });

			// Longer than every timer the window has (the 6 s hand-off grace, the
			// 5 s close backstop): it stays, and nobody has answered the page.
			await page.waitForTimeout(8_000);
			expect(win.isClosed()).toBe(false);
			await expect(win.getByText('The network refused it — nothing was sent.')).toBeVisible();
			expect((await results(page)).eth_sendTransaction).toBeUndefined();
			expect(await workerLog(wallet)).not.toMatch(/req\.answered[^\n]*outcome=error/);

			// Done: the one answer, the refusal's own; then the window goes.
			await win.getByRole('button', { name: 'Done' }).click();
			expect(await asked).toEqual({
				ok: false,
				code: -32603,
				message: 'the network refused this transaction; nothing was sent'
			});
			await noRequestWindow(context);
			await expect.poll(async () => (await ledger(wallet)).session, { timeout: 5_000 }).toEqual([]);
			await page.waitForTimeout(1_000);
			const log = await workerLog(wallet);
			expect(log.match(/req\.answered/g)).toHaveLength(2); // the connect, and this
			expect(log).toMatch(/req\.answered delivered=true outcome=error/);
			expect(log).not.toMatch(/maybe_sent=1|req\.settled/);
			// The relay refused it; it was never posted twice.
			expect(net.posted).toHaveLength(1);
		} finally {
			await test
				.info()
				.attach('stub-net', { body: net.seen.join('\n'), contentType: 'text/plain' });
			await context.close();
			await closeServer(net.server);
		}
	});

	test('EX8 (G19): a worker stopped mid-request resumes; the answer arrives once, with no Chrome text', async () => {
		const context = await loadExtension({ surface: 'panel' });
		const id = extensionId();
		const wallet = await seedWallet(context, id);
		const page = await context.newPage();
		await page.goto(`http://localhost:${DAPP_PORT}/`);
		await connectInPanel(page, wallet);

		await page.getByRole('button', { name: 'Sign' }).click();
		await sidePanelView(wallet, 20_000);

		// Stop the worker the way chrome://serviceworker-internals' Stop does.
		const cdp = await context.newCDPSession(wallet);
		const versions: string[] = [];
		cdp.on('ServiceWorker.workerVersionUpdated', (event) => {
			for (const v of event.versions) {
				if (v.scriptURL.startsWith(`chrome-extension://${id}/`) && v.runningStatus === 'running') {
					versions.push(v.versionId);
				}
			}
		});
		await cdp.send('ServiceWorker.enable');
		await expect.poll(() => versions.length, { timeout: 10_000 }).toBeGreaterThan(0);
		await cdp.send('ServiceWorker.stopWorker', { versionId: versions.at(-1)! });

		// The page's port reconnects and wakes a new worker, which resumes the
		// record rather than settling it; the panel's sheet is still there.
		await expect
			.poll(async () => await workerLog(wallet), { timeout: 15_000 })
			.toMatch(/sw\.start records=1 recovered=1/);
		expect(await sidePanelShowsRequest(wallet)).toBe(true);

		await cancelInPanel(wallet);
		await page.waitForFunction(
			() => JSON.parse(document.getElementById('out')!.textContent!).results.personal_sign,
			null,
			{ timeout: 15_000 }
		);
		const answer = (await results(page)).personal_sign;
		expect(answer).toMatchObject({ ok: false, code: 4001 });
		expect(answer.message ?? '').not.toMatch(/message channel|port closed/i);
		await expect.poll(async () => (await ledger(wallet)).session, { timeout: 5_000 }).toEqual([]);
		await context.close();
	});

	test('EX4b (RB9, G23c): the panel is still the panel after Wallet → Settings → Wallet', async () => {
		const context = await loadExtension({ surface: 'panel' });
		const wallet = await seedWallet(context, extensionId());
		const page = await context.newPage();
		await page.goto(`http://localhost:${DAPP_PORT}/`);
		await connectInPanel(page, wallet);
		expect(await sidePanelUp(wallet)).toBe(true);

		// In the panel: to Settings and back, the way a person taps it. The
		// return drops `?panel` from the URL.
		await inSidePanel(
			wallet,
			`
			const nav = (label) => {
				const link = [...panel.document.querySelectorAll('a, button')].find(
					(el) => el.getAttribute('aria-label') === label || el.textContent?.trim() === label
				);
				if (!link) throw new Error('no ' + label + ' in the panel');
				link.click();
			};
			nav('Settings');
			await new Promise((r) => setTimeout(r, 1500));
			nav('Wallet');
			await new Promise((r) => setTimeout(r, 1500));
			`
		);
		expect(await sidePanelUp(wallet)).toBe(true);

		await page.getByRole('button', { name: 'Sign' }).click();
		const sheet = await sidePanelView(wallet, 20_000);
		expect(sheet.heading.length).toBeGreaterThan(0);
		expect(context.pages().some((p) => p.url().includes('rid='))).toBe(false);
		await cancelInPanel(wallet);
		await page.waitForFunction(
			() => JSON.parse(document.getElementById('out')!.textContent!).results.personal_sign
		);
		expect((await results(page)).personal_sign).toMatchObject({ ok: false, code: 4001 });
		await context.close();
	});

	/**
	 * G55 (RB9): with the panel on Settings, tab B's Connect got
	 * `req.surface=panel` and then nothing — no card, badge or hint — until
	 * the person happened to tap 钱包 (58 s); a dApp would have waited out its
	 * 5 minutes. The panel now goes to the wallet by itself and the card rises.
	 */
	test('EX4b (G55): a request arriving while the panel shows Settings rises with no tap', async () => {
		const context = await loadExtension({ surface: 'panel' });
		const wallet = await seedWallet(context, extensionId());
		const a = await context.newPage();
		await a.goto(`http://localhost:${DAPP_PORT}/`);
		await connectInPanel(a, wallet);

		await inSidePanel(
			wallet,
			`
			const link = [...panel.document.querySelectorAll('a, button')].find(
				(el) => el.getAttribute('aria-label') === 'Settings' || el.textContent?.trim() === 'Settings'
			);
			if (!link) throw new Error('no Settings in the panel');
			link.click();
			`
		);
		await expect.poll(() => panelPath(wallet), { timeout: 10_000 }).toMatch(/settings/);

		const b = await context.newPage();
		await b.goto(`http://localhost:${DAPP_B_PORT}/`);
		await b.getByRole('button', { name: 'Connect' }).click();
		// No tap anywhere: the panel goes back to the wallet and B's card rises.
		const card = await sidePanelView(wallet, 10_000);
		expect(card.heading).toContain(`localhost:${DAPP_B_PORT}`);
		expect(await panelPath(wallet)).toMatch(/wallet/);

		await card.click('Cancel');
		await b.waitForFunction(
			() => JSON.parse(document.getElementById('out')!.textContent!).results.eth_requestAccounts
		);
		expect((await results(b)).eth_requestAccounts).toMatchObject({ ok: false, code: 4001 });
		await expect.poll(async () => (await ledger(wallet)).session, { timeout: 5_000 }).toEqual([]);
		await context.close();
	});
});

declare global {
	interface Window {
		__ask(method: string, params?: unknown[]): Promise<AskResult>;
		ethereum: { selectedAddress: string };
	}
}
