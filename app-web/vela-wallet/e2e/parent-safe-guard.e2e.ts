/**
 * §31 — a site never makes this account act for a multisig it owns, at every
 * dApp door the web has (the parent-Safe guard; disclosure 2026-09-29).
 *
 * The attack: a site connected to a Vela account asks it to call
 * `execTransaction` on a multisig the account is a member of, with the
 * "approved by msg.sender" signature (v = 1) — for a 1-of-N multisig, an
 * immediate takeover (`swapOwner` to the attacker). The core refuses it
 * before anything can be signed when the multisig is on this device's list
 * (`multisigs_changed`), and the sheet says so in the catalog's words, with
 * no confirm; closing it answers the site -32603 `parent_safe_blocked`.
 *
 * The request is the vector's case (a)
 * (`tests/vectors/rules/parent-safe-guard.json`), re-aimed at a real multisig
 * this device holds (its address derived by the core from its genesis — a
 * stored row whose address does not derive is not one the wallet keeps):
 *
 * - the web wallet: the page's own requester, the transport the web's
 *   sheet is driven by (`vela.requester`, behind the dev gate);
 * - the Chrome extension: a real site's `eth_sendTransaction`, through the
 *   packaged extension's worker and its request window (`DappRequestHost`).
 *
 * The Safari package is the same app build with Safari's manifest
 * (`extension/build.mjs --safari`); Playwright cannot load a Safari
 * extension, so `safari-package.test.ts` proves the built package carries
 * this same guard wiring.
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
import { denyOffOrigin, happyRelay, stubJsonRpc, stubRelay } from './stub-chain';
import { FIXTURES, multisigOf, storedRow, type Multisig } from './stub-multisig';

const APP_ROOT = join(import.meta.dirname, '..');
const PORT = 8853;
const RELAY = /vela-relay(-cf)?\.getvela\.app/;
const FIXTURE_PUBLIC_KEY =
	'04197db9030a1e166bec2cee05e0ddb94b26ee0b6d6f429f1748cda4eedac36f04fe546861a9c9dfaf75719b53c75e0b933d4aad6d325f18c75776a260d507647b';

const VECTOR = JSON.parse(
	readFileSync(
		join(APP_ROOT, '../../rust/crates/vela-core/tests/vectors/rules/parent-safe-guard.json'),
		'utf8'
	)
) as {
	signer: string;
	parent: string;
	cases: { name: string; input: { params: { to: string; data: string; value: string }[] } }[];
};
const CASE_A = VECTOR.cases.find(
	(c) => c.name === 'a: execTransaction(swapOwner) on a multisig on the list, v=1'
)!.input.params[0];

/**
 * Case (a), from `signer`, for `multisig`: the vector's parent and signer
 * replaced by them wherever the calldata names them (the call's target, the
 * `swapOwner` it carries, the v = 1 signature's owner).
 */
function takeover(signer: string, multisig: string): { from: string; to: string; data: string } {
	const strip = (a: string) => a.slice(2).toLowerCase();
	const data = CASE_A.data
		.replaceAll(strip(VECTOR.parent), strip(multisig))
		.replaceAll(strip(VECTOR.signer), strip(signer));
	return { from: signer, to: multisig, data };
}

/** The wallet's shortened address (`shortenAddress`): 8 then 6. */
const short = (address: string) => `${address.slice(0, 8)}…${address.slice(-6)}`;
const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

/** A 1-of-2 multisig fixture one is a member of — the one a takeover needs a single signature for. */
const memberMultisig = () =>
	multisigOf([FIXTURES.one, FIXTURES.two], 1, 'Parent guard fund', '0x31');

