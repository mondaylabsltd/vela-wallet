/**
 * Every extension surface speaks the language the person chose (spec 086,
 * issue 317).
 *
 * The report: the extension set to English on a Chinese Chrome, and the dApp
 * signing sheet came up entirely in Chinese. The request WINDOW — what a page's
 * request opens when it carries no click, as a swap's Execute does — was opened
 * by the service worker in Chrome's UI language: a worker cannot read the
 * language pinned in Settings. It opens through a doorway now (`open.html`),
 * which asks the same rule as the side panel's.
 *
 * Chrome for Testing runs in English here (`--lang` does not move it on
 * macOS), so the report is replayed mirrored: Chrome in English, the wallet
 * set to 简体中文 — the same defect, the same code path.
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
	noRequestWindow,
	requestWindow,
	sidePanelShowsRequest,
	sidePanelView
} from './extension-helpers';

const APP_ROOT = join(import.meta.dirname, '..');
/** Ports 5186–5189 are this suite's. */
const SITE_A = 5188;
const SITE_B = 5189;
const FIXTURE_ONE = '0xD400866e00B055B20752a826CD5C89b811de130b';

interface AskResult {
	ok: boolean;
	code?: number;
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
			(
				window as unknown as {
					chrome: { storage: { local: { get(k: string): Promise<Record<string, unknown>> } } };
				}
			).chrome.storage.local
				.get('vela.ext.cache')
				.then((all) => all['vela.ext.cache'] !== undefined),
		null,
		{ timeout: 30_000 }
	);
	return page;
}

/** What Settings → Language writes (`preferences.setLanguage`), from the wallet's own page. */
async function chooseLanguage(wallet: Page, language: string): Promise<void> {
	await wallet.evaluate((value) => localStorage.setItem('vela.language', value), language);
}

const pageLocale = (page: Page) => page.evaluate(() => document.documentElement.lang);

/**
 * Every side panel of the extension (one per window a request opened one in),
 * read from another extension page: its language, and whether it is showing a
 * request.
 */
const panels = (extensionPage: Page) =>
	extensionPage.evaluate(() =>
		(window as unknown as { chrome: { extension: { getViews(): Window[] } } }).chrome.extension
			.getViews()
			.filter((w) => {
				try {
					return (
						w.location.pathname.endsWith('/wallet.html') &&
						(new URLSearchParams(w.location.search).has('panel') ||
							w.sessionStorage.getItem('vela.surface.panel') === '1')
					);
				} catch {
					return false;
				}
			})
			.map((w) => ({
				lang: w.document.documentElement.lang,
				asking: !!w.document.querySelector('[role="dialog"]')
			}))
	);

