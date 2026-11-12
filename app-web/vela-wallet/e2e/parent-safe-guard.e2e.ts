/**
 * VELA-2026-002 (the 0.9.9 hotfix) — a site never makes this account act for
 * a Safe it owns, at every dApp door the web has (the parent-Safe guard;
 * disclosure 2026-09-29, APPENDIX-PARENT).
 *
 * The attack: a site connected to a Vela account asks it to call
 * `execTransaction` on a Safe the account is an owner of (a Safe made outside
 * Vela), with the "approved by msg.sender" signature (v = 1) — for a 1-of-N
 * Safe, an immediate takeover (`swapOwner` to the attacker). The core reads
 * `isOwner(account)` on that Safe before anything can be signed; read true,
 * the sheet opens refused in the catalog's words, with no confirm, and
 * closing it answers the site -32603 `parent_safe_blocked`. A read nobody
 * could make refuses too ("couldn't check"); read false, the request goes on
 * as before.
 *
 * The request is the vector's `hotfix exec-swap` case
 * (`tests/vectors/rules/parent-safe-guard.json`), re-aimed at this account:
 *
 * - the web wallet: the page's own requester, the transport the web's sheet
 *   is driven by (`vela.requester`, behind the dev gate);
 * - the Chrome extension: a real site's `eth_sendTransaction`, through the
 *   packaged extension's worker and its request window (`DappRequestHost`).
 *
 * The Safari package is the same app build with Safari's manifest
 * (`extension/build.mjs --safari`); Playwright cannot load a Safari extension,
 * so `safari-package.test.ts` proves the built package carries the same
 * guard wiring.
 */
import { createServer, type Server } from 'node:http';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test, type BrowserContext, type Page, type Route } from '@playwright/test';
import {
	extensionBuilt,
	extensionId,
	loadExtension,
	noRequestWindow,
	requestWindow
} from './extension-helpers';
import { denyOffOrigin, happyRelay, stubChainRegistry, stubJsonRpc, stubRelay } from './stub-chain';

const APP_ROOT = join(import.meta.dirname, '..');
const PORT = 8853;
const RELAY = /vela-relay(-cf)?\.getvela\.app/;
const ACCOUNT = '0xD400866e00B055B20752a826CD5C89b811de130b';
const PUBLIC_KEY =
	'04197db9030a1e166bec2cee05e0ddb94b26ee0b6d6f429f1748cda4eedac36f04fe546861a9c9dfaf75719b53c75e0b933d4aad6d325f18c75776a260d507647b';
/** The Safe made outside Vela that lists the account as an owner. */
const SAFE = '0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe';
const IS_OWNER = '0x2f54bf6e';
const TRUE = '0x' + '0'.repeat(63) + '1';
const FALSE = '0x' + '0'.repeat(64);

const VECTOR = JSON.parse(
	readFileSync(
		join(APP_ROOT, '../../rust/crates/vela-core/tests/vectors/rules/parent-safe-guard.json'),
		'utf8'
	)
) as {
	signer: string;
	unknown: string;
	cases: { name: string; input: { params: { to: string; data: string; value: string }[] } }[];
};
const EXEC_SWAP = VECTOR.cases.find((c) => c.name === 'hotfix exec-swap: isOwner true, refused')!
	.input.params[0];

/** The vector's takeover, from `signer`, for `safe`: its Safe and signer replaced by them. */
function takeover(signer: string, safe: string): { from: string; to: string; data: string } {
	const strip = (a: string) => a.slice(2).toLowerCase();
	const data = EXEC_SWAP.data
		.replaceAll(strip(VECTOR.unknown), strip(safe))
		.replaceAll(strip(VECTOR.signer), strip(signer));
	return { from: signer, to: safe, data };
}

/** The wallet's shortened address (`shortenAddress`): 8 then 6. */
const short = (address: string) => `${address.slice(0, 8)}…${address.slice(-6)}`;
const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

