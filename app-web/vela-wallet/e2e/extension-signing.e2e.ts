/**
 * A dApp asks for a signature, and gets 026's sheet (spec 027 T343 — SC-303/304).
 *
 * No second signing path exists. The request reaches `sign_request` — the same
 * app-resident machine the wallet's own screens use — and the same drawn sheet
 * renders it: the same clear-signing reading, the same never-unlimited guard,
 * the same fee policy. What 027 adds is the transport that delivered it and the
 * window it renders in.
 *
 * The order matters here too: a signature can only be ASKED for by an origin
 * the core has already granted, so every test connects first.
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
	slideToConfirm
} from './extension-helpers';

const APP_ROOT = join(import.meta.dirname, '..');
const PORT = 8813;
const FIXTURE_ONE = '0xD400866e00B055B20752a826CD5C89b811de130b';
/** "Hello, Vela" as a hex string, the way a dApp sends `personal_sign`. */
const MESSAGE_HEX = '0x48656c6c6f2c2056656c61';

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

async function seedWallet(context: BrowserContext, id: string): Promise<void> {
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
}

/** Connect the dApp, so a signature is something it is allowed to ask for. */
async function connect(context: BrowserContext, page: Page): Promise<void> {
	const asked = page.evaluate(() => window.__ask('eth_requestAccounts')) as Promise<AskResult>;
	const win = await requestWindow(context);
	await win.getByRole('button', { name: 'Connect' }).click();
	expect((await asked).result).toEqual([FIXTURE_ONE]);
	// A settled window closes on a short delay; the next request must not be
	// able to find this one still on screen.
	await noRequestWindow(context);
}

test.describe('signing for a dApp', () => {
	test.skip(!extensionBuilt(), 'extension/dist is missing — run `pnpm build:extension`');
	test.setTimeout(180_000);

	let server: Server;
	test.beforeAll(async () => {
		server = await serveDApp();
	});
	test.afterAll(async () => {
		// Keep-alive sockets otherwise hold the server open past the hook's
		// deadline — the close hangs, and the whole file is reported failed.
		server.closeAllConnections();
		await new Promise((resolve) => server.close(() => resolve(null)));
	});

	test('SC-303: a message request renders 026’s sheet, with its own words', async () => {
		const context = await loadExtension();
		const id = extensionId();
		await seedWallet(context, id);
		const page = await context.newPage();
		await page.goto(`http://localhost:${PORT}/`);
		await connect(context, page);

		// Never settles in this test — the dApp is still waiting when the context
		// closes — so its rejection is swallowed rather than reported as a
		// failure of the assertions below, which is what an unhandled floating
		// promise does here.
		page
			.evaluate(([message, address]) => window.__ask('personal_sign', [message, address]), [
				MESSAGE_HEX,
				FIXTURE_ONE
			] as const)
			.catch(() => {});
		const win = await requestWindow(context);

		// The sheet says what is being signed, decoded — not the hex the dApp
		// sent — and it names the origin and the signing account.
		await expect(win.getByText('Hello, Vela')).toBeVisible({ timeout: 30_000 });
		await expect(win.getByText(`localhost:${PORT}`).first()).toBeVisible();
		await expect(win.getByText('Parallel One')).toBeVisible();
		// An off-chain signature costs nothing, and the sheet says so rather than
		// showing an empty fee row.
		await expect(win.getByText(/No network fee/)).toBeVisible();

		// And no template placeholder reaches a person's eyes.
		const text = await win.evaluate(() => document.body.innerText);
		expect(text).not.toContain('{{');
		await context.close();
	});

	test('the sheet is the ONLY signing path: dismissing it refuses', async () => {
		const context = await loadExtension();
		const id = extensionId();
		await seedWallet(context, id);
		const page = await context.newPage();
		await page.goto(`http://localhost:${PORT}/`);
		await connect(context, page);

		const asked = page.evaluate(
			([message, address]) => window.__ask('personal_sign', [message, address]),
			[MESSAGE_HEX, FIXTURE_ONE] as const
		) as Promise<AskResult>;
		const win = await requestWindow(context);
		await expect(win.getByText('Hello, Vela')).toBeVisible({ timeout: 30_000 });
		await win.close();

		// Torn down with an answer owed: 4900, never 4001 — the code that keeps a
		// dApp from re-sending an operation that may already be at the bundler.
		const answer = await asked;
		expect(answer.ok).toBe(false);
		expect(answer.code).not.toBe(4001);
		await context.close();
	});

	/**
	 * SC-304, met in 028 Phase 8 — and the note of why it was not, kept so the
	 * fix is legible.
	 *
	 * 027 measured: the slide committed, `approve_tapped` reached the resident,
	 * the confirm gate was open, and `navigator.credentials.get` was never
	 * called. The cause was not in the sheet or the transport: the web resident
	 * never dispatched `accounts_changed`, so the machine had NO signer rows,
	 * and `approve_with` (`sign_request.rs`) returned `Command::done()` at
	 * `model.accounts.get(active_index)` — silently, by design of that guard.
	 * Expo's resident fed the rows from the wallet provider (`setSignAccounts`);
	 * the web resident now mirrors the session's rows on boot, on every change
	 * and inside the account-switch ack (`syncAccounts`).
	 */
	test('SC-304: approving returns a signature the account’s own key made', async () => {
		const context = await loadExtension();
		const id = extensionId();
		await seedWallet(context, id);
		const page = await context.newPage();
		await page.goto(`http://localhost:${PORT}/`);
		await connect(context, page);

		const asked = page.evaluate(
			([message, address]) => window.__ask('personal_sign', [message, address]),
			[MESSAGE_HEX, FIXTURE_ONE] as const
		) as Promise<AskResult>;
		const win = await requestWindow(context);
		await expect(win.getByText('Hello, Vela')).toBeVisible({ timeout: 30_000 });

		// A deliberate movement, not a tap — the control commits at 88% of its
		// own travel. In the parallel space the passkey it then asks for is the
		// fixture keyset, so this completes headlessly while remaining the REAL
		// ceremony: the same assertion builder, verified by the same core.
		await slideToConfirm(win);

		const answer = await asked;
		expect(answer.ok).toBe(true);
		// An EIP-191 signature, not an empty acknowledgement.
		expect(typeof answer.result).toBe('string');
		expect(String(answer.result)).toMatch(/^0x[0-9a-fA-F]{100,}$/);
		await context.close();
	});

	test('an ungranted origin cannot ask for a signature at all', async () => {
		const context = await loadExtension();
		const id = extensionId();
		await seedWallet(context, id);
		const page = await context.newPage();
		await page.goto(`http://localhost:${PORT}/`);

		// No connect first. The core refuses before any sheet exists — the
		// never-connected rule, and it is the core's, not the window's.
		const answer = (await page.evaluate(
			([message, address]) => window.__ask('personal_sign', [message, address]),
			[MESSAGE_HEX, FIXTURE_ONE] as const
		)) as AskResult;
		expect(answer.ok).toBe(false);
		expect(answer.code).toBe(4100);
		await context.close();
	});
});

declare global {
	interface Window {
		__ask(method: string, params?: unknown[]): Promise<AskResult>;
	}
}
