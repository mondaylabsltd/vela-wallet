/**
 * Seeing and cutting off a connected site (spec 027 T353 — SC-305, SC-307).
 *
 * A grant is a standing permission. A wallet that can grant but not revoke is a
 * wallet that only ever gets more permissive, so the list and the way out are
 * asserted together — and so is the thing that makes revocation mean anything:
 * the site's NEXT request is treated as a first one.
 *
 * The resilience half is here too, because it is the same promise from the
 * other side: a request is written down the moment it arrives, and it is gone
 * the moment it is answered. Nothing is left owing, and nothing lingers that
 * nobody can answer.
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
	requestWindow
} from './extension-helpers';

const APP_ROOT = join(import.meta.dirname, '..');
const PORT = 8814;
/**
 * The extension's own windows are phone-width, and Settings draws a DIFFERENT
 * layout above the desktop breakpoint — where "Advanced" is not a row to tap.
 * A default 1280 context therefore tests a surface no extension user ever sees.
 */
const PHONE = { viewport: { width: 420, height: 820 } };
const FIXTURE_ONE = '0xD400866e00B055B20752a826CD5C89b811de130b';

function serveDApp(): Promise<Server> {
	const html = readFileSync(join(APP_ROOT, 'e2e/testdapp/index.html'));
	const server = createServer((_req, res) => {
		res.setHeader('content-type', 'text/html; charset=utf-8');
		res.end(html);
	});
	return new Promise((resolve) => server.listen(PORT, () => resolve(server)));
}

interface AskResult {
	ok: boolean;
	code?: number;
	result?: unknown;
}

async function openWallet(context: BrowserContext, id: string): Promise<Page> {
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
				window as unknown as { chrome: { storage: { local: { get(k: string): Promise<object> } } } }
			).chrome.storage.local
				.get('vela.ext.cache')
				.then((all: Record<string, unknown>) => all['vela.ext.cache'] !== undefined),
		null,
		{ timeout: 30_000 }
	);
	return page;
}

/** Whatever the extension has written down, from a page of its own origin. */
function storage(page: Page, prefix: string): Promise<string[]> {
	return page.evaluate(
		(p) =>
			(
				window as unknown as { chrome: { storage: { local: { get(k: null): Promise<object> } } } }
			).chrome.storage.local
				.get(null)
				.then((all: Record<string, unknown>) => Object.keys(all).filter((k) => k.startsWith(p))),
		prefix
	);
}

test.describe('connections', () => {
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

	test('SC-305: a connected site is listed, revoked, and must ask again', async () => {
		const context = await loadExtension(PHONE);
		const id = extensionId();
		const wallet = await openWallet(context, id);

		const page = await context.newPage();
		await page.goto(`http://localhost:${PORT}/`);
		const asked = page.evaluate(() => window.__ask('eth_requestAccounts')) as Promise<AskResult>;
		const win = await requestWindow(context);
		await win.getByRole('button', { name: 'Connect' }).click();
		expect((await asked).result).toEqual([FIXTURE_ONE]);
		await noRequestWindow(context);

		// The site is LISTED — by its host, with the account it holds — where 023
		// drew the connections group.
		// Where 023 drew it: Settings → Advanced → Device storage → Connections.
		await wallet.goto(`chrome-extension://${id}/en/settings.html`);
		await wallet.getByText('Advanced', { exact: true }).first().click();
		await wallet.getByText('Device storage', { exact: true }).first().click();
		const row = wallet.getByText(`localhost:${PORT}`, { exact: true });
		await expect(row).toBeVisible({ timeout: 20_000 });
		// Its own account rides beside it — a grant is an address, not just a name.
		await expect(wallet.getByText('0xD40086…de130b')).toBeVisible();

		// And it can be cut off. The row says "Disconnect", singular: it cuts off
		// this one site, and a row that disconnects one must not be labelled with
		// the words for all of them.
		await wallet.getByRole('button', { name: 'Disconnect', exact: true }).first().click();
		// A destructive row asks first, in a sheet titled with the site; the
		// sheet's own Disconnect is the act.
		const sheet = wallet.locator('[role=dialog], dialog[open]');
		await expect(sheet).toContainText(`localhost:${PORT}`);
		await sheet.getByRole('button', { name: 'Disconnect', exact: true }).click();
		await expect(row).toHaveCount(0, { timeout: 10_000 });

		// Which is what makes revocation mean something: the site's next request
		// is a first request again — it must ask, not resume.
		const after = (await page.evaluate(() => window.__ask('eth_accounts'))) as AskResult;
		expect(after.result).toEqual([]);
		await context.close();
	});

	test('SC-307: a request is written down on arrival and gone once answered', async () => {
		const context = await loadExtension(PHONE);
		const id = extensionId();
		const wallet = await openWallet(context, id);

		const page = await context.newPage();
		await page.goto(`http://localhost:${PORT}/`);
		const asked = page.evaluate(() => window.__ask('eth_requestAccounts')) as Promise<AskResult>;
		const win = await requestWindow(context);

		// Durable while it is owed: this is what survives an evicted worker, and
		// the reason a torn-down window can still be settled at all.
		expect(await storage(wallet, 'vela.req.')).toHaveLength(1);

		await win.getByRole('button', { name: 'Cancel' }).click();
		expect((await asked).code).toBe(4001);

		// And gone once it is not owed. A record nobody can answer must not
		// outlive its request, or a later window could open on a dead one.
		await expect.poll(() => storage(wallet, 'vela.req.'), { timeout: 10_000 }).toHaveLength(0);
		await context.close();
	});
});

declare global {
	interface Window {
		__ask(method: string, params?: unknown[]): Promise<AskResult>;
	}
}