/** Is this JSON-RPC body the guard's `isOwner(account)` read of `SAFE`? */
function isOwnerRead(method: string, params: unknown[]): boolean {
	const call = params[0] as { to?: string; data?: string } | undefined;
	return (
		method === 'eth_call' &&
		String(call?.to ?? '').toLowerCase() === SAFE &&
		String(call?.data ?? '').startsWith(IS_OWNER)
	);
}

type Owner = 'owner' | 'not-owner' | 'unreachable';

test.describe('the web wallet (its requester)', () => {
	test.use({ viewport: { width: 390, height: 844 } });
	test.setTimeout(120_000);

	async function openWallet(page: Page, owner: Owner, reads: unknown[]): Promise<void> {
		await denyOffOrigin(page);
		// Ethereum's nodes are the stub's, so the guard's read reaches it.
		await stubChainRegistry(page, {
			1: {
				chainId: 1,
				name: 'Ethereum',
				nativeCurrency: { name: 'Ether', symbol: 'ETH', decimals: 18 },
				stables: [],
				wrappedNativeToken: null,
				dex: null,
				rpc: ['https://stub-rpc.test/rpc/1']
			}
		});
		await stubJsonRpc(page, /stub-rpc\.test\/rpc\/(\d+)/, (method, params) => {
			if (isOwnerRead(method, params)) {
				reads.push(params[0]);
				// `undefined` is a 500 from the stub: no node answered.
				if (owner === 'unreachable') return undefined;
				return owner === 'owner' ? TRUE : FALSE;
			}
			if (method === 'eth_chainId') return '0x1';
			if (method === 'eth_blockNumber') return '0x10';
			if (method === 'eth_getCode') return '0x6080';
			return undefined;
		});
		await stubRelay(page, RELAY, happyRelay('0x' + '11'.repeat(32), '0x' + '22'.repeat(32)));
		await page.addInitScript(
			({ key, account }) => {
				localStorage.setItem('vela.intro.seen', String(Date.now()));
				localStorage.setItem('vela.dev.console', '1');
				localStorage.setItem(
					'vela.accounts',
					JSON.stringify([
						{
							id: 'e2e-credential-id',
							name: 'E2E Wallet',
							address: account,
							public_key_hex: key,
							created_at_iso: '2026-01-01T00:00:00.000Z',
							keys: []
						}
					])
				);
				localStorage.setItem('vela.activeAccountIndex', '0');
			},
			{ key: PUBLIC_KEY, account: ACCOUNT }
		);
		await page.goto('/en/wallet');
		await expect(page.getByText('E2E Wallet').first()).toBeVisible();
		await page.waitForFunction(
			() => (window as unknown as { vela?: { requester?: unknown } }).vela?.requester !== undefined,
			null,
			{ timeout: 20_000 }
		);
	}

	async function fireTakeover(page: Page, to?: string): Promise<void> {
		await page.evaluate(
			(tx) => {
				const w = window as unknown as {
					vela: { requester: { fire(method: string, params: unknown[]): Promise<unknown> } };
					__answer?: unknown;
					__error?: unknown;
				};
				void w.vela.requester
					.fire('eth_sendTransaction', [{ ...tx, value: '0x0' }])
					.then((result) => (w.__answer = result))
					.catch((error) => (w.__error = error));
			},
			{ ...takeover(ACCOUNT, SAFE), ...(to === undefined ? {} : { to }) }
		);
	}

	const pageError = (page: Page) =>
		page.evaluate(() => (window as unknown as { __error?: unknown }).__error ?? null);

	test('an execTransaction (v = 1) on a Safe this account owns is refused, names it, and answers -32603', async ({
		page
	}) => {
		const reads: unknown[] = [];
		await openWallet(page, 'owner', reads);
		await fireTakeover(page);

		// The sheet opens refused: the catalog's sentence, naming the call and
		// the Safe — and no confirm, no "anyway".
		await expect(page.getByText('Vela can’t sign this')).toBeVisible({ timeout: 30_000 });
		const sentence = page.getByText(/act for a Safe you own \(execTransaction on/);
		await expect(sentence).toBeVisible();
		await expect(sentence).toContainText(new RegExp(escape(short(SAFE)), 'i'));
		await expect(page.getByTestId('signing-confirm')).toHaveCount(0);
		await expect(page.getByText(/anyway/i)).toHaveCount(0);
		expect(reads.length).toBeGreaterThanOrEqual(1);

		// Closing it answers the site: the wallet refused, not the person.
		await page.getByRole('button', { name: /^close$/i }).click();
		await expect
			.poll(() => pageError(page))
			.toMatchObject({ code: -32603, kind: 'parent_safe_blocked' });
		expect(await page.evaluate(() => (window as unknown as { __answer?: unknown }).__answer)).toBe(
			undefined
		);
	});

	test('when ownership cannot be read, it is refused “couldn’t check”, never allowed', async ({
		page
	}) => {
		const reads: unknown[] = [];
		await openWallet(page, 'unreachable', reads);
		await fireTakeover(page);
		await expect(page.getByText('Vela can’t sign this')).toBeVisible({ timeout: 30_000 });
		await expect(page.getByText(/couldn’t check if you own it/)).toBeVisible();
		await expect(page.getByTestId('signing-confirm')).toHaveCount(0);
		await page.getByRole('button', { name: /^close$/i }).click();
		await expect
			.poll(() => pageError(page))
			.toMatchObject({ code: -32603, kind: 'parent_safe_blocked' });
	});

	// 0.9.10: the parent's address written without its `0x` — what the
	// encoders read as the Safe and 0.9.9's guard did not — is refused at
	// arrival, -32602, before any read and with no sheet.
	test('the takeover with a bare-hex `to` is answered -32602 before any read (0.9.10)', async ({
		page
	}) => {
		const reads: unknown[] = [];
		await openWallet(page, 'owner', reads);
		await fireTakeover(page, SAFE.slice(2));
		await expect
			.poll(() => pageError(page))
			.toMatchObject({ code: -32602, kind: 'invalid_params' });
		await expect(page.getByText('Vela can’t sign this')).toHaveCount(0);
		await expect(page.getByTestId('signing-confirm')).toHaveCount(0);
		expect(reads).toEqual([]);
	});

	test('the same request to a Safe this account does not own goes on as before', async ({
		page
	}) => {
		const reads: unknown[] = [];
		await openWallet(page, 'not-owner', reads);
		await fireTakeover(page);
		await expect(page.getByTestId('signing-confirm')).toBeVisible({ timeout: 30_000 });
		await expect.poll(() => reads.length).toBeGreaterThanOrEqual(1);
		await expect(page.getByText('Vela can’t sign this')).toHaveCount(0);
		await expect(page.getByText(/act for a Safe you own/)).toHaveCount(0);
	});
});

interface AskResult {
	ok: boolean;
	code?: number;
	message?: string;
	result?: unknown;
}

/** Every page of the context: the guard's isOwner read of `SAFE` answers `true`. */
async function ownedEverywhere(context: BrowserContext, reads: unknown[]): Promise<void> {
	await context.route('**/*', async (route: Route) => {
		const request = route.request();
		if (request.method() === 'POST') {
			let body: { method?: string; params?: unknown[]; id?: number } | null;
			try {
				body = JSON.parse(request.postData() ?? 'null');
			} catch {
				body = null;
			}
			if (body && !Array.isArray(body) && isOwnerRead(body.method ?? '', body.params ?? [])) {
				reads.push(body.params?.[0]);
				return route.fulfill({
					contentType: 'application/json',
					body: JSON.stringify({ jsonrpc: '2.0', id: body.id ?? 1, result: TRUE })
				});
			}
		}
		return route.fallback();
	});
}

test.describe('the Chrome extension (a real site, its request window)', () => {
	test.skip(!extensionBuilt(), 'extension/dist is missing — run `pnpm build:extension`');
	test.setTimeout(180_000);

	let server: Server;
	test.beforeAll(async () => {
		const html = readFileSync(join(APP_ROOT, 'e2e/testdapp/index.html'));
		server = createServer((_req, res) => {
			res.setHeader('content-type', 'text/html; charset=utf-8');
			res.end(html);
		});
		await new Promise<void>((resolve) => server.listen(PORT, () => resolve()));
	});
	test.afterAll(async () => {
		server.closeAllConnections();
		await new Promise((resolve) => server.close(() => resolve(null)));
	});

	async function seedWallet(context: BrowserContext): Promise<void> {
		const wallet = await context.newPage();
		await wallet.addInitScript(() => {
			localStorage.setItem('vela.intro.seen', String(Date.now()));
			localStorage.setItem('vela.dev.console', '1');
		});
		await wallet.goto(`chrome-extension://${extensionId()}/en/parallel.html`);
		await wallet.getByRole('button', { name: 'Enter (seed fixture wallet)' }).click();
		await wallet.waitForURL(/wallet\.html$/, { timeout: 30_000 });
		await wallet.waitForFunction(
			() =>
				(
					window as unknown as {
						chrome: { storage: { local: { get(k: string): Promise<object> } } };
					}
				).chrome.storage.local
					.get('vela.ext.cache')
					.then((all: Record<string, unknown>) => all['vela.ext.cache'] !== undefined),
			null,
			{ timeout: 30_000 }
		);
	}

	test('an execTransaction (v = 1) on an owned Safe is refused in the window, and the site hears -32603', async () => {
		const context = await loadExtension();
		const reads: unknown[] = [];
		await ownedEverywhere(context, reads);
		await seedWallet(context);
		const page = await context.newPage();
		await page.goto(`http://localhost:${PORT}/`);

		const connected = page.evaluate(() =>
			window.__ask('eth_requestAccounts')
		) as Promise<AskResult>;
		const consent = await requestWindow(context);
		await consent.getByRole('button', { name: 'Connect' }).click();
		expect((await connected).result).toEqual([ACCOUNT]);
		await noRequestWindow(context);

		const asked = page.evaluate(
			(tx) => window.__ask('eth_sendTransaction', [{ ...tx, value: '0x0' }]),
			takeover(ACCOUNT, SAFE)
		) as Promise<AskResult>;
		const win = await requestWindow(context);
		await expect(win.getByText('Vela can’t sign this')).toBeVisible({ timeout: 30_000 });
		const sentence = win.getByText(/act for a Safe you own \(execTransaction on/);
		await expect(sentence).toBeVisible();
		await expect(sentence).toContainText(new RegExp(escape(short(SAFE)), 'i'));
		await expect(win.getByTestId('signing-confirm')).toHaveCount(0);
		expect(reads.length).toBeGreaterThanOrEqual(1);

		await win.getByRole('button', { name: /^close$/i }).click();
		const answer = await asked;
		expect(answer.ok).toBe(false);
		expect(answer.code).toBe(-32603);
		expect(answer.message).toMatch(/act for a Safe this account owns/);
		await context.close();
	});

	test('the takeover with a bare-hex `to` is answered -32602, never read or signed (0.9.10)', async () => {
		const context = await loadExtension();
		const reads: unknown[] = [];
		await ownedEverywhere(context, reads);
		await seedWallet(context);
		const page = await context.newPage();
		await page.goto(`http://localhost:${PORT}/`);

		const connected = page.evaluate(() =>
			window.__ask('eth_requestAccounts')
		) as Promise<AskResult>;
		const consent = await requestWindow(context);
		await consent.getByRole('button', { name: 'Connect' }).click();
		expect((await connected).result).toEqual([ACCOUNT]);
		await noRequestWindow(context);

		const answer = (await page.evaluate(
			(tx) => window.__ask('eth_sendTransaction', [{ ...tx, value: '0x0' }]),
			{ ...takeover(ACCOUNT, SAFE), to: SAFE.slice(2) }
		)) as AskResult;
		expect(answer.ok).toBe(false);
		expect(answer.code).toBe(-32602);
		expect(reads).toEqual([]);
		await noRequestWindow(context);
		await context.close();
	});
});
