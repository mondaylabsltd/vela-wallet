/**
 * The trusted signing page from the extension (owner, 2026-10-30: the page is
 * usable everywhere). An account whose venue is Vela's official page reviews
 * and signs a dApp's request THERE: the request window checks the page, opens
 * it, hands it the core's request, keeps the request alive while the page has
 * it, and answers the dApp with what the page signed — accepted only as the
 * core accepts it. Declined there (or the page closed unsigned), the dApp
 * hears 4001.
 *
 * The page is the locally built one (`app-web/trusted-signer/dist`, the
 * version at the front of `BUILD_ALLOWED`), served the way its host serves it
 * (`samples/serve-dist.mjs`: `_headers`, `/b/<hash>/sign`) over TLS on this
 * machine, with `sign.getvela.app` mapped to it — so the page is on
 * `getvela.app` and the fixture wallet's passkey (a virtual authenticator on
 * the page's tab, `rpId` getvela.app) is the same relying party the extension
 * signs for. The page answers the extension because Chrome reports the opener
 * as `chrome-extension://<the published id>` — the id this package's manifest
 * key derives.
 */
import { holdPress } from './stub-chain';
import { execFileSync } from 'node:child_process';
import { createECDH, createPrivateKey } from 'node:crypto';
import { createServer, type Server } from 'node:http';
import { createServer as createHttpsServer, type Server as HttpsServer } from 'node:https';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { expect, test, type BrowserContext, type Page } from '@playwright/test';
import {
	extensionBuilt,
	extensionId,
	loadExtension,
	noRequestWindow,
	requestWindow,
	confirmSigning
} from './extension-helpers';
// @ts-expect-error — a plain ESM sample of the page's own folder, no types.
import { handler as signerHost } from '../../trusted-signer/samples/serve-dist.mjs';

const APP_ROOT = join(import.meta.dirname, '..');
const PORT = 8819;
const TLS_PORT = 8846;
const FIXTURE_ONE = '0xD400866e00B055B20752a826CD5C89b811de130b';
/** Parallel One's throwaway test key (`src/lib/dev/passkey-fixture.ts`). */
const FIXTURE_ONE_KEY = 'd80133c59ce0943689a9c1ff6006242c27b19412439fbc88f94feb5ca1e802d5';
const FIXTURE_ONE_CREDENTIAL = Buffer.from('vela-fixture-01');
const MESSAGE_HEX = '0x48656c6c6f2c2056656c61';
const OFFICIAL = 'https://sign.getvela.app/';

interface AskResult {
	ok: boolean;
	code?: number;
	message?: string;
	result?: unknown;
}

function serveDApp(): Promise<Server> {
	const html = readFileSync(join(APP_ROOT, 'e2e/testdapp/index.html'));
	const server = createServer((_req, res) => {
		res.setHeader('content-type', 'text/html; charset=utf-8');
		res.end(html);
	});
	return new Promise((resolve) => server.listen(PORT, () => resolve(server)));
}

/** The signer's host, on this machine, over TLS with a throwaway certificate. */
function serveSigner(dir: string): Promise<HttpsServer> {
	execFileSync(
		'openssl',
		[
			'req',
			'-x509',
			'-newkey',
			'rsa:2048',
			'-nodes',
			'-days',
			'1',
			'-subj',
			'/CN=sign.getvela.app',
			'-keyout',
			join(dir, 'key.pem'),
			'-out',
			join(dir, 'cert.pem')
		],
		{ stdio: 'ignore' }
	);
	const server = createHttpsServer(
		{ key: readFileSync(join(dir, 'key.pem')), cert: readFileSync(join(dir, 'cert.pem')) },
		signerHost()
	);
	return new Promise((resolve) => server.listen(TLS_PORT, '127.0.0.1', () => resolve(server)));
}

