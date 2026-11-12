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
import { eip55 } from '../src/lib/multisig/external/eip55';

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

/** A Safe this device does not list — the account's only if `isOwner` says so. */
const UNKNOWN_SAFE = '0x7777777777777777777777777777777777777777';
const IS_OWNER = '0x2f54bf6e';
const ABI_TRUE = '0x' + '0'.repeat(63) + '1';
/** How long the node takes to answer `isOwner`: long enough for many frames. */
const READ_DELAY_MS = 800;

/** EIP-712 `SafeMessage` under `safe`'s domain on chain 1 (§31 (c)). */
function safeMessage(safe: string): string {
	return JSON.stringify({
		types: {
			EIP712Domain: [
				{ name: 'chainId', type: 'uint256' },
				{ name: 'verifyingContract', type: 'address' }
			],
			SafeMessage: [{ name: 'message', type: 'bytes' }]
		},
		primaryType: 'SafeMessage',
		domain: { chainId: 1, verifyingContract: safe },
		message: { message: '0x' + 'ab'.repeat(32) }
	});
}

/**
 * Every off-origin POST that is the core's `isOwner` read of
 * {@link UNKNOWN_SAFE}: answered `true` (the account owns it) after
 * {@link READ_DELAY_MS}. Anything else is refused (`hermetic`), or — the web
 * wallet, whose home reads every network in the background — a public node's
 * JSON-RPC is ANSWERED with "method not available" (a refused request would
 * have the pool find the network silent and skip it, and the read would
 * never be asked), while the suite's own stubs answer theirs.
 */
async function slowIsOwner(
	target: Page | BrowserContext,
	hermetic: boolean,
	own: RegExp = /(?:stub|witness)-rpc\.test|vela-relay/
): Promise<{ reads: number }> {
	const seen = { reads: 0 };
	await target.route(/^https?:\/\/(?!localhost[:/]|127\.0\.0\.1[:/])/, async (route) => {
		const request = route.request();
		if (request.method() === 'POST') {
			let body: unknown;
			try {
				body = JSON.parse(request.postData() ?? 'null');
			} catch {
				body = null;
			}
			const calls = (Array.isArray(body) ? body : [body]) as {
				id?: number;
				method?: string;
				params?: [{ to?: string; data?: string }];
			}[];
			const read = calls.find(
				(c) =>
					c?.method === 'eth_call' &&
					String(c.params?.[0]?.to ?? '').toLowerCase() === UNKNOWN_SAFE &&
					String(c.params?.[0]?.data ?? '').startsWith(IS_OWNER)
			);
			// R22: a corroborated read asks each source its chain first.
			if (calls.length === 1 && calls[0]?.method === 'eth_chainId' && !own.test(request.url())) {
				return route.fulfill({
					contentType: 'application/json',
					body: JSON.stringify({ jsonrpc: '2.0', id: calls[0].id ?? 1, result: '0x1' })
				});
			}
			if (read && calls.length === 1) {
				seen.reads += 1;
				await new Promise((r) => setTimeout(r, READ_DELAY_MS));
				return route.fulfill({
					contentType: 'application/json',
					body: JSON.stringify({ jsonrpc: '2.0', id: read.id ?? 1, result: ABI_TRUE })
				});
			}
		}
		if (hermetic) return route.abort('blockedbyclient');
		if (request.method() !== 'POST' || own.test(request.url())) return route.fallback();
		return route.fulfill({
			contentType: 'application/json',
			body: JSON.stringify({
				jsonrpc: '2.0',
				id: 1,
				error: { code: -32601, message: 'the method is not available on this stub' }
			})
		});
	});
	return seen;
}

/** One drawn frame of a page: its buttons, and the two lines that matter here. */
interface Frame {
	at: number;
	buttons: { text: string; disabled: boolean }[];
	confirm: { text: string; disabled: boolean } | null;
	reading: boolean;
	refused: boolean;
}

