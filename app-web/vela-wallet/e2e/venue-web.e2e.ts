/**
 * Spec 102 on the web — where an account reviews and signs (P2-09, P2-11,
 * D-16, P2b-W1), since the owner made the signing page usable everywhere
 * (2026-10-30: every signature of an account whose venue is a page goes
 * there — `signing_venue::signing_route`):
 *
 * - an account on a custom signing domain is handed to ITS OWN page: a
 *   request reaches the sheet, the person confirms, and the hand-off card
 *   names that page — never a passkey prompt in Vela. Here the page cannot
 *   be fetched (the run reaches no other origin), so its check fails and the
 *   card says so: it is not opened;
 * - Settings' "How you sign" is a choice: the account's own venue is marked,
 *   R1 alone disables what its keys cannot reach, and the screen says what a
 *   page protects against and what it does not (`settings.venue.pageProtects`
 *   / `pageLimit`) — never the round-5 "multisig approvals only" line.
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
	test(`${locale}: a custom-domain account is handed to its own page — never a passkey in Vela`, async ({
		page
	}) => {
		await seed(page, 'sign.example.com', { type: 'page', url: OWN_PAGE });
		await openWallet(page, locale);
		await fire(page, 'personal_sign', ['0x68656c6c6f', ADDRESS]);

		const confirm = page.getByTestId('signing-confirm');
		await expect(confirm).toBeVisible({ timeout: 25_000 });
		await confirm.click();

		// The hand-off card, for this account's own page.
		const card = page.getByTestId('page-handoff');
		await expect(card).toHaveAttribute('data-kind', 'signature', { timeout: 15_000 });
		await expect(card).toContainText('sign.example.com');
		// The page could not be checked here (no other origin answers this
		// run): it is refused, not opened.
		await expect(card).toHaveAttribute('data-phase', 'refused', { timeout: 20_000 });
		// No passkey was ever asked for in Vela, and the site is not answered
		// behind the person's back.
		expect(
			await page.evaluate(() => (window as unknown as { __passkeyAsks: number }).__passkeyAsks)
		).toBe(0);
		expect(await pageError(page)).toBeNull();
	});
}

test('Settings: a getvela.app account chooses how it signs; the page says what a page protects and what not', async ({
	page
}) => {
	await seed(page, 'getvela.app', { type: 'page', url: OFFICIAL_PAGE });
	await denyOffOrigin(page);
	await page.goto('/en/settings');
	const row = page.getByRole('button', { name: new RegExp(corpus('en', 'settings.venue.title')) });
	await expect(row).toBeVisible({ timeout: 20_000 });
	// Owner, 2026-10-30: the page, everywhere — the account's own choice is shown.
	await expect(row).toContainText(corpus('en', 'settings.venue.page'));
	await row.click();

	const radios = page.getByRole('radio');
	await expect(radios).toHaveCount(2);
	await expect(radios.nth(0)).toContainText(corpus('en', 'settings.venue.inVela'));
	await expect(radios.nth(0)).toHaveAttribute('aria-checked', 'false');
	await expect(radios.nth(1)).toContainText(corpus('en', 'settings.signing.pageOfficial'));
	await expect(radios.nth(1)).toHaveAttribute('aria-checked', 'true');
	// A choice now: neither row is disabled, and the web's old reason is gone.
	await expect(radios.nth(0)).toBeEnabled();
	await expect(radios.nth(1)).toBeEnabled();
	await expect(radios.nth(1)).not.toContainText(corpus('en', 'settings.venue.blockedWeb'));
	// What a page protects against, and what it does not.
	await expect(page.getByText(corpus('en', 'settings.venue.pageProtects'))).toBeVisible();
	await expect(
		page.getByText(corpus('en', 'settings.venue.pageLimit').replace('{{domain}}', 'getvela.app'))
	).toBeVisible();
});

test('Settings: a custom-domain account keeps its own page; Vela and the official page say why not', async ({
	page
}) => {
	await seed(page, 'sign.example.com', { type: 'page', url: OWN_PAGE });
	await denyOffOrigin(page);
	await page.goto('/en/settings');
	const row = page.getByRole('button', { name: new RegExp(corpus('en', 'settings.venue.title')) });
	await expect(row).toBeVisible({ timeout: 20_000 });
	await expect(row).toContainText(corpus('en', 'settings.venue.page'));
	await row.click();

	const radios = page.getByRole('radio');
	await expect(radios).toHaveCount(3);
	// R1 alone decides what can be picked: the account's keys answer only on
	// its own domain — Vela's sheet and the official page say why not.
	await expect(radios.nth(0)).toBeDisabled();
	await expect(radios.nth(0)).toContainText('sign.example.com');
	await expect(radios.nth(1)).toBeDisabled();
	await expect(radios.nth(2)).toHaveAttribute('aria-checked', 'true');
	await expect(radios.nth(2)).toContainText(
		corpus('en', 'settings.signing.pageSelfHosted').replace('{{domain}}', 'sign.example.com')
	);
	await expect(radios.nth(2)).not.toContainText(corpus('en', 'settings.venue.blockedWeb'));
	await expect(page.getByText(corpus('en', 'settings.venue.pageProtects'))).toBeVisible();
	await expect(
		page.getByText(
			corpus('en', 'settings.venue.pageLimit').replace('{{domain}}', 'sign.example.com')
		)
	).toBeVisible();
});
