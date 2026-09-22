/**
 * The Clear Signer, from the web wallet (spec 071 T041), end to end.
 *
 * The signer page — `app-web/clearsigning`, the real one — is served here on a
 * second origin, and the wallet reaches it the only way the web can: it
 * `window.open`s the page and they talk by `postMessage`, each side checking
 * the other's origin. The passkey is a CDP virtual authenticator inside the
 * popup, holding the private key whose public half the seeded wallet stores —
 * so the page runs its real ceremony, and the wallet's core judges the real
 * answer against the digest the WALLET computed.
 *
 * Covered: Settings stores the choice and the page (the core validates it);
 * a request starts at that default; a signature comes back through the page;
 * a page closed without signing leaves the request open and says so; and an
 * answer by a key that is not this wallet's is refused before anything is
 * sent.
 */
import { createServer, type Server } from 'node:http';
import { readFileSync, statSync } from 'node:fs';
import { extname, join, normalize } from 'node:path';
import { webcrypto } from 'node:crypto';
import { expect, test, type BrowserContext, type Page } from '@playwright/test';
import { en } from './live-helpers';
import { denyOffOrigin, readKv, stubJsonRpc, stubRelay, happyRelay } from './stub-chain';
import { answerWhere } from './clear-signer-helpers';

test.use({ viewport: { width: 390, height: 844 } });
test.setTimeout(120_000);

const RELAY = /vela-relay(-cf)?\.getvela\.app/;
const SAFE = '0xD400866e00B055B20752a826CD5C89b811de130b';
const PAGE_ROOT = join(import.meta.dirname, '..', '..', 'clearsigning');
const TYPES: Record<string, string> = {
	'.html': 'text/html; charset=utf-8',
	'.js': 'text/javascript; charset=utf-8',
	'.css': 'text/css; charset=utf-8',
	'.svg': 'image/svg+xml',
	'.png': 'image/png',
	'.json': 'application/json'
};

/** The signer page on its own origin: `http://localhost:<port>/` (loopback http is allowed). */
let server: Server;
let signerUrl = '';

test.beforeAll(async () => {
	server = createServer((request, response) => {
		const path = normalize(decodeURIComponent(new URL(request.url ?? '/', 'http://x').pathname));
		const file = join(PAGE_ROOT, path === '/' ? 'index.html' : path);
		try {
			if (!file.startsWith(PAGE_ROOT) || !statSync(file).isFile()) throw new Error('no');
			response.writeHead(200, {
				'content-type': TYPES[extname(file)] ?? 'application/octet-stream'
			});
			response.end(readFileSync(file));
		} catch {
			response.writeHead(404);
			response.end();
		}
	});
	await new Promise<void>((resolve) => server.listen(0, resolve));
	const address = server.address();
	if (address === null || typeof address === 'string') throw new Error('no port');
	// `localhost`, not an IP: WebAuthn will not take an IP address as a relying party.
	signerUrl = `http://localhost:${address.port}/`;
});

test.afterAll(() => {
	server.close();
});

/** A passkey: its private key for the authenticator, its public half for the wallet. */
async function passkey() {
	const pair = (await webcrypto.subtle.generateKey({ name: 'ECDSA', namedCurve: 'P-256' }, true, [
		'sign',
		'verify'
	])) as CryptoKeyPair;
	const rawId = Buffer.from(webcrypto.getRandomValues(new Uint8Array(16)));
	return {
		credentialHex: rawId.toString('hex'),
		credentialB64: rawId.toString('base64'),
		privateKeyB64: Buffer.from(await webcrypto.subtle.exportKey('pkcs8', pair.privateKey)).toString(
			'base64'
		),
		publicKeyHex: Buffer.from(await webcrypto.subtle.exportKey('raw', pair.publicKey)).toString(
			'hex'
		)
	};
}

type Passkey = Awaited<ReturnType<typeof passkey>>;

/** A signed-in wallet whose one key is `publicKeyHex`, known by `credentialHex`. */
async function openWallet(page: Page, key: { credentialHex: string; publicKeyHex: string }) {
	await denyOffOrigin(page);
	await stubJsonRpc(page, /stub-rpc\.test\/rpc\/(\d+)/, (method) => {
		if (method === 'eth_chainId') return '0x1';
		if (method === 'eth_blockNumber') return '0x10';
		if (method === 'eth_getCode') return '0x6080';
		return undefined;
	});
	await stubRelay(page, RELAY, happyRelay('0x' + '11'.repeat(32), '0x' + '22'.repeat(32)));
	await page.addInitScript(
		(seed) => {
			localStorage.setItem('vela.intro.seen', String(Date.now()));
			localStorage.setItem('vela.dev.console', '1');
			localStorage.setItem(
				'vela.accounts',
				JSON.stringify([
					{
						id: seed.credentialHex,
						name: 'E2E Wallet',
						address: seed.safe,
						public_key_hex: seed.publicKeyHex,
						created_at_iso: '2026-01-01T00:00:00.000Z',
						keys: []
					}
				])
			);
			localStorage.setItem('vela.activeAccountIndex', '0');
		},
		{ ...key, safe: SAFE }
	);
}