/** Parallel One's key as the page's virtual authenticator holds it (PKCS#8). */
function fixtureKeyPkcs8(): string {
	const ecdh = createECDH('prime256v1');
	ecdh.setPrivateKey(Buffer.from(FIXTURE_ONE_KEY, 'hex'));
	const pub = ecdh.getPublicKey();
	const b64 = (b: Buffer) => b.toString('base64url');
	const key = createPrivateKey({
		key: {
			kty: 'EC',
			crv: 'P-256',
			d: b64(Buffer.from(FIXTURE_ONE_KEY, 'hex')),
			x: b64(pub.subarray(1, 33)),
			y: b64(pub.subarray(33, 65))
		},
		format: 'jwk'
	});
	return (key.export({ type: 'pkcs8', format: 'der' }) as Buffer).toString('base64');
}

async function seedWallet(context: BrowserContext, id: string): Promise<Page> {
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

/**
 * Settings → How you sign: Parallel One reviews and signs on Vela's official
 * signing page — the person's own choice, made where the person makes it.
 */
async function chooseThePage(wallet: Page, id: string): Promise<void> {
	await wallet.goto(`chrome-extension://${id}/en/settings.html`);
	await expect(wallet.getByText('How you sign').first()).toBeVisible({ timeout: 30_000 });
	await wallet.getByText("Vela's official signing page").first().click();
	await expect
		.poll(() =>
			wallet.evaluate((address) => {
				const list = JSON.parse(localStorage.getItem('vela.accounts') ?? '[]') as {
					address: string;
					signing_venue?: { type: string; url?: string };
				}[];
				return list.find((a) => a.address.toLowerCase() === address.toLowerCase())?.signing_venue;
			}, FIXTURE_ONE)
		)
		.toEqual({ type: 'page', url: OFFICIAL });
}

async function connect(context: BrowserContext, page: Page): Promise<void> {
	const asked = page.evaluate(() => window.__ask('eth_requestAccounts')) as Promise<AskResult>;
	const win = await requestWindow(context);
	await win.getByRole('button', { name: 'Connect' }).click();
	expect((await asked).result).toEqual([FIXTURE_ONE]);
	await noRequestWindow(context);
}

/**
 * The request window's sheet is confirmed; the hand-off card rises over it
 * with the page checked; Open opens the page in its own tab. Returns that tab,
 * holding the fixture key in a virtual authenticator.
 */
async function handOff(context: BrowserContext, win: Page): Promise<Page> {
	await confirmSigning(win);
	const card = win.getByTestId('page-handoff');
	await expect(card).toHaveAttribute('data-phase', 'ready', { timeout: 30_000 });
	await expect(card).toContainText('sign.getvela.app');
	const opened = context.waitForEvent('page');
	// R22 (D4): the card's Open is the confirm that raised it — held when it was.
	await holdPress(card.getByRole('button'));
	const signer = await opened;
	await signer.waitForURL(/sign\.getvela\.app\/b\/[0-9a-f]{64}\/sign\?ch=post/);
	const cdp = await context.newCDPSession(signer);
	await cdp.send('WebAuthn.enable');
	const { authenticatorId } = await cdp.send('WebAuthn.addVirtualAuthenticator', {
		options: {
			protocol: 'ctap2',
			transport: 'internal',
			hasResidentKey: true,
			hasUserVerification: true,
			isUserVerified: true
		}
	});
	await cdp.send('WebAuthn.addCredential', {
		authenticatorId,
		credential: {
			credentialId: FIXTURE_ONE_CREDENTIAL.toString('base64'),
			isResidentCredential: true,
			rpId: 'getvela.app',
			privateKey: fixtureKeyPkcs8(),
			userHandle: Buffer.from('vela-fixture-01').toString('base64'),
			signCount: 0
		}
	});
	await expect(card).toHaveAttribute('data-phase', 'waiting');
	await signer.waitForFunction(
		() => (window as unknown as { __velaState?: { phase: string } }).__velaState?.phase === 'card',
		null,
		{ timeout: 30_000 }
	);
	return signer;
}

/** What a person does on the page: tap its one button. */
async function confirmOnPage(signer: Page): Promise<void> {
	const button = await signer.evaluateHandle(
		() => (window as unknown as { __slider: HTMLElement }).__slider
	);
	await (button.asElement() as NonNullable<ReturnType<typeof button.asElement>>).click();
}

test.describe('the trusted signing page, from the extension', () => {
	test.skip(!extensionBuilt(), 'extension/dist is missing — run `bun run build:extension`');
	test.setTimeout(240_000);

	let dapp: Server;
	let signer: HttpsServer;
	let certs: string;
	test.beforeAll(async () => {
		certs = mkdtempSync(join(tmpdir(), 'vela-signer-tls-'));
		dapp = await serveDApp();
		signer = await serveSigner(certs);
	});
	test.afterAll(async () => {
		dapp.closeAllConnections();
		signer.closeAllConnections();
		await new Promise((resolve) => dapp.close(() => resolve(null)));
		await new Promise((resolve) => signer.close(() => resolve(null)));
		rmSync(certs, { recursive: true, force: true });
	});

	const args = [
		`--host-resolver-rules=MAP sign.getvela.app:443 127.0.0.1:${TLS_PORT}`,
		'--ignore-certificate-errors',
		'--no-proxy-server'
	];

	async function ready(): Promise<{ context: BrowserContext; site: Page }> {
		const context = await loadExtension({ args });
		const id = extensionId();
		const wallet = await seedWallet(context, id);
		await chooseThePage(wallet, id);
		const site = await context.newPage();
		await site.goto(`http://localhost:${PORT}/`);
		await connect(context, site);
		return { context, site };
	}

	test('personal_sign: reviewed and signed on the page, the dApp gets the signature', async () => {
		const { context, site } = await ready();
		const asked = site.evaluate(
			([message, address]) => window.__ask('personal_sign', [message, address]),
			[MESSAGE_HEX, FIXTURE_ONE] as const
		) as Promise<AskResult>;
		const win = await requestWindow(context);
		await expect(win.getByText('Hello, Vela')).toBeVisible({ timeout: 30_000 });
		const page = await handOff(context, win);
		// The page reads the message itself, and says whose word the site is.
		await expect(page.getByText('Hello, Vela').first()).toBeVisible();
		await expect(page.getByText("Which site asked is the wallet's word")).toBeVisible();
		await confirmOnPage(page);
		const answer = await asked;
		expect(answer.ok, JSON.stringify(answer)).toBe(true);
		// An EIP-1271 contract signature, built from the page's assertion.
		expect(String(answer.result)).toMatch(/^0x[0-9a-f]{200,}$/);
		await context.close();
	});

	test('eth_signTypedData_v4: the same, over the typed data the page decoded', async () => {
		const { context, site } = await ready();
		const typed = JSON.stringify({
			types: {
				EIP712Domain: [
					{ name: 'name', type: 'string' },
					{ name: 'chainId', type: 'uint256' }
				],
				Mail: [{ name: 'contents', type: 'string' }]
			},
			primaryType: 'Mail',
			domain: { name: 'Ether Mail', chainId: 1 },
			message: { contents: 'Hello from the dApp' }
		});
		const asked = site.evaluate(
			([address, data]) => window.__ask('eth_signTypedData_v4', [address, data]),
			[FIXTURE_ONE, typed] as const
		) as Promise<AskResult>;
		const win = await requestWindow(context);
		const page = await handOff(context, win);
		await expect(page.getByText('Hello from the dApp').first()).toBeVisible();
		await confirmOnPage(page);
		const answer = await asked;
		expect(answer.ok, JSON.stringify(answer)).toBe(true);
		expect(String(answer.result)).toMatch(/^0x[0-9a-f]{200,}$/);
		await context.close();
	});

	test('the page closed unsigned: the dApp hears 4001, nothing was signed', async () => {
		const { context, site } = await ready();
		const asked = site.evaluate(
			([message, address]) => window.__ask('personal_sign', [message, address]),
			[MESSAGE_HEX, FIXTURE_ONE] as const
		) as Promise<AskResult>;
		const win = await requestWindow(context);
		const page = await handOff(context, win);
		await page.close();
		const answer = await asked;
		expect(answer).toMatchObject({ ok: false, code: 4001 });
		await context.close();
	});
});
