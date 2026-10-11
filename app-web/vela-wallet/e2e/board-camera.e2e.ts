/**
 * A board never opens a camera (PR 3 note 7 — privacy).
 *
 * A gallery sweep on the desktop walked onto its scanner state; the real
 * camera started, and a frame of the person at the machine was captured. The
 * web had the same door: on the Explore boards the scan button mounted the
 * live scanner, which asked the browser for the camera from a gallery page.
 *
 * Every board that shows a scanner is opened here with the browser's camera
 * API replaced by a counter, so nothing in this file can open a real camera
 * whatever the app does. On a board the count stays at zero — not one call,
 * not even a look at which devices exist — and the frame is the fixture: a
 * sample code in the viewfinder. On the wallet itself the same instrument
 * counts the ask, so a zero above is the gate's doing and not a deaf counter.
 */
import { expect, test, type Page } from '@playwright/test';
import { en, seedSignedIn, zh } from './live-helpers';
import { denyOffOrigin } from './stub-chain';

test.use({ viewport: { width: 390, height: 844 } });

type Asked = { getUserMedia: number; enumerateDevices: number };

/** Count every way of asking for a camera, and refuse each one. */
async function watchCamera(page: Page): Promise<void> {
	await page.addInitScript(() => {
		const asked = { getUserMedia: 0, enumerateDevices: 0 };
		(window as unknown as { __cameraAsked: typeof asked }).__cameraAsked = asked;
		const refusing = {
			getUserMedia: () => {
				asked.getUserMedia += 1;
				return Promise.reject(Object.assign(new Error('refused'), { name: 'NotAllowedError' }));
			},
			enumerateDevices: () => {
				asked.enumerateDevices += 1;
				return Promise.resolve([]);
			}
		};
		Object.defineProperty(navigator, 'mediaDevices', { configurable: true, value: refusing });
	});
}

const asked = (page: Page): Promise<Asked> =>
	page.evaluate(() => (window as unknown as { __cameraAsked: Asked }).__cameraAsked);

test('the Explore board’s scan button draws the fixture frame and never asks for a camera', async ({
	page
}, testInfo) => {
	await watchCamera(page);
	for (const [locale, t] of [
		['en', en],
		['zh', zh]
	] as const) {
		await page.goto(`/${locale}/gallery/e1`);
		const frame = page.getByTestId('scan-fixture-frame');
		// The button is in the server's HTML before the page hydrates; pressed
		// then, it does nothing. Press until the scanner answers.
		await expect(async () => {
			if (!(await frame.isVisible())) {
				await page.getByRole('button', { name: t('explore.scan') }).click({ timeout: 1_000 });
			}
			await expect(frame).toBeVisible({ timeout: 1_000 });
		}).toPass({ timeout: 20_000 });
		// A sample code is drawn in the viewfinder; no `<video>` is mounted.
		await expect(frame.locator('svg rect').first()).toBeVisible();
		await expect(page.locator('video')).toHaveCount(0);
		// The hint, not a refusal: the board is a picture of a scan.
		await expect(page.getByText(t('componentsUi.scanner.permissionText'))).toHaveCount(0);
		// The tools are drawn and pressing them reaches no camera either.
		await page.getByRole('button', { name: t('componentsUi.scanner.flipCamera') }).click();
		await page.getByRole('button', { name: t('componentsUi.scanner.torch') }).click();
		await page.waitForTimeout(300);
		expect(await asked(page), `${locale}/gallery/e1`).toEqual({
			getUserMedia: 0,
			enumerateDevices: 0
		});
		if (locale === 'en') await page.screenshot({ path: testInfo.outputPath('note7-explore.png') });
	}
});

test('the scanner boards themselves — phone and wide — are the fixture frame', async ({ page }) => {
	await watchCamera(page);
	for (const state of ['s1', 'ds1']) {
		await page.goto(`/en/gallery/${state}`);
		await expect(page.getByTestId('scan-fixture-frame')).toBeVisible();
		await expect(page.locator('video')).toHaveCount(0);
		await page.waitForTimeout(300);
		expect(await asked(page), state).toEqual({ getUserMedia: 0, enumerateDevices: 0 });
	}
});

test('the wallet itself still asks: the same counter hears the live scanner', async ({ page }) => {
	await watchCamera(page);
	await seedSignedIn(page);
	await denyOffOrigin(page);
	await page.goto('/en/wallet');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await page
		.getByRole('button', { name: en('componentsUi.dock.scan') })
		.first()
		.click();
	// The camera was asked for (and, here, refused): said in words, with the
	// way round it still offered.
	await expect(page.getByText(en('componentsUi.scanner.permissionText'))).toBeVisible();
	await expect(page.getByTestId('scan-fixture-frame')).toHaveCount(0);
	expect((await asked(page)).getUserMedia).toBe(1);
});