test.describe('the web wallet (its requester)', () => {
	test.use({ viewport: { width: 390, height: 844 } });
	test.setTimeout(120_000);

	async function openWallet(page: Page, ms: Multisig): Promise<void> {
		await denyOffOrigin(page);
		await stubJsonRpc(page, /stub-rpc\.test\/rpc\/(\d+)/, (method) => {
			if (method === 'eth_chainId') return '0x1';
			if (method === 'eth_blockNumber') return '0x10';
			if (method === 'eth_getCode') return '0x6080';
			return undefined;
		});
		await stubRelay(page, RELAY, happyRelay('0x' + '11'.repeat(32), '0x' + '22'.repeat(32)));
		await page.addInitScript(
			({ key, account, row }) => {
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
				// The multisig is on this device, NOT the account shown: the site
				// talks to the Vela account, which is one of its members.
				localStorage.setItem('vela.multisigAccounts', JSON.stringify([row]));
			},
			{ key: FIXTURE_PUBLIC_KEY, account: FIXTURES.one, row: storedRow(ms, [1]) }
		);
		await page.goto('/en/wallet');
		await expect(page.getByText('E2E Wallet').first()).toBeVisible();
		await page.waitForFunction(
			() => (window as unknown as { vela?: { requester?: unknown } }).vela?.requester !== undefined,
			null,
			{ timeout: 20_000 }
		);
	}

	test('case (a) on a multisig of this device is refused, names it, and answers -32603', async ({
		page
	}) => {
		const ms = await memberMultisig();
		await openWallet(page, ms);
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
			takeover(FIXTURES.one, ms.address)
		);

		// The sheet opens refused: the catalog's sentence, naming the call and
		// the multisig — and no confirm, no fee, no "anyway".
		await expect(page.getByText('Vela can’t sign this')).toBeVisible({ timeout: 30_000 });
		const sentence = page.getByText(/multisig you’re a member of \(execTransaction on/);
		await expect(sentence).toBeVisible();
		await expect(sentence).toContainText(new RegExp(escape(short(ms.address)), 'i'));
		await expect(page.getByTestId('signing-confirm')).toHaveCount(0);
		await expect(page.getByText(/anyway/i)).toHaveCount(0);

		// Closing it answers the site: the wallet refused, not the person.
		await page.getByRole('button', { name: /^close$/i }).click();
		await expect
			.poll(() =>
				page.evaluate(() => (window as unknown as { __error?: { code?: number } }).__error ?? null)
			)
			.toMatchObject({ code: -32603, kind: 'parent_safe_blocked' });
		expect(await page.evaluate(() => (window as unknown as { __answer?: unknown }).__answer)).toBe(
			undefined
		);
	});
});

interface AskResult {
	ok: boolean;
	code?: number;
	message?: string;
	result?: unknown;
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

	/** The fixture wallet in the parallel space, holding `ms` (not shown). */
	async function walletWith(context: BrowserContext, ms: Multisig): Promise<void> {
		const id = extensionId();
		const wallet = await context.newPage();
		await wallet.addInitScript(() => {
			localStorage.setItem('vela.intro.seen', String(Date.now()));
			localStorage.setItem('vela.dev.console', '1');
		});
		await wallet.goto(`chrome-extension://${id}/en/parallel.html`);
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
		await wallet.evaluate(
			(row) => localStorage.setItem('vela.multisigAccounts', JSON.stringify([row])),
			storedRow(ms, [1])
		);
		await wallet.reload();
		await wallet.waitForURL(/wallet\.html$/, { timeout: 30_000 });
	}

	test('case (a) on a multisig of this device is refused in the window, and the site hears -32603', async () => {
		const ms = await memberMultisig();
		const context = await loadExtension();
		await walletWith(context, ms);
		const page = await context.newPage();
		await page.goto(`http://localhost:${PORT}/`);

		const connected = page.evaluate(() =>
			window.__ask('eth_requestAccounts')
		) as Promise<AskResult>;
		const consent = await requestWindow(context);
		await consent.getByRole('button', { name: 'Connect' }).click();
		expect((await connected).result).toEqual([FIXTURES.one]);
		await noRequestWindow(context);

		const asked = page.evaluate(
			(tx) => window.__ask('eth_sendTransaction', [{ ...tx, value: '0x0' }]),
			takeover(FIXTURES.one, ms.address)
		) as Promise<AskResult>;
		const win = await requestWindow(context);
		await expect(win.getByText('Vela can’t sign this')).toBeVisible({ timeout: 30_000 });
		const sentence = win.getByText(/multisig you’re a member of \(execTransaction on/);
		await expect(sentence).toBeVisible();
		await expect(sentence).toContainText(new RegExp(escape(short(ms.address)), 'i'));
		await expect(win.getByTestId('signing-confirm')).toHaveCount(0);

		await win.getByRole('button', { name: /^close$/i }).click();
		const answer = await asked;
		expect(answer.ok).toBe(false);
		expect(answer.code).toBe(-32603);
		expect(answer.message).toMatch(/act for a multisig this account is a member of/);
		await context.close();
	});
});
