/**
 * The package uploaded to the Chrome Web Store, installed (spec 094).
 *
 * Every other extension suite loads the DEVELOPMENT package: it keeps the
 * manifest `key` (the id they compute) and the parallel space (the fixture
 * wallet they enter through). The store package has neither — the store
 * assigns the id, and release builds carry no developer features — so this is
 * the one suite that loads exactly what a reviewer installs, finds its id the
 * way Chrome assigned it, and checks what such a person meets first:
 *
 *   - the welcome opens in a tab on install, and says to reload the tabs that
 *     were already open (S3);
 *   - the wasm the app names is in the package (B2) and the core runs;
 *   - the developer pages are not there;
 *   - a site gets the provider, announced under its own name, and nothing in
 *     its console (the NICE item);
 *   - the EIP-5792 status methods answer instead of 4200 (S6).
 */
import { createServer, type Server } from 'node:http';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test, type BrowserContext } from '@playwright/test';
import {
	loadExtension,
	runningExtensionId,
	STORE_DIST,
	storeBuilt,
	welcomeTab
} from './extension-helpers';

const APP_ROOT = join(import.meta.dirname, '..');
const PORT = 8840;

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

test.describe('the Chrome Web Store package, installed', () => {
	test.skip(!storeBuilt(), 'extension/dist-store is missing — run `pnpm build:extension`');
	test.setTimeout(120_000);

	let server: Server;
	test.beforeAll(async () => {
		server = await serveDApp();
	});
	test.afterAll(async () => {
		server.closeAllConnections();
		await new Promise((resolve) => server.close(() => resolve(null)));
	});

	/** A fresh browser with the store package installed, its welcome tab left open. */
	async function install(): Promise<{ context: BrowserContext; id: string }> {
		const context = await loadExtension({ dist: STORE_DIST, keepWelcome: true });
		return { context, id: await runningExtensionId(context) };
	}

	test('carries no key, so Chrome assigns the id — and no developer pages', async () => {
		const manifest = JSON.parse(readFileSync(join(STORE_DIST, 'manifest.json'), 'utf8'));
		expect(manifest.key).toBeUndefined();
		const { context, id } = await install();
		expect(id).toMatch(/^[a-p]{32}$/);
		const page = await context.newPage();
		await page.goto(`chrome-extension://${id}/manifest.json`);
		const served = await page.evaluate(async () => {
			const status = async (path: string) => {
				try {
					return (await fetch(`/${path}`)).ok ? 'served' : 'missing';
				} catch {
					return 'missing';
				}
			};
			return {
				parallel: await status('en/parallel.html'),
				gallery: await status('en/gallery.html'),
				wallet: await status('en/wallet.html')
			};
		});
		expect(served).toEqual({ parallel: 'missing', gallery: 'missing', wallet: 'served' });
		await context.close();
	});

	test('opens the welcome on install, and the core it names is there and runs', async () => {
		const { context, id } = await install();
		const welcome = await welcomeTab(context);
		const failures: string[] = [];
		welcome.on('pageerror', (error) => failures.push(String(error).slice(0, 200)));
		// No wallet yet: the doorway → the wallet → its welcome, which opens
		// on the first-run intro.
		await welcome.waitForURL(/\/en\.html$/, { timeout: 30_000 });
		await expect(welcome.getByRole('button', { name: 'Continue' })).toBeVisible({
			timeout: 30_000
		});

		// The core the app names (`WASM_URL`), fetched and compiled where the
		// app runs — B2's failure was a package that named a wasm it did not
		// carry, and every page then failed to decide anything.
		const wasmUrl = /export const WASM_URL = '([^']+)'/.exec(
			readFileSync(join(APP_ROOT, '../../rust/pkg-web/vela_core_wasm_url.js'), 'utf8')
		)![1];
		const compiled = await welcome.evaluate(async (url) => {
			const response = await fetch(url);
			if (!response.ok) return `HTTP ${response.status}`;
			const module = await WebAssembly.compile(await response.arrayBuffer());
			return WebAssembly.Module.exports(module).length > 0 ? 'compiled' : 'empty';
		}, wasmUrl);
		expect(compiled).toBe('compiled');
		expect(failures).toEqual([]);
		expect(new URL(welcome.url()).host).toBe(id);
		await context.close();
	});

	test('a site gets the provider under its own name, and nothing in its console', async () => {
		const { context } = await install();
		const page = await context.newPage();
		const logged: string[] = [];
		page.on('console', (message) => logged.push(message.text()));
		await page.goto(`http://localhost:${PORT}/`);
		await page.waitForFunction(
			() => (window as unknown as { __velaProvider?: unknown }).__velaProvider
		);
		const state = await page.evaluate(() =>
			JSON.parse(document.getElementById('out')!.textContent!)
		);
		expect(state.announced).toHaveLength(1);
		expect(state.announced[0]).toMatchObject({ name: 'Vela Wallet', rdns: 'app.getvela' });
		expect(state.legacy).toMatchObject({ isVela: true, hasRequest: true });
		// Let the provider's warm-up reads come back before reading the console.
		await page.evaluate(() => window.ethereum.request({ method: 'eth_chainId' }).catch(() => null));
		expect(logged.filter((line) => line.includes('[Vela]'))).toEqual([]);
		await context.close();
	});

	test('EIP-5792: an unknown batch is an unknown bundle, and capabilities need a connection', async () => {
		const { context } = await install();
		const page = await context.newPage();
		await page.goto(`http://localhost:${PORT}/`);
		await page.waitForFunction(
			() => (window as unknown as { __velaProvider?: unknown }).__velaProvider
		);
		// Answered, not refused as unsupported (4200, before 094): a batch this
		// wallet never sent is EIP-5792's unknown bundle…
		const status = (await page.evaluate(() =>
			window.__ask('wallet_getCallsStatus', [`0x${'ee'.repeat(32)}`])
		)) as AskResult;
		expect(status).toMatchObject({ ok: false, code: 5730 });
		// …and a site nobody connected is told nothing about an account.
		const capabilities = (await page.evaluate(() =>
			window.__ask('wallet_getCapabilities', [`0x${'a1'.repeat(20)}`])
		)) as AskResult;
		expect(capabilities).toMatchObject({ ok: false, code: 4100 });
		await context.close();
	});

	test('limited site access is said in plain words, with the one-click grant (S2)', async () => {
		const { context, id } = await install();
		const page = await context.newPage();
		// Chrome's own answer when a person set "Site access: On click": the
		// host permissions are withheld, until the extension asks and they agree.
		await page.addInitScript(() => {
			const permissions = (
				window as unknown as {
					chrome?: {
						permissions?: {
							contains: () => Promise<boolean>;
							request: () => Promise<boolean>;
						};
					};
				}
			).chrome?.permissions;
			if (!permissions) return;
			let granted = false;
			permissions.contains = async () => granted;
			permissions.request = async () => {
				(window as unknown as { __askedForAccess: boolean }).__askedForAccess = true;
				granted = true;
				return true;
			};
		});
		await page.goto(`chrome-extension://${id}/en.html`);
		const notice = page.getByTestId('ext-site-access');
		await expect(notice).toBeVisible({ timeout: 30_000 });
		await expect(notice).toContainText('Site access is limited');
		await notice.getByRole('button', { name: 'Allow access' }).click();
		await expect(notice).toBeHidden({ timeout: 10_000 });
		expect(
			await page.evaluate(
				() => (window as unknown as { __askedForAccess?: boolean }).__askedForAccess
			)
		).toBe(true);
		await context.close();
	});

	test('the welcome says to reload the tabs that were open before the install (S3)', async () => {
		const { context } = await install();
		const welcome = await welcomeTab(context);
		await welcome.waitForURL(/\/en\.html$/, { timeout: 30_000 });
		// No web page was open in this fresh browser, so the worker said nothing
		// (the rule is `background.test.ts`'s); the mark it leaves is what the
		// welcome reads.
		await expect(welcome.getByTestId('ext-installed')).toBeHidden();
		await welcome.evaluate(() => {
			sessionStorage.setItem('vela.ext.installed', '1');
			localStorage.setItem('vela.intro.seen', String(Date.now()));
		});
		await welcome.reload();
		const note = welcome.getByTestId('ext-installed');
		await expect(note).toBeVisible({ timeout: 30_000 });
		await expect(note).toContainText('Reload tabs that were already open');
		await note.getByRole('button', { name: 'Got it' }).click();
		await expect(note).toBeHidden();
		await context.close();
	});
});
