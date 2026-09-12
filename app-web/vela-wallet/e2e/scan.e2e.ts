/**
 * Reading a code, and every way the wallet can say no (spec 028 T425 — SC-402).
 *
 * Phase 2 proved the wallet can SHOW a code that decodes. This is the other
 * half of the same loop, and it is asserted the same way: a real QR image, made
 * by an encoder the app never sees, put through the real decoder in a real
 * browser, and followed all the way to the recipient field it prefills.
 *
 * The refusals get equal billing, because they are what a person actually meets
 * — most desktops have no rear camera, most CI machines have no camera at all,
 * and a refusal remembered from months ago looks exactly like a broken one. A
 * viewfinder that stays black says none of that, which is why each of these
 * asserts a SENTENCE and not just an absence of pixels.
 */
import QRCode from 'qrcode';
import { expect, test, type Page } from '@playwright/test';
import { CHAINS } from '../src/lib/services/chains';
import { en, seedSignedIn } from './live-helpers';
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

const STUB = 'https://stub-rpc.test/rpc';
const ONE_AND_A_HALF_ETH = 1_500_000_000_000_000_000n;
const THREE_THOUSAND_USD_8DP = 3000n * 100_000_000n;
const ALICE = '0x' + 'a1'.repeat(20);

/** A real QR image — encoded here, so nothing in the app is on both ends. */
async function qrPng(text: string): Promise<Buffer> {
	return QRCode.toBuffer(text, {
		errorCorrectionLevel: 'M',
		margin: 4,
		width: 600,
		type: 'png'
	});
}

async function pick(page: Page, text: string): Promise<void> {
	await page.locator('input[type="file"]').setInputFiles({
		name: 'code.png',
		mimeType: 'image/png',
		buffer: await qrPng(text)
	});
}

/**
 * A camera that answers the way a browser does.
 *
 * `null` removes `mediaDevices` entirely, which on a secure origin is the
 * "there is no camera here" case; a name rejects `getUserMedia` with it.
 */
async function stubCamera(page: Page, reject: string | null): Promise<void> {
	await page.addInitScript((name) => {
		Object.defineProperty(navigator, 'mediaDevices', {
			configurable: true,
			value:
				name === null
					? undefined
					: {
							getUserMedia: () => Promise.reject(Object.assign(new Error(name), { name }))
						}
		});
	}, reject);
}

/** The wallet home, with one chain answering — so the send flow has a token. */
async function openHome(page: Page): Promise<void> {
	await seedSignedIn(page);
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
		if (method === 'eth_call') {
			const call = params[0] as { data?: string } | undefined;
			const n = call?.data ? aggregate3CallCount(call.data) : 0;
			const balance = chainId === 1 ? ONE_AND_A_HALF_ETH : 0n;
			const data =
				'0x' +
				abiWord(balance) +
				abiWord(THREE_THOUSAND_USD_8DP) +
				abiWord(0) +
				abiWord(0) +
				abiWord(0);
			return encodeAggregate3Result(Array.from({ length: n }, () => ({ success: true, data })));
		}
		return undefined;
	});
	await page.goto('/en/wallet');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await seedNetworkOverrides(
		page,
		CHAINS.map((c) => ({ chainId: c.chainId, rpcURL: `${STUB}/${c.chainId}` }))
	);
	await page.reload();
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
}

/** Open the scanner from the home and wait for the surface to be up. */
async function openScanner(page: Page): Promise<void> {
	await page
		.getByRole('button', { name: en('componentsUi.dock.scan') })
		.first()
		.click();
	// The tool row, not the hint: a refusal REPLACES the hint, and every one of
	// these tests is about a refusal.
	await expect(page.getByText(en('componentsUi.scanner.gallery')).first()).toBeVisible();
}

test('a code in a chosen image is read, and its address reaches the send form', async ({
	page
}) => {
	// The camera is refused, which is the ordinary case on a desktop — and the
	// whole point of the photo path is that it is not a dead end.
	await stubCamera(page, 'NotAllowedError');
	await openHome(page);
	await openScanner(page);
	await expect(page.getByText(en('componentsUi.scanner.permissionText'))).toBeVisible();

	await pick(page, ALICE);

	// Read, parsed, and handed to the send core — which picks the account's
	// one token and puts the scanned address where a person can see it.
	const recipient = page.getByRole('textbox', { name: en('send.recipientLabel') });
	await expect(recipient).toBeVisible({ timeout: 30_000 });
	await expect(recipient).toHaveValue(ALICE);
});

test('the scanner opened from the send form fills the row it was opened from', async ({ page }) => {
	// The other half of T423, and the one the CORE owns end to end: the
	// recipient row dispatches `open_scanner`, the core's own `show_scanner`
	// puts the surface on screen, and `scan_resolved` both fills the row and
	// closes it. No shell flag decides any of that — which is why the sweep
	// picker's scan button, drawn against the same field, needs nothing new.
	await stubCamera(page, 'NotAllowedError');
	await openHome(page);
	await page
		.getByRole('button', { name: en('componentsUi.dock.send') })
		.first()
		.click();
	await page.getByText('ETH', { exact: true }).first().click();
	const recipient = page.getByRole('textbox', { name: en('send.recipientLabel') });
	await expect(recipient).toBeVisible({ timeout: 30_000 });

	await page
		.getByRole('button', { name: en('send.scanAria') })
		.first()
		.click();
	await expect(page.getByText(en('componentsUi.scanner.gallery')).first()).toBeVisible();

	await pick(page, ALICE);
	await expect(recipient).toBeVisible({ timeout: 30_000 });
	await expect(recipient).toHaveValue(ALICE);
});

test('a refused camera says so — and still offers the way round it', async ({ page }) => {
	// Both "said no just now" and "said no once, months ago" arrive as
	// NotAllowedError, and for a person they are one instruction: change it in
	// the site settings.
	await stubCamera(page, 'NotAllowedError');
	await seedSignedIn(page);
	await denyOffOrigin(page);
	await page.goto('/en/wallet');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await openScanner(page);

	await expect(page.getByText(en('componentsUi.scanner.permissionText'))).toBeVisible();
	// The alternative is on the same screen, not in a help article.
	await expect(page.getByText(en('componentsUi.scanner.gallery')).first()).toBeVisible();
});

test('a machine with no camera is told that, not left with a black frame', async ({ page }) => {
	await stubCamera(page, null);
	await seedSignedIn(page);
	await denyOffOrigin(page);
	await page.goto('/en/wallet');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await openScanner(page);

	// A different sentence from the refusal above, because it is a different
	// thing to do about it — and neither is the permission prompt that a
	// machine with no camera must never be shown.
	await expect(page.getByText(en('componentsUi.scanner.noCamera'))).toBeVisible();
});

test('a code that is not a payment is said to be that, and the scanner stays alive', async ({
	page
}) => {
	await stubCamera(page, 'NotAllowedError');
	await seedSignedIn(page);
	await denyOffOrigin(page);
	await page.goto('/en/wallet');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await openScanner(page);

	// This is also the control for the test above it: a decoder that returned
	// nothing would report "no QR found" here, and one that returned this
	// string proves the picked image really was read.
	await pick(page, 'https://example.com/not-a-payment');
	await expect(page.getByText(en('home.invalidQrTitle'))).toBeVisible({ timeout: 30_000 });
	await expect(page.getByText(en('componentsUi.scanner.gallery')).first()).toBeVisible();
});