test.describe('the language a person chose, on every surface (issue 317)', () => {
	test.skip(!extensionBuilt(), 'extension/dist is missing — run `pnpm build:extension`');
	test.setTimeout(180_000);

	let siteA: Server;
	let siteB: Server;
	test.beforeAll(async () => {
		siteA = await serveDApp(SITE_A);
		siteB = await serveDApp(SITE_B);
	});
	test.afterAll(async () => {
		await closeServer(siteA);
		await closeServer(siteB);
	});

	test('the request window — consent card and signing sheet — speaks the chosen language, not Chrome’s', async () => {
		const context = await loadExtension();
		const id = extensionId();
		const wallet = await seedWallet(context, id);
		await chooseLanguage(wallet, 'zh');
		const page = await context.newPage();
		await page.goto(`http://localhost:${SITE_A}/`);

		// Connect: the window opens in Chinese, through the doorway.
		const asked = page.evaluate(() => window.__ask('eth_requestAccounts')) as Promise<AskResult>;
		const win = await requestWindow(context);
		await expect(win).toHaveURL(/\/zh\/request\.html\?rid=/);
		expect(await pageLocale(win)).toBe('zh');
		await expect(win.getByRole('heading')).toContainText('连接到');
		await win.getByRole('button', { name: '连接', exact: true }).click();
		expect((await asked).result).toEqual([FIXTURE_ONE]);
		await noRequestWindow(context);

		// A signature: the sheet itself is Chinese, end to end.
		const signed = page.evaluate(
			(account) => window.__ask('personal_sign', ['0x48656c6c6f2c2056656c61', account]),
			FIXTURE_ONE
		) as Promise<AskResult>;
		const sheet = await requestWindow(context);
		await expect(sheet).toHaveURL(/\/zh\/request\.html\?rid=/);
		const slider = sheet.getByRole('button', { name: /^滑动以确认/ });
		await slider.waitFor({ state: 'visible', timeout: 30_000 });
		await expect(sheet.getByText('签名账户')).toBeVisible();
		await expect(sheet.getByText(/Slide to confirm|Signing account|Technical details/)).toHaveCount(
			0
		);
		await sheet.screenshot({ path: process.env.LOCALE_SHOT ?? 'test-results/317-sheet-zh.png' });
		await slider.focus();
		await slider.press('Enter');
		const answer = await signed;
		expect(answer.ok).toBe(true);
		expect(String(answer.result)).toMatch(/^0x[0-9a-f]+$/);
		await context.close();
	});

	test('nothing chosen (first run): Chrome’s language', async () => {
		const context = await loadExtension();
		const id = extensionId();
		const wallet = await seedWallet(context, id);
		await wallet.evaluate(() => localStorage.removeItem('vela.language'));
		const page = await context.newPage();
		await page.goto(`http://localhost:${SITE_A}/`);
		const asked = page.evaluate(() => window.__ask('eth_requestAccounts')) as Promise<AskResult>;
		const win = await requestWindow(context);
		await expect(win).toHaveURL(/\/en\/request\.html\?rid=/);
		await win.getByRole('button', { name: 'Connect', exact: true }).click();
		expect((await asked).result).toEqual([FIXTURE_ONE]);

		// "Follow system" is not a pin either.
		await chooseLanguage(wallet, 'auto');
		await noRequestWindow(context);
		const again = page.evaluate(
			(account) => window.__ask('personal_sign', ['0x48656c6c6f', account]),
			FIXTURE_ONE
		) as Promise<AskResult>;
		const sheet = await requestWindow(context);
		await expect(sheet).toHaveURL(/\/en\/request\.html\?rid=/);
		await sheet.close();
		expect((await again).ok).toBe(false);
		await context.close();
	});

	test('a side panel open across a change follows it once it is idle — never under a request', async () => {
		const context = await loadExtension({ surface: 'panel' });
		const id = extensionId();
		const wallet = await seedWallet(context, id);
		const pageA = await context.newPage();
		await pageA.goto(`http://localhost:${SITE_A}/`);
		await pageA.getByRole('button', { name: 'Connect' }).click();
		await (await sidePanelView(wallet)).click('Connect');
		await expect.poll(() => sidePanelShowsRequest(wallet), { timeout: 10_000 }).toBe(false);
		expect(await inSidePanel<string>(wallet, 'return panel.document.documentElement.lang;')).toBe(
			'en'
		);

		// A request owed: the panel shows it, and the language changes in the tab.
		const pageB = await context.newPage();
		await pageB.goto(`http://localhost:${SITE_B}/`);
		await pageB.getByRole('button', { name: 'Connect' }).click();
		const card = await sidePanelView(wallet);
		await chooseLanguage(wallet, 'ja');
		// The card a person is reading is not redrawn under them.
		await new Promise((resolve) => setTimeout(resolve, 3_000));
		const asking = (await panels(wallet)).filter((p) => p.asking);
		expect(asking).toEqual([{ lang: 'en', asking: true }]);
		await card.click('Cancel');
		await pageB.waitForFunction(
			() => JSON.parse(document.getElementById('out')!.textContent!).results.eth_requestAccounts
		);
		const result = await pageB.evaluate(
			() => JSON.parse(document.getElementById('out')!.textContent!).results.eth_requestAccounts
		);
		expect(result.code).toBe(4001);

		// Idle: every panel goes back through its doorway, in Japanese.
		await expect
			.poll(async () => (await panels(wallet).catch(() => [])).map((p) => p.lang), {
				timeout: 20_000
			})
			.toEqual(expect.arrayContaining(['ja']));
		await expect
			.poll(
				async () =>
					(await panels(wallet).catch(() => [{ lang: '' }])).every((p) => p.lang === 'ja'),
				{ timeout: 20_000 }
			)
			.toBe(true);

		// And the next request is drawn in Japanese.
		await pageB.getByRole('button', { name: 'Connect' }).click();
		const next = await sidePanelView(wallet);
		expect(next.heading).toContain('に接続');
		await next.click('接続');
		await pageB.waitForFunction(
			() => JSON.parse(document.getElementById('out')!.textContent!).results.eth_requestAccounts?.ok
		);
		await context.close();
	});
});

test.describe('the side panel keeps the chosen language when a request arrives (issue 317)', () => {
	test.skip(!extensionBuilt(), 'extension/dist is missing — run `pnpm build:extension`');
	test.setTimeout(180_000);

	let site: Server;
	test.beforeAll(async () => {
		site = await serveDApp(SITE_A);
	});
	test.afterAll(async () => {
		await closeServer(site);
	});

	test('a panel opened in Japanese draws the request in Japanese — it is not sent to the English wallet', async () => {
		// A page the extension loads fresh has no route params, and the root
		// layout read `page.params.locale ?? 'en'`: the request that opened the
		// panel took it to `/en/wallet`, and the card and sheet were English.
		const context = await loadExtension({ surface: 'panel' });
		const id = extensionId();
		const wallet = await seedWallet(context, id);
		await chooseLanguage(wallet, 'ja');
		const page = await context.newPage();
		await page.goto(`http://localhost:${SITE_A}/`);
		await page.getByRole('button', { name: 'Connect' }).click();
		const card = await sidePanelView(wallet);
		expect(card.heading).toContain('に接続');
		expect((await panels(wallet)).map((p) => p.lang)).toEqual(['ja']);
		await card.click('接続');
		await page.waitForFunction(
			() => JSON.parse(document.getElementById('out')!.textContent!).results.eth_requestAccounts?.ok
		);
		await context.close();
	});
});

declare global {
	interface Window {
		__ask(method: string, params?: unknown[]): Promise<AskResult>;
	}
}
