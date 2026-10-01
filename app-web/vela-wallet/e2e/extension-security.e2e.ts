/**
 * What a hostile page cannot do (spec 027 T362).
 *
 * The channel contract makes seven promises. Five were already asserted by the
 * suites that needed them; these are the two that were only ever true because
 * the code said so, and they are the two a page can actually attack:
 *
 *   1. **the origin is the browser's fact.** A page can post anything it likes
 *      onto its own window. It must not be able to rename itself into somebody
 *      more trustworthy, because the origin is what a grant is keyed on.
 *   2. **one answer, once.** A repeated request id must not open a second
 *      window or produce a second answer — an operation answered twice is an
 *      operation a dApp may act on twice.
 */
import { createServer, type Server } from 'node:http';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from '@playwright/test';
import {
	extensionBuilt,
	loadExtension,
	requestWindow,
	requestWindowOpen
} from './extension-helpers';

const APP_ROOT = join(import.meta.dirname, '..');
const PORT = 8815;

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
}

test.describe('what a page cannot do', () => {
	test.skip(!extensionBuilt(), 'extension/dist is missing — run `pnpm build:extension`');
	test.setTimeout(150_000);

	let server: Server;
	test.beforeAll(async () => {
		server = await serveDApp();
	});
	test.afterAll(async () => {
		server.closeAllConnections();
		await new Promise((resolve) => server.close(() => resolve(null)));
	});

	test('cannot rename itself into a more trustworthy origin', async () => {
		const context = await loadExtension();
		const page = await context.newPage();
		await page.goto(`http://localhost:${PORT}/`);

		// The page claims to be somewhere it is not, on the real channel.
		await page.evaluate(() => window.__forge('https://app.uniswap.org'));
		const win = await requestWindow(context);

		// The window names the REAL origin. `sender.origin` is added on the far
		// side of the message boundary, by the browser, and the page's own claim
		// never reaches a grant.
		await expect(win.getByRole('heading')).toContainText(`localhost:${PORT}`);
		await expect(win.getByRole('heading')).not.toContainText('uniswap');
		await context.close();
	});

	test('cannot get one request answered twice', async () => {
		const context = await loadExtension();
		const page = await context.newPage();
		await page.goto(`http://localhost:${PORT}/`);

		// The first one opens a window and waits there.
		await page.evaluate(() => window.__forge(window.location.origin));
		await requestWindow(context);
		const windowsAfterFirst = context.pages().filter((p) => p.url().includes('/request.html'));
		expect(windowsAfterFirst).toHaveLength(1);

		// The same id again. It must not open a second window: a request that can
		// be answered twice is an operation a dApp may act on twice.
		await page.evaluate(() => window.__forge(window.location.origin));
		await page.waitForTimeout(2000);
		expect(context.pages().filter((p) => p.url().includes('/request.html'))).toHaveLength(1);
		await context.close();
	});

	/**
	 * Spec 089: `storage.local` holds each site's grant, the signed-in address
	 * and the RPC catalog. content.js never reads it, but Chrome opens it to
	 * content scripts by default — and a content script runs in the renderer of
	 * every page. The worker closes it to them at start; the isolated world
	 * itself is asked here, over CDP.
	 */
	test('its content script cannot read the wallet’s storage', async () => {
		const context = await loadExtension();
		const page = await context.newPage();
		const cdp = await context.newCDPSession(page);
		const contexts: { id: number; name: string; auxData?: { type?: string } }[] = [];
		cdp.on('Runtime.executionContextCreated', (event) => contexts.push(event.context));
		await cdp.send('Runtime.enable');
		await page.goto(`http://localhost:${PORT}/`);
		await expect
			.poll(() => contexts.some((c) => c.auxData?.type === 'isolated'), { timeout: 10_000 })
			.toBe(true);
		const isolated = contexts.filter((c) => c.auxData?.type === 'isolated').at(-1)!;
		const { result } = await cdp.send('Runtime.evaluate', {
			contextId: isolated.id,
			awaitPromise: true,
			returnByValue: true,
			expression: `chrome.storage.local.get(null).then(() => 'read', (e) => 'refused: ' + e.message)`
		});
		expect(String(result.value)).toMatch(/^refused/);
		await context.close();
	});

	test('cannot use the wallet as an open RPC relay', async () => {
		const context = await loadExtension();
		const page = await context.newPage();
		await page.goto(`http://localhost:${PORT}/`);

		// `eth_signTransaction` is NOT caught by the signing predicate, which is
		// exactly why the router is an allowlist: a catch-all "read" bucket would
		// have forwarded it to a public node on any site's behalf.
		const signed = (await page.evaluate(() =>
			window.__ask('eth_signTransaction', [{}])
		)) as AskResult;
		expect(signed.ok).toBe(false);
		expect(signed.code).toBe(4200);
		expect(requestWindowOpen(context)).toBe(false);
		await context.close();
	});
});

declare global {
	interface Window {
		__ask(method: string, params?: unknown[]): Promise<AskResult>;
		__forge(origin: string): void;
	}
}
