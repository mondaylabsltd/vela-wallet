/**
 * A request's life in the packaged extension (spec 082 T100, RH5) — the
 * device rows EX4, EX4b, EX5, EX6 and EX8, on the real worker and the real
 * side panel.
 *
 * Each of these failed on 079's build: a reload left the old sheet up (G17),
 * a second tab's request never showed (G18), a stopped worker lost a live
 * request and the page read Chrome's "message channel closed" (G19), the
 * panel's ✕ answered nothing (EX6), and the panel stopped being the panel
 * after a visit to Settings (G23c). Every case asserts the same three things:
 * one answer per request, 4900 with plain words when nobody decided, and no
 * `vela.req.*` left behind.
 *
 * Its own ports (8827/8828), so it never shares a server with the other
 * extension suites (8817/8818) or another session's run.
 */
import { createServer, type Server } from 'node:http';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test, type BrowserContext, type Page } from '@playwright/test';
import {
	extensionBuilt,
	extensionId,
	inSidePanel,
	loadExtension,
	sidePanelShowsRequest,
	sidePanelUp,
	sidePanelView
} from './extension-helpers';

const APP_ROOT = join(import.meta.dirname, '..');
const DAPP_PORT = 8827;
const DAPP_B_PORT = 8828;
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
});

declare global {
	interface Window {
		__ask(method: string, params?: unknown[]): Promise<AskResult>;
	}
}
