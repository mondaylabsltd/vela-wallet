/**
 * Mode gate + screenshot harvest (spec SC-002/SC-006, FR-005): the page must
 * fully swap between the dark and light token sets with the system color
 * scheme, and screenshots land in e2e/__screenshots__/ for human comparison
 * against docs/design/onboarding/{W1,W1L,D1,D1L}.png (zh, like the mocks).
 */
import { mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from '@playwright/test';
import { COLORS } from '../src/lib/tokens/tokens';

const SHOTS = join(import.meta.dirname, '__screenshots__');

const hexToRgb = (hex: string): string => {
	const n = parseInt(hex.slice(1), 16);
	return `rgb(${(n >> 16) & 0xff}, ${(n >> 8) & 0xff}, ${n & 0xff})`;
};

const VIEWPORTS = [
	{ name: 'mobile', width: 390, height: 844 },
	{ name: 'desktop', width: 1440, height: 900 }
] as const;

for (const scheme of ['dark', 'light'] as const) {
	for (const viewport of VIEWPORTS) {
		test(`${scheme} ${viewport.name}: token surfaces applied, screenshot saved`, async ({
			page
		}) => {
			mkdirSync(SHOTS, { recursive: true });
			await page.emulateMedia({ colorScheme: scheme });
			await page.setViewportSize({ width: viewport.width, height: viewport.height });
			await page.goto('/zh');
			const bodyBackground = await page.evaluate(
				() => getComputedStyle(document.body).backgroundColor
			);
			expect(bodyBackground).toBe(hexToRgb(COLORS[scheme]['color.bg.base']));
			const bodyColor = await page.evaluate(() => getComputedStyle(document.body).color);
			expect(bodyColor).toBe(hexToRgb(COLORS[scheme]['color.fg.base']));
			await page.screenshot({
				path: join(SHOTS, `${scheme}-${viewport.name}.png`),
				fullPage: false
			});
		});
	}
}

test('switching the scheme swaps the treatment in both directions', async ({ page }) => {
	await page.setViewportSize({ width: 1440, height: 900 });
	await page.emulateMedia({ colorScheme: 'dark' });
	await page.goto('/zh');
	const dark = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);
	await page.emulateMedia({ colorScheme: 'light' });
	const light = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);
	await page.emulateMedia({ colorScheme: 'dark' });
	const darkAgain = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);
	expect(dark).not.toBe(light);
	expect(darkAgain).toBe(dark);
});
