/**
 * §38 (review 0-5): the web wallet opens the signing page FROM THE BYTES ITS
 * CHECK HASHED — never by asking the host a second time.
 *
 * Before, the wallet fetched `/b/<h>/sign`, hashed it, and then had the
 * browser open that URL: a host that answers the check with the published
 * page and the window with another one was caught by nothing. Now, where the
 * wallet's own origin can use the account's keys (the core's
 * `CheckedPage::opens_from_bytes`), the window is a `blob:` URL made from the
 * very bytes that were hashed.
 *
 * Here the page is the locally built one at the front of `FROM_BYTES`
 * (`app-web/trusted-signer/dist`), its host a loopback address the run routes
 * (so the account is a `localhost` one, as the device harness's page is),
 * and its passkey a virtual authenticator on the page's own window:
 *
 * - a person signs a message there end to end — the page runs under the
 *   wallet's policy (its script's hash is in the wallet's CSP), asks the
 *   passkey for `localhost` from the blob's origin, and the core accepts the
 *   answer;
 * - a host that serves the published page once and other bytes afterwards
 *   changes nothing: the window was never one of its requests.
 */
import { createECDH, createPrivateKey } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test, type Page, type Route } from '@playwright/test';
import { denyOffOrigin, happyRelay, stubJsonRpc, stubRelay } from './stub-chain';

test.use({ viewport: { width: 390, height: 844 } });
test.setTimeout(120_000);

const ROOT = join(import.meta.dirname, '..', '..', '..');
const PAGE_HOST = 'http://localhost:5199';
const RELAY = /vela-relay(-cf)?\.getvela\.app/;
const ADDRESS = '0xD400866e00B055B20752a826CD5C89b811de130b';
/** Parallel One's throwaway test key (`src/lib/dev/passkey-fixture.ts`). */
const KEY = 'd80133c59ce0943689a9c1ff6006242c27b19412439fbc88f94feb5ca1e802d5';
const CREDENTIAL = Buffer.from('vela-fixture-01');

/** The newest page the core opens from its bytes: `integrity::FROM_BYTES[0]`. */
function newestFromBytes(): string {
	const source = readFileSync(
		join(ROOT, 'rust/crates/vela-core/src/trusted_signer/integrity.rs'),
		'utf8'
	);
	const block = /pub const FROM_BYTES: &\[&str\] =\s*&\[([^\]]*)\]/.exec(source)?.[1] ?? '';
	const first = /"([0-9a-f]{64})"/.exec(block)?.[1];
	if (!first) throw new Error('integrity.rs names no FROM_BYTES page');
	return first;
}

const VERSION = newestFromBytes();
const PAGE = readFileSync(join(ROOT, `app-web/trusted-signer/dist/b/${VERSION}/sign.html`));

function publicKeyHex(): string {
	const ecdh = createECDH('prime256v1');
	ecdh.setPrivateKey(Buffer.from(KEY, 'hex'));
	return ecdh.getPublicKey().toString('hex');
}

function keyPkcs8(): string {
	const pub = Buffer.from(publicKeyHex(), 'hex');
	const b64 = (b: Buffer) => b.toString('base64url');
	const key = createPrivateKey({
		key: {
			kty: 'EC',
			crv: 'P-256',
			d: b64(Buffer.from(KEY, 'hex')),
			x: b64(pub.subarray(1, 33)),
			y: b64(pub.subarray(33, 65))
		},
		format: 'jwk'
	});
	return (key.export({ type: 'pkcs8', format: 'der' }) as Buffer).toString('base64');
}

/** A signed-in `localhost` account whose venue is the loopback page. */
async function seed(page: Page): Promise<void> {
	const credential = CREDENTIAL.toString('hex');
	await page.addInitScript(
		([key, address, id, venue]) => {
			localStorage.setItem('vela.intro.seen', String(Date.now()));
			localStorage.setItem('vela.dev.console', '1');
			localStorage.setItem(
				'vela.accounts',
				JSON.stringify([
					{
						id,
						name: 'E2E Wallet',
						address,
						public_key_hex: key,
						created_at_iso: '2026-01-01T00:00:00.000Z',
						keys: [
							{ credential_id: id, public_key_hex: key, name: 'E2E Wallet', transports: 'internal' }
						],
						sign_in_key: { credential_id: id, method: 'platform', transports: 'internal' },
						signing_domain: 'localhost',
						signing_venue: { type: 'page', url: venue }
					}
				])
			);
			localStorage.setItem('vela.activeAccountIndex', '0');
		},
		[publicKeyHex(), ADDRESS, credential, `${PAGE_HOST}/`] as const
	);
}

/**
 * The page's host: `index.json` and the version's bytes, with CORS for the
 * wallet's check. `served(n)` is what the n-th request for the page gets.
 */
async function serveHost(page: Page, served: (n: number) => Buffer): Promise<string[]> {
	const asked: string[] = [];
	let n = 0;
	const cors = { 'access-control-allow-origin': '*' };
	await page.context().route(`${PAGE_HOST}/**`, (route: Route) => {
		const url = new URL(route.request().url());
		asked.push(url.pathname);
		if (url.pathname === '/index.json') {
			return route.fulfill({ json: { versions: [VERSION] }, headers: cors });
		}
		if (url.pathname === `/b/${VERSION}/sign.html`) {
			return route.fulfill({
				body: served(n++),
				headers: { ...cors, 'content-type': 'text/html; charset=utf-8' }
			});
		}
		return route.fulfill({ status: 404, headers: cors });
	});
	return asked;
}

