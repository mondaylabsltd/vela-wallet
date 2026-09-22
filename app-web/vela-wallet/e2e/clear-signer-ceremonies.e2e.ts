/**
 * The Clear Signer as a PASSKEY ROUTE, end to end (spec 075 US1–US2).
 *
 * The page here is the real one (`app-web/clearsigning`), served on a second
 * localhost origin, and the passkey is a CDP virtual authenticator inside the
 * page's own window — so the page runs a real `navigator.credentials.create()`
 * and real `get()`s, and the wallet's core judges the real answers.
 *
 * Covered:
 * - creating a wallet through it: the key AND its member proof on ONE page
 *   visit, the challenge fetched by both ends from the same registry, and the
 *   stored record remembering which page the key lives behind;
 * - signing in again through it, on a browser that was never used to create
 *   anything, with the chooser offering it beside the three platform routes.
 *
 * The registry is a stand-in that derives the member challenge with the PAGE's
 * own code (`clear-signer-helpers.ts`), because a wallet and a page that
 * disagree about those 32 bytes is exactly the failure the design is built to
 * catch — a stand-in that got it wrong would pass this test for the wrong
 * reason.
 */
import { expect, test, type Page } from '@playwright/test';
import { en } from './live-helpers';
import { denyOffOrigin, stubJsonRpc, stubRelay, happyRelay } from './stub-chain';
import {
	answerWhere,
	holdAuthenticator,
	openMethodPicker,
	passkey,
	registryStub,
	seedKv,
	serveSignerPage,
	signOnPage,
	stubRegistry,
	type ServedPage
} from './clear-signer-helpers';

test.use({ viewport: { width: 390, height: 844 } });
test.setTimeout(180_000);

const RELAY = /vela-relay(-cf)?\.getvela\.app/;
const SAFE = '0xD400866e00B055B20752a826CD5C89b811de130b';

let signer: ServedPage;

test.beforeAll(async () => {
	signer = await serveSignerPage();
});

test.afterAll(() => {
	signer.close();
});

/** The chain, silent and stubbed: none of these tests is about a balance. */
async function quietChain(page: Page): Promise<void> {
	await denyOffOrigin(page);
	await stubJsonRpc(page, /stub-rpc\.test\/rpc\/(\d+)/, (method) => {
		if (method === 'eth_chainId') return '0x1';
		if (method === 'eth_blockNumber') return '0x10';
		if (method === 'eth_getCode') return '0x6080';
		return undefined;
	});
	await stubRelay(page, RELAY, happyRelay('0x' + '11'.repeat(32), '0x' + '22'.repeat(32)));
}

/** What the page is doing, as its own state machine reports it. */
function pageShows(popup: Page, kind: string): Promise<unknown> {
	return popup.waitForFunction(
		(want) => {
			const state = (window as unknown as { __velaState?: { phase?: string; kind?: string } })
				.__velaState;
			return state?.phase === 'card' && state?.kind === want;
		},
		kind,
		{ timeout: 60_000 }
	);
}

