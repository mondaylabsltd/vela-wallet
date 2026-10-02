/**
 * The receive code, read back off the screen (spec 028 T414 — SC-401).
 *
 * The unit test proves the encoder round-trips. This proves the SCREEN does —
 * that what a person actually shows a friend is the address, not a pattern that
 * looks like one. It takes the path out of the rendered DOM, rasterises it the
 * way a camera would see it, and hands it to a decoder that knows nothing about
 * the encoder.
 *
 * This is the assertion whose absence let spec 021's placeholder ship onto a
 * live receive screen: "a QR appeared" is exactly what a decorative pattern
 * passes.
 */
import jsQR from 'jsqr';
import { expect, test } from '@playwright/test';
import { CHAINS } from '../src/lib/services/chains';
import { TEST_ACCOUNT_ADDRESS, en, seedSignedIn } from './live-helpers';
import { denyOffOrigin } from './stub-chain';

test.use({ viewport: { width: 390, height: 844 } });

/** Draw the code's own path into pixels, with the quiet zone a decoder needs. */
function rasterise(path: string, modules: number, scale = 8, quiet = 4) {
	const dark = new Set<string>();
	for (const [, x, y, run] of path.matchAll(/M(\d+) (\d+)h(\d+)/g)) {
		for (let i = 0; i < Number(run); i++) dark.add(`${Number(x) + i},${y}`);
	}
	const side = (modules + quiet * 2) * scale;
	const pixels = new Uint8ClampedArray(side * side * 4).fill(255);
	for (const cell of dark) {
		const [cx, cy] = cell.split(',').map(Number);
		for (let dy = 0; dy < scale; dy++) {
			for (let dx = 0; dx < scale; dx++) {
				const at = (((cy + quiet) * scale + dy) * side + ((cx + quiet) * scale + dx)) * 4;
				pixels[at] = pixels[at + 1] = pixels[at + 2] = 0;
			}
		}
	}
	return { pixels, side };
}

test('the code on the receive screen decodes to this wallet’s address', async ({ page }) => {
	await seedSignedIn(page);
	await denyOffOrigin(page);
	await page.goto('/en/wallet');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();

	// Receive → pick a network → the code.
	await page
		.getByRole('button', { name: en('componentsUi.dock.receive') })
		.first()
		.click();
	// Each network row carries two buttons — copy the address, and show its
	// code. The second is the one a person points a camera at.
	const showCode = page.getByRole('button', { name: en('componentsUi.funding.showQr') }).first();
	await expect(showCode).toBeVisible({ timeout: 20_000 });
	await showCode.click();
	// The code is a sheet over the list; wait for it rather than for a duration.
	await expect(page.locator('svg[role="img"] path').first()).toBeVisible({ timeout: 20_000 });

	// What the card actually drew: one path, in module units.
	const drawn = await page
		.locator('svg[role="img"] path')
		.first()
		.evaluate((node) => {
			const svg = node.closest('svg')!;
			const box = svg.getAttribute('viewBox')!.split(' ');
			return { path: node.getAttribute('d') ?? '', modules: Number(box[2]) };
		});

	expect(drawn.modules, 'a real address code is 29 modules at EC level M').toBe(29);

	const { pixels, side } = rasterise(drawn.path, drawn.modules);
	const decoded = jsQR(pixels, side, side)?.data ?? null;
	expect(decoded, 'the rendered code must BE the address, not resemble one').toBe(
		TEST_ACCOUNT_ADDRESS
	);
});

/**
 * The list is the wallet's, and the code is the tapped network's (spec 028
 * Phase 9, T481/T482): twelve rows, twelve in the subtitle, and the second
 * row's code is titled for the second chain — not "Ethereum" whatever was
 * tapped. 保存图片 hands over a PNG named for the address.
 */
