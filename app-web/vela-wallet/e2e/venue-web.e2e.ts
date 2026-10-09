/**
 * Spec 102 on the web — where an account reviews and signs, when the shell
 * opens no signing page (P2-09, P2-11, D-16, P2b-W1).
 *
 * The web is Vela's own sheet and nothing else. So:
 *
 * - an account on a custom signing domain cannot sign here: its keys answer
 *   only on its own page. A request reaches the sheet, the person confirms,
 *   and the refusal is said on the sheet in the core's words — "Signing pages
 *   open from the Vela apps, not the web." — in the person's language, with
 *   no passkey prompt, no "try again", and the page answered -32603 once the
 *   sheet is closed;
 * - Settings' "Where you review and sign" is a statement, not a choice: for a
 *   `getvela.app` account reviewing on the official page, Vela's sheet is
 *   marked (that is where the web signs it) and the page row is shown,
 *   disabled, with the web's reason; for a custom-domain account nothing is
 *   marked and the row under the account says why.
 */
import { expect, test, type Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { denyOffOrigin, happyRelay, stubJsonRpc, stubRelay } from './stub-chain';

test.use({ viewport: { width: 390, height: 844 } });
test.setTimeout(90_000);

const RELAY = /vela-relay(-cf)?\.getvela\.app/;
const ADDRESS = '0xD400866e00B055B20752a826CD5C89b811de130b';
const PUBLIC_KEY =
	'04197db9030a1e166bec2cee05e0ddb94b26ee0b6d6f429f1748cda4eedac36f04fe546861a9c9dfaf75719b53c75e0b933d4aad6d325f18c75776a260d507647b';
const OFFICIAL_PAGE = 'https://sign.getvela.app/';
const OWN_PAGE = 'https://sign.example.com/';

/** The generated corpus, read as the page reads it — assertions speak its words. */
function corpus(locale: 'en' | 'zh', path: string): string {
	const raw = JSON.parse(
		readFileSync(
			join(import.meta.dirname, '..', '..', '..', 'assets', 'i18n', `${locale}.json`),
			'utf8'
		)
	) as Record<string, unknown>;
	const value = path
		.split('.')
		.reduce<unknown>(
			(node, key) =>
				node !== null && typeof node === 'object'
					? (node as Record<string, unknown>)[key]
					: undefined,
			raw
		);
	if (typeof value !== 'string') throw new Error(`${locale} corpus has no string at ${path}`);
	return value;
}

/** A signed-in account on `domain`, reviewing at `venue`, with the dev gate on. */
async function seed(page: Page, domain: string, venue: unknown): Promise<void> {
	await page.addInitScript(
		([key, address, signingDomain, signingVenue]) => {
			localStorage.setItem('vela.intro.seen', String(Date.now()));
			localStorage.setItem('vela.dev.console', '1');
			localStorage.setItem(
				'vela.accounts',
				JSON.stringify([
					{
						id: 'e2e-credential-id',
						name: 'E2E Wallet',
						address,
						public_key_hex: key,
						created_at_iso: '2026-01-01T00:00:00.000Z',
						keys: [
							{
								credential_id: 'e2e-credential-id',
								public_key_hex: key,
								name: 'E2E Wallet',
								transports: 'internal'
							}
						],
						sign_in_key: {
							credential_id: 'e2e-credential-id',
							method: 'platform',
							transports: 'internal'
						},
						signing_domain: signingDomain,
						signing_venue: signingVenue
					}
				])
			);
			localStorage.setItem('vela.activeAccountIndex', '0');
			// Count every passkey prompt: a refused venue must open none.
			const counter = window as unknown as { __passkeyAsks: number };
			counter.__passkeyAsks = 0;
			const credentials = navigator.credentials;
			if (credentials) {
				const get = credentials.get.bind(credentials);
				credentials.get = (options?: CredentialRequestOptions) => {
					counter.__passkeyAsks += 1;
					return get(options);
				};
			}
		},
		[PUBLIC_KEY, ADDRESS, domain, venue] as const
	);
}

async function openWallet(page: Page, locale: 'en' | 'zh'): Promise<void> {
	await denyOffOrigin(page);
	await stubJsonRpc(page, /stub-rpc\.test\/rpc\/(\d+)/, (method) => {
		if (method === 'eth_chainId') return '0x1';
		if (method === 'eth_blockNumber') return '0x10';
		if (method === 'eth_getCode') return '0x6080';
		return undefined;
	});
	await stubRelay(page, RELAY, happyRelay('0x' + '11'.repeat(32), '0x' + '22'.repeat(32)));
	await page.goto(`/${locale}/wallet`);
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await page.waitForFunction(
		() => (window as unknown as { vela?: { requester?: unknown } }).vela?.requester !== undefined,
		null,
		{ timeout: 20_000 }
	);
}

/** Fire a request from the page; its answer lands on `window.__answer` / `__error`. */
async function fire(page: Page, method: string, params: unknown[]): Promise<void> {
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

const pageError = (page: Page) =>
	page.evaluate(() => {
		const error = (window as unknown as { __error?: { code?: number; message?: string } }).__error;
		return error === undefined ? null : { code: error.code, message: error.message };
	});

for (const locale of ['en', 'zh'] as const) {
	test(`${locale}: a custom-domain account is refused on the sheet, in the person’s words`, async ({
		page
	}) => {
		await seed(page, 'sign.example.com', { type: 'page', url: OWN_PAGE });
		await openWallet(page, locale);
		await fire(page, 'personal_sign', ['0x68656c6c6f', ADDRESS]);

		const confirm = page.getByTestId('signing-confirm');
		await expect(confirm).toBeVisible({ timeout: 25_000 });
		await confirm.click();

		// The core's reason, in this locale — and the failed title over it.
		const reason = corpus(locale, 'settings.venue.blockedWeb');
		await expect(page.getByText(reason)).toBeVisible({ timeout: 15_000 });
		await expect(
			page.getByText(corpus(locale, 'componentsTx.receipt.statusFailed'), { exact: true }).first()
		).toBeVisible();
		// Not something trying again would change.
		await expect(page.getByRole('button', { name: corpus(locale, 'send.txRetryBtn') })).toHaveCount(
			0
		);
		// No passkey was ever asked for.
		expect(
			await page.evaluate(() => (window as unknown as { __passkeyAsks: number }).__passkeyAsks)
		).toBe(0);
		// The page waits for the person to read it…
		expect(await pageError(page)).toBeNull();
		await page
			.getByRole('button', { name: corpus(locale, 'componentsTx.receipt.done'), exact: true })
			.click();
		// …then hears the core's sentence.
		await expect
			.poll(() => pageError(page), { timeout: 20_000 })
			.toEqual({
				code: -32603,
				message: 'This account cannot sign here'
			});
	});
}

test('Settings: a getvela.app account on the official page signs in Vela here; the page row says why not', async ({
	page
}) => {
	await seed(page, 'getvela.app', { type: 'page', url: OFFICIAL_PAGE });
	await denyOffOrigin(page);
	await page.goto('/en/settings');
	const row = page.getByRole('button', { name: new RegExp(corpus('en', 'settings.venue.title')) });
	await expect(row).toBeVisible({ timeout: 20_000 });
	await expect(row).toContainText(corpus('en', 'settings.venue.inVela'));
	await row.click();

	const radios = page.getByRole('radio');
	await expect(radios).toHaveCount(2);
	await expect(radios.nth(0)).toHaveAttribute('aria-checked', 'true');
	await expect(radios.nth(0)).toContainText(corpus('en', 'settings.venue.inVela'));
	await expect(radios.nth(1)).toHaveAttribute('aria-checked', 'false');
	await expect(radios.nth(1)).toBeDisabled();
	await expect(radios.nth(1)).toContainText(corpus('en', 'settings.signing.pageOfficial'));
	await expect(radios.nth(1)).toContainText(corpus('en', 'settings.venue.blockedWeb'));
	// A statement, not a choice: nothing is offered on Vela's own row either.
	await expect(radios.nth(0)).toHaveAttribute('aria-disabled', 'true');
});

test('Settings: a custom-domain account signs nowhere on the web, and the row says why', async ({
	page
}) => {
	await seed(page, 'sign.example.com', { type: 'page', url: OWN_PAGE });
	await denyOffOrigin(page);
	await page.goto('/en/settings');
	const reason = corpus('en', 'settings.venue.blockedWeb');
	const row = page.getByRole('button', { name: new RegExp(corpus('en', 'settings.venue.title')) });
	await expect(row).toBeVisible({ timeout: 20_000 });
	await expect(row).toContainText(reason);
	await row.click();

	const radios = page.getByRole('radio');
	await expect(radios).toHaveCount(3);
	for (let i = 0; i < 3; i += 1) {
		await expect(radios.nth(i)).toHaveAttribute('aria-checked', 'false');
		await expect(radios.nth(i)).toBeDisabled();
	}
	// R1's reason under Vela's sheet; the web's under the account's own page,
	// which is named as self-hosted, never as "my own".
	await expect(radios.nth(0)).toContainText('sign.example.com');
	await expect(radios.nth(2)).toContainText(
		corpus('en', 'settings.signing.pageSelfHosted').replace('{{domain}}', 'sign.example.com')
	);
	await expect(radios.nth(2)).toContainText(reason);
});
