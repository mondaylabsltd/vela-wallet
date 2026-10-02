/**
 * A dApp connects, and is told about, only the account the extension is
 * signed in to (spec 086, issue #315).
 *
 * The report: signed in to "26", Uniswap kept showing and connecting the
 * previously used account — through a disconnect, a refresh and a reconnect.
 * The grant was answered for ANY account the device held, and only a wallet
 * document that SAW the switch re-pinned it; a sign-out left the worker's
 * snapshot behind; and a side panel open across the switch went on answering
 * as the account it booted with. Each test is one of those roads, on the real
 * worker and the real wallet, with the parallel space's fixed keys.
 *
 * Signing in to another wallet needs a passkey ceremony against the index
 * service, which a hermetic test cannot reach; its END STATE is what a fresh
 * wallet document finds in storage, and that is what these write.
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
/** Two sites, two origins (ports 5186–5189 are this suite's). */
const SITE_A = 5186;
const SITE_B = 5187;
const FIXTURE_ONE = '0xD400866e00B055B20752a826CD5C89b811de130b';

interface AskResult {
	ok: boolean;
	code?: number;
	result?: unknown;
}
interface DAppState {
	events: { event: string; data: unknown }[];
	results: Record<string, AskResult>;
}
type ChromeStorage = {
	chrome: { storage: { local: { get(k: string | null): Promise<Record<string, unknown>> } } };
};

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

/** A wallet inside the extension, with the fixture keyset; Parallel One signed in. */
async function seedWallet(context: BrowserContext, id: string): Promise<Page> {
	const page = await context.newPage();
	await page.addInitScript(() => {
		localStorage.setItem('vela.intro.seen', String(Date.now()));
		localStorage.setItem('vela.dev.console', '1');
	});
	await page.goto(`chrome-extension://${id}/en/parallel.html`);
	await page.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
	await page.waitForURL(/wallet\.html$/, { timeout: 30_000 });
	await snapshotSays(page, FIXTURE_ONE);
	return page;
}

/** The fixture accounts as the wallet stored them. */
async function storedAccounts(wallet: Page): Promise<{ name: string; address: string }[]> {
	return wallet.evaluate(() => JSON.parse(localStorage.getItem('vela.accounts') ?? '[]'));
}

/** Wait until the worker's snapshot names `address` as signed in (`null`: nobody). */
async function snapshotSays(page: Page, address: string | null): Promise<void> {
	await page.waitForFunction(
		(want) =>
			(window as unknown as ChromeStorage).chrome.storage.local
				.get('vela.ext.cache')
				.then((all) => {
					const snapshot = all['vela.ext.cache'] as { address?: string } | undefined;
					return want === null
						? snapshot === undefined
						: snapshot?.address?.toLowerCase() === want.toLowerCase();
				}),
		address,
		{ timeout: 30_000 }
	);
}

const readState = (page: Page) =>
	page.evaluate(() => JSON.parse(document.getElementById('out')!.textContent!) as DAppState);
const accountsOf = async (page: Page) =>
	((await page.evaluate(() => window.__ask('eth_accounts'))) as AskResult).result;

/** Connect through the fallback window (no gesture: a page-fired request). */
async function connectInWindow(context: BrowserContext, page: Page): Promise<unknown> {
	const asked = page.evaluate(() => window.__ask('eth_requestAccounts')) as Promise<AskResult>;
	const win = await requestWindow(context);
	await win.getByRole('button', { name: 'Connect' }).click();
	await noRequestWindow(context);
	return (await asked).result;
}

/** Wait for the page to have heard `event` (with `data`, when given). */
async function heard(page: Page, event: string, data?: unknown): Promise<void> {
	await page.waitForFunction(
		([name, want]) =>
			JSON.parse(document.getElementById('out')!.textContent!).events.some(
				(e: { event: string; data: unknown }) =>
					e.event === name && (want === undefined || JSON.stringify(e.data) === want)
			),
		[event, data === undefined ? undefined : JSON.stringify(data)] as const,
		{ timeout: 30_000 }
	);
}

