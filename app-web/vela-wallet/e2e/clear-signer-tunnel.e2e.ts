/**
 * Creating a wallet with the Clear Signer on ANOTHER device (spec 075 US4).
 *
 * Two browser pages stand in for two devices: the wallet, and the signer page
 * opened from the pairing link. Between them runs the page's own mock relay
 * (`app-web/clearsigning/samples/mock-relay.mjs`, written against
 * `contracts/relay.md` §1) — dumb and blind, forwarding frames it cannot read.
 *
 * What this pins is the part no unit test can: the person's two looks. The
 * wallet draws a link (and its QR) and waits; the page joins the room, the two
 * ends derive the same six digits without the relay learning anything; and
 * nothing is sent until the person confirms those digits on the wallet. Then a
 * real `navigator.credentials.create()` runs on the other device, and its key
 * and member proof come back sealed, one after the other, on the same room.
 */
import { expect, test, type Page } from '@playwright/test';
import { en } from './live-helpers';
import { denyOffOrigin, stubJsonRpc, stubRelay, happyRelay } from './stub-chain';
import {
	answerWhere,
	holdAuthenticator,
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

interface MockRelay {
	url: string;
	tap: { room: string; from: string; isBinary: boolean; data: Buffer | string }[];
	close(): Promise<void>;
}

let signer: ServedPage;
let relay: MockRelay;

test.beforeAll(async () => {
	signer = await serveSignerPage();
	// The page's own mock, by path: it is plain ESM beside the page, with no
	// types, and this suite only ever reads from `app-web/clearsigning`.
	const module = (await import(
		/* @vite-ignore */ new URL('../../clearsigning/samples/mock-relay.mjs', import.meta.url).href
	)) as { startRelay(options?: { port?: number }): Promise<MockRelay> };
	relay = await module.startRelay();
});

test.afterAll(async () => {
	signer.close();
	await relay.close();
});

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

test('a wallet is created on another device, through the relay, after the codes are compared', async ({
	page,
	context
}) => {
	const registry = registryStub();
	await denyOffOrigin(page);
	await stubJsonRpc(page, /stub-rpc\.test\/rpc\/(\d+)/, (method) =>
		method === 'eth_chainId' ? '0x1' : undefined
	);
	await stubRelay(page, RELAY, happyRelay('0x' + '11'.repeat(32), '0x' + '22'.repeat(32)));
	await stubRegistry(page, context, registry);
	// The page this person uses, and the relay they pair through — both as
	// Settings would have written them.
	await seedKv(page, {
		'vela.clearSignerUrl': signer.url,
		'vela.clearSignerRelay': relay.url
	});
	await page.addInitScript(() => {
		localStorage.setItem('vela.intro.seen', String(Date.now()));
	});

	await page.goto('/en/create');
	await page.getByRole('textbox').first().fill('Across Devices');
	// The three gates. The native checkbox is under its own box, so the tap
	// lands where a person's does — on the box, never on row 3's policy links.
	const acks = page.locator('label.row .box');
	for (let index = 0; index < 3; index++) await acks.nth(index).click();
	await page.getByRole('button', { name: en('onboarding.create.nextBtn') }).click();

	await page.locator('button.method').nth(3).click();
	await answerWhere(page, 'other');

	// The pairing sheet: the link, its code to scan, and the waiting line.
	await expect(page.getByText(en('componentsUi.signing.clearSignerPair'))).toBeVisible({
		timeout: 30_000
	});
	await expect(page.getByText(en('componentsUi.signing.clearSignerPairWaiting'))).toBeVisible();
	// A real QR of a real link — the receive screen's encoder, not a pattern.
	const modules = await page.locator('.sheet svg[role="img"] path').getAttribute('d');
	expect((modules ?? '').length).toBeGreaterThan(200);
	const link = (await page.locator('.sheet .link').textContent())?.trim() ?? '';
	expect(link.startsWith(`${signer.url}sign.html?ch=relay#`)).toBe(true);
	const fragment = new URLSearchParams(new URL(link).hash.slice(1));
	expect(fragment.get('relay')).toBe(relay.url);
	expect(fragment.get('room')).toMatch(/^[A-Za-z0-9_-]{22}$/);
	expect(fragment.get('rk')).toMatch(/^[A-Za-z0-9_-]{22}$/);

	// The other device opens it.
	const other = await context.newPage();
	await other.goto(link);
	const authenticator = await holdAuthenticator(other);
	expect(authenticator.authenticatorId).not.toBe('');

	// Both screens show the same six digits, derived end to end.
	await other.waitForFunction(
		() => !!(window as unknown as { __velaState?: { code?: string } }).__velaState?.code,
		null,
		{ timeout: 60_000 }
	);
	const theirs = await other.evaluate(
		() => (window as unknown as { __velaState: { code: string } }).__velaState.code
	);
	const digits = page.locator('.sheet .digits');
	await expect(digits).toBeVisible({ timeout: 30_000 });
	const ours = (await digits.textContent())?.match(/\d{6}/)?.[0];
	expect(ours).toBe(theirs);

	// Nothing has been sent yet: the page is still waiting, not holding a card.
	expect(
		await other.evaluate(
			() => (window as unknown as { __velaState: { phase: string } }).__velaState.phase
		)
	).toBe('waiting');

	await page
		.getByRole('button', { name: en('componentsUi.signing.clearSignerCodeConfirm') })
		.click();

	// 1. the create, on the other device.
	await pageShows(other, 'create');
	await expect(other.getByText('Across Devices').first()).toBeVisible();
	await signOnPage(other);

	// 2. the member proof, in the same room.
	await pageShows(other, 'memberProof');
	await signOnPage(other);

	const create = page.getByRole('button', { name: en('onboarding.create.createWalletBtn') });
	await expect(create).toBeEnabled({ timeout: 60_000 });
	await create.click();
	await expect(page.getByText(en('onboarding.create.successTitle'))).toBeVisible({
		timeout: 120_000
	});

	// The relay saw two hellos in the clear and nothing else — no intent, no
	// answer, no key.
	const clear = relay.tap.filter((frame) => !frame.isBinary);
	expect(clear).toHaveLength(2);
	expect(clear.every((frame) => JSON.parse(String(frame.data)).t === 'hello')).toBe(true);
	const sealed = relay.tap.filter((frame) => frame.isBinary);
	expect(sealed.length).toBeGreaterThanOrEqual(4);
	expect(
		sealed.every((frame) => {
			const text = Buffer.from(frame.data).toString('latin1');
			return !text.includes('vela_') && !text.includes('registration');
		})
	).toBe(true);

	// And the key remembers the page it was minted behind.
	const stored = await page.evaluate(() => localStorage.getItem('vela.accounts'));
	const accounts = JSON.parse(stored ?? '[]') as { keys: { signer_origin?: string }[] }[];
	expect(accounts[0].keys[0].signer_origin).toBe(signer.origin);
});
