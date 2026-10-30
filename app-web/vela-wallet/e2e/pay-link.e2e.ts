/**
 * A payment link, opened in the web wallet (spec 039 Part B).
 *
 * `/pay?to=…&chain=…&token=…&amount=…` — the link the receive screen shares,
 * and the one getvela.app's `/pay` forwards here — through the built Worker:
 * the root's hop to the locale's pay page, the core's ruling on the link, and
 * Send opened on it. Each refusal is asserted as a SENTENCE, the core's reason
 * in the corpus's words, because "this link is broken" and "this link's
 * amount was changed" are different things to go back to the sender with.
 *
 * Nothing here is ever signed: a link opens Send for the person to review.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test, type Page } from '@playwright/test';
import { CHAINS } from '../src/lib/services/chains';
import { en, seedSignedIn, zh } from './live-helpers';
import {
	abiWord,
	aggregate3CallCount,
	denyOffOrigin,
	encodeAggregate3Result,
	seedNetworkOverrides,
	stubChainRegistry,
	stubJsonRpc
} from './stub-chain';

test.use({ viewport: { width: 390, height: 844 } });
test.setTimeout(120_000);

const STUB = 'https://stub-rpc.test/rpc';
const BALANCE = 1_000_000_000_000_000_000n;
const PRICE_8DP = 3000n * 100_000_000n;
const ALICE = '0x52908400098527886E0F7030069857D2E4169EE7';
/** A well-formed address with no token behind it on Ethereum. */
const NO_TOKEN = '0x00000000000000000000000000000000deadbeef';
const REPO = join(import.meta.dirname, '..', '..', '..');

/** One chain answering: 1 ETH held, priced — and nothing at `NO_TOKEN`. */
async function stubChain(page: Page): Promise<void> {
	await denyOffOrigin(page);
	await stubChainRegistry(page, {
		1: {
			chainId: 1,
			name: 'Ethereum',
			nativeCurrency: { name: 'Ether', symbol: 'ETH', decimals: 18 },
			stables: [],
			wrappedNativeToken: null,
			dex: null,
			rpc: [`${STUB}/1`]
		}
	});
	await stubJsonRpc(page, /stub-rpc\.test\/rpc\/(\d+)/, (method, params, url) => {
		const chainId = Number(/\/rpc\/(\d+)/.exec(url)?.[1]);
		if (method === 'eth_chainId') return '0x' + chainId.toString(16);
		if (method === 'eth_blockNumber') return '0x10';
		if (method === 'eth_getCode') return '0x6080';
		if (method === 'eth_getBlockByNumber') return { baseFeePerGas: '0x3b9aca00' };
		if (method === 'eth_call') {
			const call = params[0] as { data?: string } | undefined;
			const n = call?.data ? aggregate3CallCount(call.data) : 0;
			if (n === 0) return '0x' + abiWord(0);
			// The token's symbol() and decimals(): no contract answers them.
			if (call?.data?.toLowerCase().includes(NO_TOKEN.slice(2))) {
				return encodeAggregate3Result(
					Array.from({ length: n }, () => ({ success: false, data: '0x' }))
				);
			}
			const balance = chainId === 1 ? BALANCE : 0n;
			const data =
				'0x' + abiWord(balance) + abiWord(PRICE_8DP) + abiWord(0) + abiWord(0) + abiWord(0);
			return encodeAggregate3Result(Array.from({ length: n }, () => ({ success: true, data })));
		}
		if (method === 'eth_getLogs') return [];
		return undefined;
	});
}

/** A returning person with a wallet on this device, its reads answered. */
async function walletOnDevice(page: Page, locale = 'en'): Promise<void> {
	await seedSignedIn(page);
	await stubChain(page);
	await page.goto(`/${locale}/wallet`);
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await seedNetworkOverrides(
		page,
		CHAINS.map((c) => ({ chainId: c.chainId, rpcURL: `${STUB}/${c.chainId}` }))
	);
}

/** Send, opened on the link: the payee, the amount, the network — and nothing signed. */
async function expectSendPrefilled(page: Page, amount: string): Promise<void> {
	await page.waitForURL(/\/en\/wallet/);
	const recipient = page.getByRole('textbox', { name: en('send.recipientLabel') });
	await expect(recipient).toHaveValue(ALICE, { timeout: 30_000 });
	await expect(page.getByRole('textbox', { name: 'ETH' })).toHaveValue(amount);
	await expect(page.getByText('Ethereum ·').first()).toBeVisible();
	// The person's next step is theirs: Continue to the review. No passkey
	// sheet, no confirm, no signing until they get there and say so.
	await expect(page.getByRole('button', { name: en('send.continueBtn') }).first()).toBeVisible();
}