/** Settings → Advanced → "Sign with" = Clear Signer, and the page = ours. */
async function chooseClearSigner(page: Page): Promise<void> {
	await page.goto('/en/settings');
	await page.getByText(en('settings.sections.advanced'), { exact: true }).click();

	await page.getByText(en('settings.signing.title'), { exact: true }).click();
	await expect(page.getByText(en('settings.signing.subtitle'))).toBeVisible();
	await page
		.getByRole('option', { name: new RegExp(en('componentsUi.signing.clearSignerTitle')) })
		.click();

	await page.getByText(en('settings.signing.pageTitle'), { exact: true }).click();
	const field = page.getByRole('textbox', { name: en('settings.signing.pageTitle') });
	// An address the browser would not sign on is refused by the core, and said so.
	await field.fill('http://sign.example.org');
	await page.getByRole('button', { name: en('settings.signing.pageSave') }).click();
	await expect(page.getByText(en('settings.signing.pageInsecure'))).toBeVisible();
	await field.fill(signerUrl);
	await page.getByRole('button', { name: en('settings.signing.pageSave') }).click();
	await expect(page.getByText(en('settings.signing.pageInsecure'))).toBeHidden();
	// A page off getvela.app cannot use this wallet's passkeys — said beside it.
	await expect(page.getByText(en('settings.signing.pageForeign'))).toBeVisible();
	await expect(page.getByRole('button', { name: en('settings.signing.pageReset') })).toBeVisible();

	await expect.poll(() => readKv(page, 'vela.signMethod')).toBe('clear_signer');
	await expect.poll(() => readKv(page, 'vela.clearSignerUrl')).toBe(signerUrl);
}

async function fire(page: Page, method: string, params: unknown[]): Promise<void> {
	await page.waitForFunction(
		() => (window as unknown as { vela?: { requester?: unknown } }).vela?.requester !== undefined,
		null,
		{ timeout: 20_000 }
	);
	await page.evaluate(
		([m, p]) => {
			const w = window as unknown as {
				vela: { requester: { fire(method: string, params: unknown[]): Promise<unknown> } };
				__answer?: unknown;
				__error?: unknown;
			};
			w.__answer = undefined;
			w.__error = undefined;
			void w.vela.requester
				.fire(m as string, p as unknown[])
				.then((result) => (w.__answer = result))
				.catch((error) => (w.__error = error));
		},
		[method, params] as const
	);
}

/**
 * Slide (the keyboard path of the same control), answer WHERE the signer is,
 * and catch the page it opens.
 *
 * Spec 075: the Clear Signer is a passkey route, and a route can be on another
 * device — so every request asks first. The tap is also the user activation a
 * popup needs, which the swipe may have spent by the time the operation was
 * assembled.
 */
async function slideToClearSigner(page: Page, context: BrowserContext): Promise<Page> {
	const slider = page.getByRole('button', { name: /^Slide to confirm/ });
	await slider.waitFor({ state: 'visible', timeout: 30_000 });
	const popup = context.waitForEvent('page', { timeout: 30_000 });
	await slider.focus();
	await slider.press('Enter');
	await answerWhere(page, 'this');
	return popup;
}

/** The authenticator the person carries — inside the signer page's window. */
async function holdPasskey(popup: Page, key: Passkey): Promise<void> {
	const cdp = await popup.context().newCDPSession(popup);
	await cdp.send('WebAuthn.enable');
	const { authenticatorId } = await cdp.send('WebAuthn.addVirtualAuthenticator', {
		options: {
			protocol: 'ctap2',
			transport: 'internal',
			hasResidentKey: true,
			hasUserVerification: true,
			isUserVerified: true,
			automaticPresenceSimulation: true
		}
	});
	await cdp.send('WebAuthn.addCredential', {
		authenticatorId,
		credential: {
			credentialId: key.credentialB64,
			isResidentCredential: true,
			rpId: 'localhost',
			privateKey: key.privateKeyB64,
			userHandle: Buffer.from('vela-e2e').toString('base64'),
			signCount: 0
		}
	});
}

/** The page rendered the request; sign it there (its automation hook is the slide's own confirm). */
async function signOnPage(popup: Page): Promise<void> {
	await popup.waitForFunction(
		() => !!(window as unknown as { __slider?: unknown }).__slider,
		null,
		{
			timeout: 20_000
		}
	);
	await popup.evaluate(() =>
		(window as unknown as { __slider: { __confirm(): void } }).__slider.__confirm()
	);
}

const answerOf = (page: Page) =>
	page.evaluate(() => (window as unknown as { __answer?: unknown }).__answer ?? null);