test('a wallet is created through the Clear Signer — key and member proof, one page visit', async ({
	page,
	context
}) => {
	const registry = registryStub();
	await quietChain(page);
	await stubRegistry(page, context, registry);
	// The page this device trusts. Chosen in Settings normally; there is no
	// Settings to visit before a wallet exists, so it is seeded where Settings
	// would have written it.
	await seedKv(page, { 'vela.clearSignerUrl': signer.url });
	await page.addInitScript(() => {
		localStorage.setItem('vela.intro.seen', String(Date.now()));
	});

	await page.goto('/en/create');
	await page.getByRole('textbox').first().fill('E2E Clear');
	// The three gates. The native checkbox is under its own box, so the tap
	// lands where a person's does — on the box, never on row 3's policy links.
	const acks = page.locator('label.row .box');
	for (let index = 0; index < 3; index++) await acks.nth(index).click();
	await page.getByRole('button', { name: en('onboarding.create.nextBtn') }).click();

	// The key chooser offers four routes now, the Clear Signer among them.
	const methods = page.locator('button.method');
	await expect(methods).toHaveCount(4);
	await expect(methods.nth(3)).toContainText(en('componentsUi.signing.clearSignerTitle'));

	const opened = context.waitForEvent('page', { timeout: 60_000 });
	await methods.nth(3).click();
	// Where is it? On this device — and that tap is the popup's activation.
	await answerWhere(page, 'this');
	const popup = await opened;
	expect(popup.url()).toBe(`${signer.url}sign.html?ch=post`);

	// The passkey lives in the page's window, not the wallet's.
	await holdAuthenticator(popup);

	// 1. the create card, on the page, naming the wallet.
	await pageShows(popup, 'create');
	await expect(popup.getByText('E2E Clear').first()).toBeVisible();
	await signOnPage(popup);

	// 2. the member proof, on the SAME page visit — no second window.
	await pageShows(popup, 'memberProof');
	expect(context.pages().filter((one) => one.url().includes('sign.html'))).toHaveLength(1);
	await signOnPage(popup);

	// Both ends asked the same registry for the same 32 bytes: the wallet
	// fetched them to hand to its core, the page to derive and compare.
	await expect
		.poll(() => registry.asked.filter((one) => one.path === '/api/challenge').length, {
			timeout: 30_000
		})
		.toBeGreaterThanOrEqual(2);
	const memberCalls = registry.asked.filter(
		(one) => one.path === '/api/challenge' && one.body?.publicKey !== undefined
	);
	expect(memberCalls.length).toBeGreaterThanOrEqual(2);
	// Same key, same group, same rpId — the page's, `localhost` here.
	expect(new Set(memberCalls.map((one) => String(one.body?.publicKey)))).toHaveProperty('size', 1);
	expect(new Set(memberCalls.map((one) => String(one.body?.rpId)))).toEqual(new Set(['localhost']));

	// The key row is drawn, confirmed, and the wallet can be created.
	const create = page.getByRole('button', { name: en('onboarding.create.createWalletBtn') });
	await expect(create).toBeEnabled({ timeout: 60_000 });
	await create.click();

	await expect(page.getByText(en('onboarding.create.successTitle'))).toBeVisible({
		timeout: 120_000
	});

	// The record remembers WHERE the key lives: without it, the next signature
	// would be aimed at a platform sheet that cannot see this key at all.
	const stored = await page.evaluate(() => localStorage.getItem('vela.accounts'));
	const accounts = JSON.parse(stored ?? '[]') as {
		name: string;
		keys: { signer_origin?: string }[];
	}[];
	expect(accounts).toHaveLength(1);
	expect(accounts[0].name).toBe('E2E Clear');
	expect(accounts[0].keys[0].signer_origin).toBe(signer.origin);
});

test('signing in again goes to the Clear Signer, on a browser that never created anything', async ({
	page,
	context
}) => {
	const key = await passkey();
	await quietChain(page);
	await seedKv(page, { 'vela.clearSignerUrl': signer.url });
	// The wallet this person has, as this device remembers it — its one key
	// living behind the signer page.
	await page.addInitScript(
		(seed) => {
			localStorage.setItem('vela.intro.seen', String(Date.now()));
			localStorage.setItem(
				'vela.accounts',
				JSON.stringify([
					{
						id: seed.credentialHex,
						name: 'E2E Wallet',
						address: seed.safe,
						public_key_hex: seed.publicKeyHex,
						created_at_iso: '2026-01-01T00:00:00.000Z',
						keys: [
							{
								credential_id: seed.credentialHex,
								public_key_hex: seed.publicKeyHex,
								name: 'E2E Wallet',
								transports: '',
								signer_origin: seed.origin
							}
						]
					}
				])
			);
			localStorage.setItem('vela.activeAccountIndex', '0');
		},
		{ ...key, safe: SAFE, origin: signer.origin }
	);

	await page.goto('/en');
	// The sign-in sheet is the same chooser: four routes, the Clear Signer last.
	await openMethodPicker(page, en('onboarding.welcome.alreadyHaveWallet'));
	const methods = page.locator('button.method');
	const opened = context.waitForEvent('page', { timeout: 60_000 });
	await methods.nth(3).click();
	await answerWhere(page, 'this');
	const popup = await opened;

	// The key is already on this device's authenticator — inside the PAGE's
	// window, which is the only place a Clear Signer ceremony can reach it.
	const authenticator = await holdAuthenticator(popup);
	await authenticator.addCredential({
		credentialIdBase64: key.credentialB64,
		privateKeyBase64: key.privateKeyB64,
		rpId: 'localhost',
		userHandle: 'E2E Wallet\u0000e2e'
	});

	await pageShows(popup, 'signIn');
	await signOnPage(popup);

	// The core resolved the account from the credential the page found, and the
	// wallet opened.
	await expect(page).toHaveURL(/\/en\/wallet/, { timeout: 60_000 });
	await expect(page.getByText('E2E Wallet').first()).toBeVisible({ timeout: 60_000 });
});
