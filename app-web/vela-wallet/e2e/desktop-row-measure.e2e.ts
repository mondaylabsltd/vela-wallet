/**
 * Two desktop layout reports from a wide Windows display (issues 195, 198),
 * neither of which was about Windows:
 *
 * - 195: a label↔value row is two-ended, so every pixel of column width lands
 *   as dead space BETWEEN the two things the eye has to connect. The rows ran
 *   the full 800 content column at every desktop width — a token sat 610px
 *   from its balance at 1440 exactly as it did at 2200. A wide, sparse window
 *   is only where it became impossible to not see.
 * - 198: the settings nav was a fixed 216 while its labels scale with the text
 *   size and change with the language. English wrapped at the largest size
 *   (what was reported); ja and ru wrapped at the standard one.
 *
 * Both are pinned as measurements, because "looks fine on my screen" is how
 * they shipped.
 *
 * Chromium only: nothing here is about storage.
 */
import { expect, test, type Page } from '@playwright/test';
import { seedSignedIn } from './live-helpers';
import { SUPPORTED_LOCALES } from '../src/lib/i18n/locales';

/** `TEXT_SCALE_LEVELS` in services/preferences.svelte.ts, plus the standard 1. */
const TEXT_SCALES = [0.82, 0.91, 1, 1.1, 1.22, 1.35];
const WIDTHS = [1440, 2200];

async function quiet(page: Page): Promise<void> {
	await page.addInitScript(() => {
		window.localStorage.setItem('vela.intro.seen', String(Date.now()));
		window.localStorage.setItem('vela.launch.played', String(Date.now()));
	});
}

async function setTextScale(page: Page, scale: number): Promise<void> {
	await page.evaluate((s) => {
		document.documentElement.style.setProperty('--text-scale', String(s));
	}, scale);
}

async function rowMeasure(page: Page): Promise<number> {
	const px = await page.evaluate(() =>
		parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--layout-rowMeasure'))
	);
	expect(px).toBeGreaterThan(0);
	return px;
}

for (const width of WIDTHS) {
	test.describe(`issue 195 at ${width}px`, () => {
		test.use({ viewport: { width, height: 1000 } });

		test('an asset row is no wider than the row measure, and the column shares one edge', async ({
			page
		}) => {
			await quiet(page);
			await page.goto('/en/gallery/d1');
			const rows = page.locator('main button.row').filter({ has: page.locator('.numbers') });
			await expect(rows.first()).toBeVisible();
			const measure = await rowMeasure(page);

			for (const scale of TEXT_SCALES) {
				await setTextScale(page, scale);
				const boxes = await page.evaluate(() => {
					const box = (el: Element) => el.getBoundingClientRect();
					return {
						rows: [...document.querySelectorAll('main button.row')].map((el) => ({
							width: box(el).width,
							right: box(el).right
						})),
						actionsRight: box(document.querySelector('main .actions')!).right
					};
				});
				expect(boxes.rows.length).toBeGreaterThan(0);
				for (const row of boxes.rows) {
					expect(row.width, `row width at text scale ${scale}`).toBeLessThanOrEqual(measure + 1);
					// The dock, the activity rows and the asset rows end on one line.
					expect(Math.abs(row.right - boxes.actionsRight)).toBeLessThanOrEqual(1);
				}
			}
		});

		test('a settings row is no wider than the row measure, and its control never reaches its label', async ({
			page
		}) => {
			await quiet(page);
			await page.goto('/en/gallery/dst2');
			await expect(page.locator('.form-row').first()).toBeVisible();
			const measure = await rowMeasure(page);

			for (const scale of TEXT_SCALES) {
				await setTextScale(page, scale);
				const rows = await page.evaluate(() => {
					const ink = (el: Element) => {
						const range = document.createRange();
						range.selectNodeContents(el);
						return range.getBoundingClientRect();
					};
					return [...document.querySelectorAll('.form-row')].map((row) => ({
						label: row.querySelector('.label')!.textContent,
						width: row.getBoundingClientRect().width,
						gap: ink(row.querySelector('.control')!).left - ink(row.querySelector('.label')!).right
					}));
				});
				expect(rows.length).toBeGreaterThan(0);
				for (const row of rows) {
					expect(row.width, `${row.label} at ${scale}`).toBeLessThanOrEqual(measure + 1);
					// Narrowing the row must not have bought a collision instead.
					expect(row.gap, `${row.label} at ${scale}`).toBeGreaterThan(0);
				}
			}
		});
	});
}

test.describe('issue 198', () => {
	test.use({ viewport: { width: 1440, height: 1000 } });

	for (const locale of SUPPORTED_LOCALES) {
		test(`${locale}: every settings nav item is one line at every text size`, async ({ page }) => {
			await quiet(page);
			await seedSignedIn(page);
			await page.goto(`/${locale}/settings`);
			const items = page.locator('nav.settings-nav button');
			await expect(items.first()).toBeVisible();

			for (const scale of TEXT_SCALES) {
				await setTextScale(page, scale);
				// Height, not a line count: the label is a flex item, so it is one
				// box however many lines it holds — a wrapped item is a TALLER one.
				const heights = await items.evaluateAll((els) =>
					els.map((el) => ({
						label: el.textContent?.trim(),
						height: el.getBoundingClientRect().height
					}))
				);
				expect(heights.length).toBeGreaterThan(1);
				const shortest = Math.min(...heights.map((h) => h.height));
				const wrapped = heights.filter((h) => h.height > shortest + 1).map((h) => h.label);
				expect(wrapped, `wrapped at text scale ${scale}`).toEqual([]);
			}

			// The column grew to fit; the panel beside it must still have its room.
			await setTextScale(page, 1.35);
			const panel = await page.locator('main .panel').boundingBox();
			expect(panel?.width).toBeGreaterThan(await rowMeasure(page));
		});
	}
});
