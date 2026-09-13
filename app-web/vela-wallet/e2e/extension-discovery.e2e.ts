/**
 * A dApp finds Vela, and can be refused (spec 027 T324 — US1's plumbing).
 *
 * Everything here happens in a real page, through the real channel: the MAIN
 * world provider, the isolated-world bridge, the service worker, and a window
 * this extension owns. What Phase 3 delivers is the whole PATH plus one
 * complete answer — a refusal. Granting is `dapp_permissions`' ruling (Phase 4)
 * and signing is 026's sheet (Phase 5); neither is faked here.
 *
 * The refusal is the half worth landing first. A wallet that cannot say no is
 * not safe to install, and the failure mode of an extension wallet is silence:
 * a dApp promise that never settles leaves a person unable to tell whether
 * their money moved. Both ways of saying no are asserted — the button, and
 * simply closing the window.
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
const DAPP_PORT = 8811;

/** A page that is only a dApp: it announces nothing and decides nothing. */
function serveTestDApp(): Promise<Server> {
	const html = readFileSync(join(APP_ROOT, 'e2e/testdapp/index.html'));
	const server = createServer((_req, res) => {
		res.setHeader('content-type', 'text/html; charset=utf-8');
		res.end(html);
	});
	return new Promise((resolve) => server.listen(DAPP_PORT, () => resolve(server)));
}

interface AskResult {
	ok: boolean;
	code?: number;
	message?: string;
	result?: unknown;
}

test.describe('the injected provider', () => {
	test.skip(!extensionBuilt(), 'extension/dist is missing — run `pnpm build:extension`');
	test.setTimeout(120_000);

	let server: Server;
	test.beforeAll(async () => {
		server = await serveTestDApp();
	});
	test.afterAll(async () => {
		await new Promise((resolve) => server.close(() => resolve(null)));
	});

	test('announces Vela with its real identity, and answers to the legacy name too', async () => {
		const context = await loadExtension();
		const page = await context.newPage();
		await page.goto(`http://localhost:${DAPP_PORT}/`);

		const seen = await page.evaluate(
			() =>
				JSON.parse(document.getElementById('out')!.textContent!) as {
					announced: {
						name: string;
						rdns: string;
						iconScheme: string;
						sameAsWindowEthereum: boolean;
						frozenInfo: boolean;
					}[];
					legacy: { isMetaMask: boolean; isVela: boolean; hasRequest: boolean } | null;
				}
		);

		// EIP-6963 states the truth: this is Vela, whatever the legacy flag says.
		expect(seen.announced).toHaveLength(1);
		expect(seen.announced[0].name).toBe('Vela Wallet');
		expect(seen.announced[0].rdns).toBe('app.getvela');
		// A data URI, not a URL: a remote icon would leak every dApp visit. SVG —
		// the canonical mark from docs/design/icon/app-icon.svg, crisp at any size.
		expect(seen.announced[0].iconScheme).toContain('data:image/svg');
		expect(seen.announced[0].sameAsWindowEthereum).toBe(true);
		expect(seen.announced[0].frozenInfo).toBe(true);

		// And the many dApps that only ever look for one wallet find one.
		expect(seen.legacy?.isMetaMask).toBe(true);
		expect(seen.legacy?.isVela).toBe(true);
		expect(seen.legacy?.hasRequest).toBe(true);
		await context.close();
	});

	test('a connect request opens a window that names the SITE, not its claims', async () => {
		const context = await loadExtension();
		const page = await context.newPage();
		await page.goto(`http://localhost:${DAPP_PORT}/`);

		const asked = page.evaluate(
			() => window.__ask('eth_requestAccounts') as Promise<AskResult>
		) as Promise<AskResult>;
		const win = await requestWindow(context);

		// The origin is the browser's fact, carried across the boundary. A page
		// cannot rename itself into someone more trustworthy.
		await expect(win.getByRole('heading')).toContainText(`localhost:${DAPP_PORT}`);
		await expect(win.getByText('eth_requestAccounts')).toBeVisible();

		// Refusing answers the dApp — in the standard shape, and exactly once.
		await win.getByRole('button', { name: 'Cancel' }).click();
		const answer = await asked;
		expect(answer.ok).toBe(false);
		expect(answer.code).toBe(4001);
		await context.close();
	});

	test('closing the window without deciding settles 4900, never 4001', async () => {
		const context = await loadExtension();
		const page = await context.newPage();
		await page.goto(`http://localhost:${DAPP_PORT}/`);

		const asked = page.evaluate(
			() => window.__ask('eth_requestAccounts') as Promise<AskResult>
		) as Promise<AskResult>;
		const win = await requestWindow(context);
		await win.close();

		// The dApp is told, rather than left waiting — but NOT with 4001. That
		// code means "the user said no, nothing happened", and a dApp that reads
		// it re-sends; if the request had reached the bundler, the re-send is a
		// double spend. The code is `dapp_permissions`', asked rather than
		// restated (spec 027 D43), and `instant.test.ts` pins the worker's
		// backstop against the core's own answer.
		const answer = await asked;
		expect(answer.ok).toBe(false);
		expect(answer.code).not.toBe(4001);
		expect(answer.code).toBe(4900);
		await context.close();
	});

	test('refuses what it will not do, and asks for nothing it cannot answer', async () => {
		const context = await loadExtension();
		const page = await context.newPage();
		await page.goto(`http://localhost:${DAPP_PORT}/`);

		// `eth_sign` signs an opaque digest — refused by policy, not by a window.
		const signed = (await page.evaluate(() =>
			window.__ask('eth_sign', ['0x00', '0x00'])
		)) as AskResult;
		expect(signed.code).toBe(4200);

		// And a state read from an ungranted origin opens no window. In this
		// context no wallet has ever been opened, so there is no published
		// snapshot and the honest answer is that Vela cannot say yet — not an
		// invented chain id.
		const chain = (await page.evaluate(() => window.__ask('eth_chainId'))) as AskResult;
		expect(chain.code).toBe(4100);
		const accounts = (await page.evaluate(() => window.__ask('eth_accounts'))) as AskResult;
		expect(accounts.result).toEqual([]);
		expect(requestWindowOpen(context)).toBe(false);
		await context.close();
	});
});

declare global {
	interface Window {
		__ask(method: string, params?: unknown[]): Promise<AskResult>;
	}
}