test.describe('the connected account follows the signed-in account (#315)', () => {
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

	test('signed out, then into another wallet: the site is told, and never gets the old account back', async () => {
		// The phone layout: Sign Out is a row on Settings itself.
		const context = await loadExtension({ viewport: { width: 390, height: 844 } });
		const id = extensionId();
		const wallet = await seedWallet(context, id);
		const two = (await storedAccounts(wallet))[1];
		const page = await context.newPage();
		await page.goto(`http://localhost:${SITE_A}/`);
		expect(await connectInWindow(context, page)).toEqual([FIXTURE_ONE]);

		// Sign out, the way a person does it: Settings → Sign Out → confirm.
		await wallet.goto(`chrome-extension://${id}/en/settings.html`);
		await wallet.getByRole('button', { name: 'Sign Out', exact: true }).first().click();
		await wallet
			.getByRole('dialog')
			.filter({ has: wallet.getByRole('button', { name: 'Cancel' }) })
			.getByRole('button', { name: 'Sign Out', exact: true })
			.click();
		await snapshotSays(wallet, null);

		// The site hears that nobody is connected, and asking says the same.
		await heard(page, 'accountsChanged', []);
		expect(await accountsOf(page)).toEqual([]);

		// Signed in to Parallel Two alone, in a fresh wallet document.
		await wallet.evaluate((account) => {
			localStorage.setItem('vela.accounts', JSON.stringify([account]));
			localStorage.setItem('vela.activeAccountIndex', '0');
		}, two);
		await wallet.close();
		const fresh = await context.newPage();
		await fresh.goto(`chrome-extension://${id}/en/wallet.html`);
		await snapshotSays(fresh, two.address);

		// Parallel One left this device: her grant is dropped and the site cut
		// off — never handed Parallel Two without being asked.
		await heard(page, 'disconnect');
		expect(await accountsOf(page)).toEqual([]);

		// Reconnecting ASKS, and connects the account the person is in.
		expect(await connectInWindow(context, page)).toEqual([two.address]);
		expect(await accountsOf(page)).toEqual([two.address]);
		await context.close();
	});

	test('a wallet document that boots on another account carries the site over at once', async () => {
		const context = await loadExtension();
		const id = extensionId();
		const wallet = await seedWallet(context, id);
		const two = (await storedAccounts(wallet))[1];
		const page = await context.newPage();
		await page.goto(`http://localhost:${SITE_A}/`);
		expect(await connectInWindow(context, page)).toEqual([FIXTURE_ONE]);

		// Parallel Two is signed in by a document that is gone before it could
		// follow (this page's own write is not an event to itself) — the boot
		// of the NEXT wallet document is the first to see it. It used to be "a
		// boot, not a switch", and the site kept Parallel One for good.
		await wallet.evaluate(() => localStorage.setItem('vela.activeAccountIndex', '1'));
		await wallet.close();
		const fresh = await context.newPage();
		await fresh.goto(`chrome-extension://${id}/en/wallet.html`);
		await snapshotSays(fresh, two.address);

		await heard(page, 'accountsChanged', [two.address]);
		expect(await accountsOf(page)).toEqual([two.address]);
		// A reconnect is answered with Parallel Two, with nobody asked (an ask
		// would leave this waiting on a Connect nobody presses).
		const again = (await page.evaluate(() => window.__ask('eth_requestAccounts'))) as AskResult;
		expect(again.result).toEqual([two.address]);
		expect((await readState(page)).events.some((e) => e.event === 'disconnect')).toBe(false);
		await context.close();
	});

	test('a side panel open across a switch in the wallet tab connects the account switched to', async () => {
		const context = await loadExtension({ surface: 'panel' });
		const id = extensionId();
		const wallet = await seedWallet(context, id);
		const two = (await storedAccounts(wallet))[1];

		// Site A connects in the panel, on Parallel One.
		const pageA = await context.newPage();
		await pageA.goto(`http://localhost:${SITE_A}/`);
		await pageA.getByRole('button', { name: 'Connect' }).click();
		await (await sidePanelView(wallet)).click('Connect');
		await pageA.waitForFunction(
			() => JSON.parse(document.getElementById('out')!.textContent!).results.eth_requestAccounts
		);
		expect((await readState(pageA)).results.eth_requestAccounts.result).toEqual([FIXTURE_ONE]);
		await expect.poll(() => sidePanelShowsRequest(wallet), { timeout: 10_000 }).toBe(false);

		// In the wallet TAB, the switcher → Parallel Two. The panel stays open.
		await wallet.bringToFront();
		await wallet
			.getByRole('button', { name: /Parallel One/ })
			.first()
			.click();
		await wallet
			.getByRole('dialog')
			.getByRole('button', { name: /Parallel Two/ })
			.click();
		await snapshotSays(wallet, two.address);
		await heard(pageA, 'accountsChanged', [two.address]);

		// The panel follows the switch it did not make…
		await expect
			.poll(() => inSidePanel<string>(wallet, 'return panel.document.body.innerText;'), {
				timeout: 15_000
			})
			.toContain('Parallel Two');

		// …so a NEW site it connects gets the account switched to — it used to
		// get the one the panel booted with.
		const pageB = await context.newPage();
		await pageB.goto(`http://localhost:${SITE_B}/`);
		await pageB.getByRole('button', { name: 'Connect' }).click();
		await (await sidePanelView(wallet)).click('Connect');
		await pageB.waitForFunction(
			() => JSON.parse(document.getElementById('out')!.textContent!).results.eth_requestAccounts
		);
		expect((await readState(pageB)).results.eth_requestAccounts.result).toEqual([two.address]);
		expect(await accountsOf(pageA)).toEqual([two.address]);
		await snapshotSays(wallet, two.address);
		await context.close();
	});
});

declare global {
	interface Window {
		__ask(method: string, params?: unknown[]): Promise<AskResult>;
	}
}
