/**
 * A dApp connects, and stays connected (spec 027 T336 — SC-301, SC-302).
 *
 * The whole doorway, on the real machines: the site asks, `dapp_permissions`
 * rules, a person decides, the core authors the grant and the answer, and the
 * next question is answered from what the core published — with no window and
 * no second decision.
 *
 * The wallet is seeded through the parallel space, so the account is the
 * fixture keyset's and the address is the core's own derivation.
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
const PORT = 8812;
/** Fixture wallet #1 — derived by the core, never read from storage. */
const FIXTURE_ONE = '0xD400866e00B055B20752a826CD5C89b811de130b';

/** Both fixture dApps on one origin: the modern one, and the legacy one. */
function serveDApps(): Promise<Server> {
	const modern = readFileSync(join(APP_ROOT, 'e2e/testdapp/index.html'));
	const legacy = readFileSync(join(APP_ROOT, 'e2e/testdapp/legacy.html'));
	const server = createServer((req, res) => {
		res.setHeader('content-type', 'text/html; charset=utf-8');
		res.end(req.url?.includes('legacy') ? legacy : modern);
	});
	return new Promise((resolve) => server.listen(PORT, () => resolve(server)));
}

interface AskResult {
	ok: boolean;
	code?: number;
	result?: unknown;
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
	// The wallet publishes what a connected site may be told; wait for it rather
	// than for a duration.
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

test.describe('connecting a dApp', () => {
	test.skip(!extensionBuilt(), 'extension/dist is missing — run `pnpm build:extension`');
	test.setTimeout(150_000);

	let server: Server;
	test.beforeAll(async () => {
		server = await serveDApps();
	});
	test.afterAll(async () => {
		// Keep-alive sockets otherwise hold the server open past the hook's
		// deadline — the close hangs, and the whole file is reported failed.
		server.closeAllConnections();
		await new Promise((resolve) => server.close(() => resolve(null)));
	});

	test('SC-301: asks, is granted, and stays granted without a second window', async () => {
		const context = await loadExtension();
		const id = extensionId();
		await seedWallet(context, id);

		const page = await context.newPage();
		await page.goto(`http://localhost:${PORT}/`);

		// Before any grant the honest answer is an empty list — a disconnected
		// wallet, with no prompt. EIP-1193 asks for exactly this.
		expect(((await page.evaluate(() => window.__ask('eth_accounts'))) as AskResult).result).toEqual(
			[]
		);

		const asked = page.evaluate(() => window.__ask('eth_requestAccounts')) as Promise<AskResult>;
		const win = await requestWindow(context);
		await expect(win.getByRole('heading')).toContainText(`localhost:${PORT}`);
		await win.getByRole('button', { name: 'Connect' }).click();
		// The window closes on a short delay; the assertions below are about
		// there being NO second window, so wait for the first one to be gone.
		await noRequestWindow(context);

		// The address the dApp receives is the GRANT's, which is the core's
		// derivation — never a stored field.
		const answer = await asked;
		expect(answer.ok).toBe(true);
		expect(answer.result).toEqual([FIXTURE_ONE]);

		// And the next question is answered from what the core published: no
		// window, no second decision, same address.
		const again = (await page.evaluate(() => window.__ask('eth_accounts'))) as AskResult;
		expect(again.result).toEqual([FIXTURE_ONE]);
		expect(context.pages().some((p) => p.url().includes('/request.html'))).toBe(false);

		// The permission is visible in the shape EIP-2255 asks for.
		const perms = (await page.evaluate(() => window.__ask('wallet_getPermissions'))) as AskResult;
		expect(perms.result).toEqual([{ parentCapability: 'eth_accounts' }]);
		await context.close();
	});

	/**
	 * Spec 089: a connected site asking AGAIN — `eth_requestAccounts` on every
	 * load, a second Connect click — used to open a request window that only
	 * asked the core and closed: a window flashed open and shut and took the
	 * focus each time. It is answered by the worker now, and nothing opens.
	 */
	test('a connected site asking again is answered at once, and no window opens', async () => {
		const context = await loadExtension();
		await seedWallet(context, extensionId());
		const page = await context.newPage();
		await page.goto(`http://localhost:${PORT}/`);
		const asked = page.evaluate(() => window.__ask('eth_requestAccounts')) as Promise<AskResult>;
		const win = await requestWindow(context);
		await win.getByRole('button', { name: 'Connect' }).click();
		expect((await asked).result).toEqual([FIXTURE_ONE]);
		await noRequestWindow(context);

		const opened: string[] = [];
		context.on('page', (p) => opened.push(p.url()));
		const again = (await page.evaluate(() => window.__ask('eth_requestAccounts'))) as AskResult;
		const perms = (await page.evaluate(() =>
			window.__ask('wallet_requestPermissions', [{ eth_accounts: {} }])
		)) as AskResult;
		await page.waitForTimeout(1_500);
		expect(again).toEqual({ ok: true, result: [FIXTURE_ONE] });
		expect(perms).toEqual({ ok: true, result: [{ parentCapability: 'eth_accounts' }] });
		expect(opened).toEqual([]);
		await context.close();
	});

	test('SC-302: a dApp that only knows one wallet can still connect', async () => {
		const context = await loadExtension();
		const id = extensionId();
		await seedWallet(context, id);

		const page = await context.newPage();
		await page.goto(`http://localhost:${PORT}/legacy`);

		// It found a provider, and its own gate opened.
		const seen = await page.evaluate(
			() => JSON.parse(document.getElementById('out')!.textContent!) as Record<string, boolean>
		);
		expect(seen.found).toBe(true);
		expect(seen.gated).toBe(true);

		const asked = page.evaluate(() => window.__ask('eth_requestAccounts')) as Promise<AskResult>;
		const win = await requestWindow(context);
		await win.getByRole('button', { name: 'Connect' }).click();
		const answer = await asked;
		expect(answer.ok).toBe(true);
		expect(answer.result).toEqual([FIXTURE_ONE]);
		await context.close();
	});

	test('a refused connection grants nothing', async () => {
		const context = await loadExtension();
		const id = extensionId();
		await seedWallet(context, id);

		const page = await context.newPage();
		await page.goto(`http://localhost:${PORT}/`);
		const asked = page.evaluate(() => window.__ask('eth_requestAccounts')) as Promise<AskResult>;
		const win = await requestWindow(context);
		await win.getByRole('button', { name: 'Cancel' }).click();

		expect((await asked).code).toBe(4001);
		// Nothing was written: the site is exactly as unknown as before.
		const after = (await page.evaluate(() => window.__ask('eth_accounts'))) as AskResult;
		expect(after.result).toEqual([]);
		await context.close();
	});
});

declare global {
	interface Window {
		__ask(method: string, params?: unknown[]): Promise<AskResult>;
	}
}