test('a valid link opens Send on its payee, network and amount, for the person to review', async ({
	page
}) => {
	await walletOnDevice(page);
	// The root door, as a shared link names it: the Worker sends it on to the
	// locale's page with the query whole.
	const response = await page.request.get(`/pay?to=${ALICE}&chain=1`, { maxRedirects: 0 });
	expect(response.status()).toBe(307);
	expect(response.headers()['location']).toBe(`/en/pay?to=${ALICE}&chain=1`);

	await page.goto(`/pay?to=${ALICE}&chain=1&amount=0.5&sym=ETH&dec=18&net=Ethereum`);
	await expectSendPrefilled(page, '0.5');
	// Taken once: a reload is a plain visit to the wallet, not a second Send.
	expect(await page.evaluate(() => localStorage.getItem('vela.payKeptLink'))).toBeNull();
});

test('a tampered link is refused, says why in the core’s words, and offers the way home', async ({
	page
}) => {
	await walletOnDevice(page);

	// The amount, changed into hex: the core's grammar refuses it — and says
	// it is the AMOUNT, not "a recipient or network".
	await page.goto(`/pay?to=${ALICE}&chain=1&amount=0x10&sym=ETH&dec=18`);
	await expect(page.getByRole('heading', { name: en('receive.pay.invalidTitle') })).toBeVisible({
		timeout: 30_000
	});
	await expect(page.getByText(en('receive.pay.invalidAmount'))).toBeVisible();
	await expect(page.getByText(en('receive.pay.invalidBody'))).toHaveCount(0);
	// Nothing kept: a broken link is not a request.
	expect(await page.evaluate(() => localStorage.getItem('vela.payKeptLink'))).toBeNull();

	// The recipient, cut short.
	await page.goto(`/pay?to=${ALICE.slice(0, 30)}&chain=1&amount=1`);
	await expect(page.getByText(en('receive.pay.invalidBody'))).toBeVisible({ timeout: 30_000 });

	// Home is the wallet, and no Send opens there.
	await page.getByRole('link', { name: en('receive.pay.home') }).click();
	await page.waitForURL(/\/en\/wallet/);
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await expect(page.getByRole('textbox', { name: en('send.recipientLabel') })).toHaveCount(0);
});

test('a link on a network the wallet does not have: said, with the way to add it', async ({
	page
}) => {
	await walletOnDevice(page);
	// Linea — well-formed, so the link is accepted; whether THIS wallet has
	// the network is Send's to find, and it stops there, in its own words.
	await page.goto(`/pay?to=${ALICE}&chain=59144&amount=1`);
	const stop = page.getByRole('dialog', { name: en('send.lock.netTitle') });
	await expect(stop).toBeVisible({ timeout: 30_000 });
	await expect(stop).toContainText(en('send.lock.netBody').replace('{{chainId}}', '59144'));
	await expect(stop.getByRole('button', { name: en('send.lock.addNetwork') })).toBeEnabled();
	await expect(page.getByRole('textbox', { name: en('send.recipientLabel') })).toHaveCount(0);
});

test('a link for a token that is not on the chain stops Send and says so', async ({ page }) => {
	await walletOnDevice(page);
	await page.goto(`/pay?to=${ALICE}&chain=1&token=${NO_TOKEN}&amount=1&sym=USDC&dec=6`);
	const stop = page.getByRole('dialog', { name: en('send.lock.tokenTitle') });
	await expect(stop).toBeVisible({ timeout: 30_000 });
	await expect(stop).toContainText(en('send.lock.tokenBody'));
	// The link's own word for the token ("USDC") is never shown as the token.
	await expect(page.getByRole('textbox', { name: 'USDC' })).toHaveCount(0);
	await expect(page.getByRole('textbox', { name: en('send.recipientLabel') })).toHaveCount(0);
});

test('with no wallet here: said, the two ways in, and the request waits for the wallet', async ({
	page
}) => {
	await page.addInitScript(() => localStorage.setItem('vela.intro.seen', String(Date.now())));
	await stubChain(page);
	await page.goto(`/pay?to=${ALICE}&chain=1&amount=0.25`);
	await expect(page.getByText(en('receive.pay.noWalletBody'))).toBeVisible({ timeout: 30_000 });
	await expect(page.getByText(ALICE)).toBeVisible();
	await expect(
		page.getByRole('link', { name: en('onboarding.welcome.createWallet') })
	).toHaveAttribute('href', '/en/create');
	await expect(
		page.getByRole('link', { name: en('onboarding.welcome.alreadyHaveWallet') })
	).toBeVisible();
	expect(await page.evaluate(() => localStorage.getItem('vela.payKeptLink'))).not.toBeNull();

	// A wallet arrives (created or signed into): the wallet opens Send on the
	// request that waited for it.
	await seedSignedIn(page);
	await page.goto('/en/wallet');
	await expectSendPrefilled(page, '0.25');
});