async function openWallet(page: Page): Promise<void> {
	await denyOffOrigin(page);
	await stubJsonRpc(page, /stub-rpc\.test\/rpc\/(\d+)/, (method) => {
		if (method === 'eth_chainId') return '0x1';
		if (method === 'eth_blockNumber') return '0x10';
		if (method === 'eth_getCode') return '0x6080';
		return undefined;
	});
	await stubRelay(page, RELAY, happyRelay('0x' + '11'.repeat(32), '0x' + '22'.repeat(32)));
	await page.goto('/en/wallet');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await page.waitForFunction(
		() => (window as unknown as { vela?: { requester?: unknown } }).vela?.requester !== undefined,
		null,
		{ timeout: 20_000 }
	);
}

/** Ask for a signature as a site would; the answer lands on `window.__answer` / `__error`. */
async function askToSign(page: Page): Promise<void> {
	await page.evaluate((address) => {
		const w = window as unknown as {
			vela: { requester: { fire(method: string, params: unknown[]): Promise<unknown> } };
			__answer?: unknown;
			__error?: unknown;
		};
		void w.vela.requester
			.fire('personal_sign', ['0x68656c6c6f', address])
			.then((result) => (w.__answer = result))
			.catch((error) => (w.__error = { code: error?.code, message: String(error?.message) }));
	}, ADDRESS);
}

/** Confirm in the wallet, then Open on the hand-off card: the page's window. */
async function openPage(page: Page): Promise<Page> {
	const confirm = page.getByTestId('signing-confirm');
	await expect(confirm).toBeVisible({ timeout: 30_000 });
	await confirm.click();
	const card = page.getByTestId('page-handoff');
	await expect(card).toHaveAttribute('data-phase', 'ready', { timeout: 30_000 });
	const opened = page.context().waitForEvent('page');
	await card.getByRole('button').first().click();
	const signer = await opened;
	await signer.waitForLoadState('domcontentloaded');
	return signer;
}

async function holdTheKey(signer: Page): Promise<void> {
	const cdp = await signer.context().newCDPSession(signer);
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
			credentialId: CREDENTIAL.toString('base64'),
			isResidentCredential: true,
			rpId: 'localhost',
			privateKey: keyPkcs8(),
			userHandle: CREDENTIAL.toString('base64'),
			signCount: 0
		}
	});
}

async function pageCard(signer: Page): Promise<void> {
	await signer.waitForFunction(
		() => (window as unknown as { __velaState?: { phase: string } }).__velaState?.phase === 'card',
		null,
		{ timeout: 30_000 }
	);
}

test.describe('the signing page, opened from the bytes the wallet checked', () => {
	// The virtual authenticator is Chromium's (the DevTools protocol).
	test.skip(({ browserName }) => browserName !== 'chromium', 'WebAuthn stands in through CDP');

	test('a message is signed on the page the wallet hashed, end to end', async ({ page }) => {
		await seed(page);
		const asked = await serveHost(page, () => PAGE);
		await openWallet(page);
		await askToSign(page);
		const signer = await openPage(page);

		// The window is the checked bytes, in the wallet's own origin.
		expect(signer.url().startsWith(`blob:${new URL(page.url()).origin}/`)).toBe(true);
		expect(signer.url().endsWith(`#ch=post&lang=en&b=${VERSION}`)).toBe(true);
		expect(await signer.evaluate(() => self.origin)).toBe(new URL(page.url()).origin);
		await holdTheKey(signer);
		await pageCard(signer);
		// It names the version the wallet hashed.
		await expect(signer.locator('#trust')).toContainText(VERSION.slice(0, 8));

		const slider = await signer.evaluateHandle(
			() => (window as unknown as { __slider: HTMLElement }).__slider
		);
		await (slider.asElement() as NonNullable<ReturnType<typeof slider.asElement>>).click();

		await page.waitForFunction(
			() => {
				const w = window as unknown as { __answer?: unknown; __error?: unknown };
				return w.__answer !== undefined || w.__error !== undefined;
			},
			null,
			{ timeout: 30_000 }
		);
		const outcome = await page.evaluate(() => {
			const w = window as unknown as { __answer?: unknown; __error?: unknown };
			return { answer: w.__answer, error: w.__error };
		});
		expect(outcome.error).toBeUndefined();
		expect(String(outcome.answer)).toMatch(/^0x[0-9a-f]+$/);
		// One request for the page's bytes: the check's. The window made none.
		expect(asked.filter((path) => path.startsWith('/b/'))).toEqual([`/b/${VERSION}/sign.html`]);
	});

	test('a host that changes the page after the check changes nothing that opens', async ({
		page
	}) => {
		await seed(page);
		const tampered = Buffer.from(
			'<!doctype html><html><body><p id="evil">not the page that was checked</p></body></html>'
		);
		const asked = await serveHost(page, (n) => (n === 0 ? PAGE : tampered));
		await openWallet(page);
		await askToSign(page);
		const signer = await openPage(page);
		await pageCard(signer);
		expect(await signer.locator('#evil').count()).toBe(0);
		await expect(signer.locator('#trust')).toContainText(VERSION.slice(0, 8));
		// Reloading the window runs the same checked bytes — it never asks the host.
		await signer.reload();
		await expect(signer.locator('#trust')).toContainText(VERSION.slice(0, 8));
		expect(await signer.locator('#evil').count()).toBe(0);
		expect(asked.filter((path) => path.startsWith('/b/'))).toEqual([`/b/${VERSION}/sign.html`]);
	});
});