test('Settings chooses the Clear Signer; a request starts there and is signed on its page', async ({
	page,
	context
}) => {
	const key = await passkey();
	await openWallet(page, key);
	await chooseClearSigner(page);

	await page.goto('/en/wallet');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await fire(page, 'personal_sign', ['0x68656c6c6f', SAFE]);

	// The request starts at the stored default, not at "Automatic".
	await expect(page.getByText(en('componentsUi.signing.clearSignerTitle')).first()).toBeVisible({
		timeout: 25_000
	});

	const popup = await slideToClearSigner(page, context);
	expect(popup.url()).toBe(`${signerUrl}sign.html?ch=post`);
	await expect(page.getByText(en('componentsUi.signing.clearSignerWaiting'))).toBeVisible();
	await holdPasskey(popup, key);
	await signOnPage(popup);

	// The page's answer, accepted by the wallet's core over the wallet's digest,
	// became the EIP-1271 signature the dApp gets — as a passkey's would have.
	await expect.poll(() => answerOf(page), { timeout: 20_000 }).toMatch(/^0x[0-9a-f]{200,}$/);
	await expect(page.getByText(en('componentsUi.signing.clearSignerWaiting'))).toBeHidden();
});

test('a page closed without signing leaves the request open, and says so', async ({
	page,
	context
}) => {
	const key = await passkey();
	await openWallet(page, key);
	await chooseClearSigner(page);
	await page.goto('/en/wallet');
	await fire(page, 'personal_sign', ['0x68656c6c6f', SAFE]);

	const popup = await slideToClearSigner(page, context);
	await popup.waitForLoadState();
	await popup.close();

	await expect(page.getByText(en('componentsUi.signing.clearSignerClosed'))).toBeVisible({
		timeout: 10_000
	});
	await page
		.getByRole('button', { name: en('componentsUi.signing.close'), exact: true })
		.last()
		.click();
	// Nothing was signed, nothing answered: the request is still there to sign another way.
	await expect(page.getByRole('button', { name: /^Slide to confirm/ })).toBeVisible();
	expect(await answerOf(page)).toBeNull();
});

test('an answer by a key that is not this wallet’s is refused before anything is sent', async ({
	page,
	context
}) => {
	const held = await passkey();
	const other = await passkey();
	// The wallet knows the credential id, but a different public key stands behind it.
	await openWallet(page, { credentialHex: held.credentialHex, publicKeyHex: other.publicKeyHex });
	await chooseClearSigner(page);
	await page.goto('/en/wallet');
	await fire(page, 'personal_sign', ['0x68656c6c6f', SAFE]);

	const popup = await slideToClearSigner(page, context);
	await holdPasskey(popup, held);
	await signOnPage(popup);

	await expect(page.getByText(en('componentsUi.signing.clearSignerMismatch'))).toBeVisible({
		timeout: 20_000
	});
	expect(await answerOf(page)).toBeNull();
});

/**
 * Spec 075: a key that lives behind a page is signed THERE, with no Settings
 * involved.
 *
 * `auto` is the factory default — "do what you always did" — and what this
 * wallet always did was open the browser's own passkey sheet. For a key minted
 * on a Clear Signer page that sheet can see nothing at all, so `auto` must
 * follow the key to its page instead. The account record's `signer_origin` is
 * the only thing that says where, which is why it has to survive every rewrite.
 */
test('a key that lives behind the page is signed there by `auto`, with nothing set in Settings', async ({
	page,
	context
}) => {
	const key = await passkey();
	await denyOffOrigin(page);
	await stubJsonRpc(page, /stub-rpc\.test\/rpc\/(\d+)/, (method) => {
		if (method === 'eth_chainId') return '0x1';
		if (method === 'eth_blockNumber') return '0x10';
		if (method === 'eth_getCode') return '0x6080';
		return undefined;
	});
	await stubRelay(page, RELAY, happyRelay('0x' + '11'.repeat(32), '0x' + '22'.repeat(32)));
	await page.addInitScript(
		(seed) => {
			localStorage.setItem('vela.intro.seen', String(Date.now()));
			localStorage.setItem('vela.dev.console', '1');
			localStorage.setItem(
				'vela.accounts',
				JSON.stringify([
					{
						id: seed.credentialHex,
						name: 'Behind a page',
						address: seed.safe,
						public_key_hex: seed.publicKeyHex,
						created_at_iso: '2026-01-01T00:00:00.000Z',
						keys: [
							{
								credential_id: seed.credentialHex,
								public_key_hex: seed.publicKeyHex,
								name: 'Behind a page',
								transports: '',
								signer_origin: seed.origin
							}
						]
					}
				])
			);
			localStorage.setItem('vela.activeAccountIndex', '0');
		},
		{ ...key, safe: SAFE, origin: signerUrl.replace(/\/$/, '') }
	);

	await page.goto('/en/wallet');
	await expect(page.getByText('Behind a page').first()).toBeVisible();
	// Nothing was chosen anywhere: the preference is the factory's `auto`.
	expect(await readKv(page, 'vela.signMethod')).toBeNull();
	await fire(page, 'personal_sign', ['0x68656c6c6f', SAFE]);

	const popup = await slideToClearSigner(page, context);
	// The key's OWN page, and it is the one Settings never named.
	expect(popup.url()).toBe(`${signerUrl}sign.html?ch=post`);
	await holdPasskey(popup, key);
	await signOnPage(popup);

	await expect.poll(() => answerOf(page), { timeout: 30_000 }).toMatch(/^0x[0-9a-f]{200,}$/);
});