test('every network is listed, the tapped one is the code’s, and the image saves', async ({
	page
}) => {
	await seedSignedIn(page);
	await denyOffOrigin(page);
	await page.goto('/en/wallet');
	await expect(page.getByText('E2E Wallet').first()).toBeVisible();
	await page
		.getByRole('button', { name: en('componentsUi.dock.receive') })
		.first()
		.click();

	const showCode = page.getByRole('button', { name: en('componentsUi.funding.showQr') });
	await expect(showCode.first()).toBeVisible({ timeout: 20_000 });
	const rows = await showCode.count();
	expect(rows, 'one row per network the wallet knows').toBe(CHAINS.length);
	await expect(page.getByText(String(rows)).first()).toBeVisible();

	// The second network's code: its name in the title, not the first's.
	await showCode.nth(1).click();
	const sheet = page.getByRole('dialog');
	await expect(sheet.getByText(CHAINS[1]!.displayName, { exact: false }).first()).toBeVisible({
		timeout: 20_000
	});
	await expect(sheet.getByText(CHAINS[0]!.displayName, { exact: true })).toHaveCount(0);

	const [download] = await Promise.all([
		page.waitForEvent('download'),
		sheet.getByRole('button', { name: en('receive.request.saveImage') }).click()
	]);
	expect(download.suggestedFilename()).toMatch(/^vela-0x.*\.png$/);
});

/** The code the open sheet draws, decoded the way a camera would read it. */
async function readCode(page: import('@playwright/test').Page): Promise<string | null> {
	const drawn = await page
		.locator('svg[role="img"] path')
		.first()
		.evaluate((node) => {
			const svg = node.closest('svg')!;
			const box = svg.getAttribute('viewBox')!.split(' ');
			return { path: node.getAttribute('d') ?? '', modules: Number(box[2]) };
		});
	const { pixels, side } = rasterise(drawn.path, drawn.modules);
	return jsQR(pixels, side, side)?.data ?? null;
}

/**
 * Spec 090: the "include network" switch under the code. Off by default — the
 * bare address every wallet reads. On, the code names the network on screen
 * (`ethereum:<address>@<chain>`, the core's value) and the calm hint appears
 * under the switch; off again, both go. The two states are attached as
 * screenshots for review.
 */
for (const viewport of [
	{ width: 390, height: 844 },
	{ width: 1440, height: 900 }
]) {
	test.describe(`at ${viewport.width}px`, () => {
		test.use({ viewport });

		test('include network: the code names the network, and the hint says some wallets cannot read it', async ({
			page
		}, testInfo) => {
			await seedSignedIn(page);
			await denyOffOrigin(page);
			await page.goto('/en/wallet');
			await expect(page.getByText('E2E Wallet').first()).toBeVisible();
			// The screenshots are of the sheet, not of the launch animation's
			// dissolve over it.
			await expect
				.poll(() => page.evaluate(() => document.documentElement.dataset.launch ?? null))
				.toBeNull();
			await page
				.getByRole('button', { name: en('componentsUi.dock.receive') })
				.first()
				.click();
			const showCode = page.getByRole('button', { name: en('componentsUi.funding.showQr') });
			await expect(showCode.first()).toBeVisible({ timeout: 20_000 });
			// The second network, so the chain in the code is not a default.
			await showCode.nth(1).click();

			const toggle = page.getByRole('switch', { name: en('receive.includeNetwork') });
			const hint = page.getByText(en('receive.includeNetworkHint'));
			await expect(toggle).toHaveAttribute('aria-checked', 'false', { timeout: 20_000 });
			await expect(hint).toHaveCount(0);
			expect(await readCode(page)).toBe(TEST_ACCOUNT_ADDRESS);
			await page.waitForTimeout(500); // the sheet's own entrance, for the picture
			await testInfo.attach(`web-${viewport.width}-switch-off`, {
				body: await page.screenshot(),
				contentType: 'image/png'
			});

			await toggle.click();
			await expect(toggle).toHaveAttribute('aria-checked', 'true');
			await expect(hint).toBeVisible();
			await expect
				.poll(() => readCode(page))
				.toBe(`ethereum:${TEST_ACCOUNT_ADDRESS}@${CHAINS[1]!.chainId}`);
			await page.waitForTimeout(500); // the track's colour transition, for the picture
			await testInfo.attach(`web-${viewport.width}-switch-on`, {
				body: await page.screenshot(),
				contentType: 'image/png'
			});

			await toggle.click();
			await expect(toggle).toHaveAttribute('aria-checked', 'false');
			await expect(hint).toHaveCount(0);
			await expect.poll(() => readCode(page)).toBe(TEST_ACCOUNT_ADDRESS);
		});
	});
}