test('the Worker serves the app-link associations as the platform lanes wrote them', async ({
	request
}) => {
	for (const [path, file] of [
		[
			'/.well-known/apple-app-site-association',
			'app-ios/universal-links/wallet.getvela.app.apple-app-site-association.json'
		],
		['/.well-known/assetlinks.json', 'app-android/vela-wallet/app-links/assetlinks.json']
	] as const) {
		const response = await request.get(path, { maxRedirects: 0 });
		expect(response.status(), path).toBe(200);
		expect(response.headers()['content-type'], path).toMatch(/^application\/json/);
		expect(await response.text(), path).toBe(readFileSync(join(REPO, file), 'utf8'));
	}
});

/**
 * The pay flow's screens, for review — en and zh, light and dark. Off in the
 * suite; `VELA_PAY_SHOTS=<dir>` writes them there.
 */
test('pay flow screenshots', async ({ page }) => {
	const dir = process.env.VELA_PAY_SHOTS;
	test.skip(!dir, 'set VELA_PAY_SHOTS=<dir> to write the pay flow screenshots');
	test.setTimeout(600_000);
	await page.addInitScript(() => localStorage.setItem('vela.intro.seen', String(Date.now())));
	await stubChain(page);
	const words = { en, zh };
	for (const locale of ['en', 'zh'] as const) {
		for (const scheme of ['light', 'dark'] as const) {
			await page.emulateMedia({ colorScheme: scheme });
			const shot = (name: string) =>
				page.screenshot({ path: join(dir!, `${locale}-${scheme}-${name}.png`) });

			await page.evaluate(() => localStorage.removeItem('vela.accounts')).catch(() => {});
			await page.goto(`/${locale}/pay?to=${ALICE}&chain=1&amount=0x10`);
			await expect(page.getByText(words[locale]('receive.pay.invalidAmount'))).toBeVisible({
				timeout: 30_000
			});
			// A first visit plays the launch mark over the page (3 s at most).
			await page.waitForTimeout(3500);
			await shot('1-refused');

			await page.evaluate(() => {
				localStorage.removeItem('vela.accounts');
				localStorage.removeItem('vela.activeAccountIndex');
			});
			await page.goto(`/${locale}/pay?to=${ALICE}&chain=1&amount=0.5`);
			await expect(page.getByText(words[locale]('receive.pay.noWalletBody'))).toBeVisible({
				timeout: 30_000
			});
			await shot('2-no-wallet');

			await page.evaluate(() => {
				localStorage.setItem(
					'vela.accounts',
					JSON.stringify([
						{
							id: 'e2e-credential-id',
							name: 'E2E Wallet',
							address: '0x14fb1f4e2b9c7a5d8e3f6a1b4c7d9e2f5a8b1d1e',
							public_key_hex: '04' + 'ab'.repeat(64),
							created_at_iso: '2026-01-01T00:00:00.000Z',
							keys: []
						}
					])
				);
				localStorage.setItem('vela.activeAccountIndex', '0');
			});
			await page.goto(`/${locale}/wallet`);
			await seedNetworkOverrides(
				page,
				CHAINS.map((c) => ({ chainId: c.chainId, rpcURL: `${STUB}/${c.chainId}` }))
			);
			await page.goto(`/${locale}/pay?to=${ALICE}&chain=1&amount=0.5&sym=ETH&dec=18`);
			await expect(page.getByRole('textbox', { name: 'ETH' })).toHaveValue('0.5', {
				timeout: 30_000
			});
			await page.waitForTimeout(1500);
			await shot('3-send-prefilled');

			await page.goto(`/${locale}/pay?to=${ALICE}&chain=59144&amount=1`);
			await expect(
				page.getByRole('dialog', { name: words[locale]('send.lock.netTitle') })
			).toBeVisible({ timeout: 30_000 });
			await page.waitForTimeout(800);
			await shot('4-unknown-chain');

			await page.goto(`/${locale}/pay?to=${ALICE}&chain=1&token=${NO_TOKEN}&amount=1&dec=6`);
			await expect(
				page.getByRole('dialog', { name: words[locale]('send.lock.tokenTitle') })
			).toBeVisible({ timeout: 30_000 });
			await page.waitForTimeout(800);
			await shot('5-token-not-on-chain');
		}
	}
});