/**
 * r18-6: a FRAME WATCHER, from the document's first frame — every mutation
 * and every animation frame, every button whatever its state (a shut "Sign
 * anyway" is the bug), recorded each time what is drawn changes.
 */
function watchFrames(): void {
	const w = window as unknown as { __frames: Frame[] };
	w.__frames = [];
	const started = performance.now();
	let last = '';
	const look = () => {
		const body = document.body;
		if (!body) return;
		const text = body.textContent ?? '';
		const buttons = [...document.querySelectorAll('button')].map((b) => ({
			text: (b.textContent ?? '').replace(/\s+/g, ' ').trim(),
			disabled: b.disabled || b.getAttribute('aria-disabled') === 'true'
		}));
		const c = document.querySelector<HTMLButtonElement>('[data-testid="signing-confirm"]');
		const frame = {
			buttons,
			confirm: c
				? {
						text: (c.textContent ?? '').replace(/\s+/g, ' ').trim(),
						disabled: c.disabled || c.getAttribute('aria-disabled') === 'true'
					}
				: null,
			reading: text.includes('Reading the request…'),
			refused: text.includes('Vela can’t sign this')
		};
		const key = JSON.stringify(frame);
		if (key === last) return;
		last = key;
		w.__frames.push({ at: Math.round(performance.now() - started), ...frame });
	};
	new MutationObserver(look).observe(document, {
		subtree: true,
		childList: true,
		attributes: true,
		characterData: true
	});
	const tick = () => {
		look();
		requestAnimationFrame(tick);
	};
	requestAnimationFrame(tick);
}

/** What every frame from the request's arrival to its refusal drew. */
function expectNoAnywayFrame(frames: Frame[]): void {
	const anyway = frames.filter((f) => f.buttons.some((b) => /anyway/i.test(b.text)));
	expect(
		anyway.map((f) => `${f.at} ms: ${f.buttons.map((b) => b.text).join(' | ')}`),
		'a frame drew an "… anyway" button'
	).toEqual([]);
	// The read was seen out: "Reading the request…" over a shut, plain "Sign".
	expect(
		frames.some((f) => f.reading && f.confirm?.text === 'Sign' && f.confirm.disabled),
		'no frame said "Reading the request…" over a disabled "Sign"'
	).toBe(true);
	// …and then refused, with no confirm at all.
	expect(frames.some((f) => f.refused && f.confirm === null)).toBe(true);
}

/** A 1-of-2 multisig fixture one is a member of — the one a takeover needs a single signature for. */
const memberMultisig = () =>
	multisigOf([FIXTURES.one, FIXTURES.two], 1, 'Parent guard fund', '0x31');

test.describe('the web wallet (its requester)', () => {
	test.use({ viewport: { width: 390, height: 844 } });
	test.setTimeout(120_000);

	async function openWallet(
		page: Page,
		ms: Multisig,
		routes?: () => Promise<unknown>
	): Promise<void> {
		await denyOffOrigin(page);
		await stubJsonRpc(page, /stub-rpc\.test\/rpc\/(\d+)/, (method) => {
			if (method === 'eth_chainId') return '0x1';
			if (method === 'eth_blockNumber') return '0x10';
			if (method === 'eth_getCode') return '0x6080';
			return undefined;
		});
		await stubRelay(page, RELAY, happyRelay('0x' + '11'.repeat(32), '0x' + '22'.repeat(32)));
		await routes?.();
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
		await expect(sentence).toContainText(new RegExp(escape(short(eip55(ms.address)))));
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

	/**
	 * r18-6: a SafeMessage under a Safe this device does not list — the core
	 * reads `isOwner` first (`confirm_block: reading`). Every frame from its
	 * arrival to the refusal: "Reading the request…" over a shut "Sign",
	 * never a "Sign anyway" (shut or not), then the parent's refusal.
	 */
	test('a SafeMessage to an unlisted Safe: every frame until the refusal, and none says "anyway"', async ({
		page
	}) => {
		const ms = await memberMultisig();
		let seen = { reads: 0 };
		await openWallet(page, ms, async () => (seen = await slowIsOwner(page, false)));
		await page.evaluate(watchFrames);
		await page.evaluate(
			({ signer, data }) => {
				const w = window as unknown as {
					vela: { requester: { fire(method: string, params: unknown[]): Promise<unknown> } };
					__error?: unknown;
				};
				void w.vela.requester
					.fire('eth_signTypedData_v4', [signer, data])
					.catch((error) => (w.__error = error));
			},
			{ signer: FIXTURES.one, data: safeMessage(UNKNOWN_SAFE) }
		);
		await expect(page.getByText('Vela can’t sign this')).toBeVisible({ timeout: 30_000 });
		await expect(page.getByText(/multisig you’re a member of \(SafeMessage on/)).toBeVisible();
		// R22: a corroborated read (SEC-PARENT-3) — "an owner" refuses on one
		// node's word; a second, independent node may be asked beside it.
		expect(seen.reads).toBeGreaterThanOrEqual(1);
		expect(seen.reads).toBeLessThanOrEqual(2);
		const frames = await page.evaluate(() => (window as unknown as { __frames: Frame[] }).__frames);
		expectNoAnywayFrame(frames);
		await page.getByRole('button', { name: /^close$/i }).click();
		await expect
			.poll(() =>
				page.evaluate(() => (window as unknown as { __error?: { code?: number } }).__error ?? null)
			)
			.toMatchObject({ code: -32603, kind: 'parent_safe_blocked' });
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
		await expect(sentence).toContainText(new RegExp(escape(short(eip55(ms.address)))));
		await expect(win.getByTestId('signing-confirm')).toHaveCount(0);

		await win.getByRole('button', { name: /^close$/i }).click();
		const answer = await asked;
		expect(answer.ok).toBe(false);
		expect(answer.code).toBe(-32603);
		expect(answer.message).toMatch(/act for a multisig \(Safe\) on this account.s behalf/);
		await context.close();
	});

	/**
	 * r18-6 (seen on Safari's package, the same code): the request window, a
	 * SafeMessage under a Safe the device does not list — every frame of the
	 * window from its first: never a "Sign anyway", "Reading the request…"
	 * over a shut "Sign" while `isOwner` is out, then the refusal.
	 */
	test('a SafeMessage to an unlisted Safe: every frame of the window, and none says "anyway"', async () => {
		const ms = await memberMultisig();
		const context = await loadExtension();
		await walletWith(context, ms);
		const seen = await slowIsOwner(context, true);
		// Every document of the context from its first frame: the request
		// window included, which opens with the request.
		await context.addInitScript(watchFrames);
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
			({ signer, data }) => window.__ask('eth_signTypedData_v4', [signer, data]),
			{ signer: FIXTURES.one, data: safeMessage(UNKNOWN_SAFE) }
		) as Promise<AskResult>;
		const win = await requestWindow(context);
		await expect(win.getByText('Vela can’t sign this')).toBeVisible({ timeout: 30_000 });
		await expect(win.getByText(/multisig you’re a member of \(SafeMessage on/)).toBeVisible();
		// R22: a corroborated read (SEC-PARENT-3) — "an owner" refuses on one
		// node's word; a second, independent node may be asked beside it.
		expect(seen.reads).toBeGreaterThanOrEqual(1);
		expect(seen.reads).toBeLessThanOrEqual(2);
		const frames = await win.evaluate(() => (window as unknown as { __frames: Frame[] }).__frames);
		expectNoAnywayFrame(frames);
		await win.getByRole('button', { name: /^close$/i }).click();
		const answer = await asked;
		expect(answer.ok).toBe(false);
		expect(answer.code).toBe(-32603);
		await context.close();
	});
});
