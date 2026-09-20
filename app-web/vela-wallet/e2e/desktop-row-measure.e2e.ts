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
 * - the dropdown menus (founder, es-MX, 2026-09-20): the same family. The menu
 *   was exactly as wide as the 280 control column, while a choice is a label
 *   AND an unwrappable note ("Seguir al sistema" + "Sistema · Español
 *   (México)"). The note took what it needed, the label was squeezed to one
 *   word per line, and the note still spilled out of the menu.
 *
 * All pinned as measurements, because "looks fine on my screen" is how they
 * shipped.
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

test.describe('dropdown menus hold their choices', () => {
	test.use({ viewport: { width: 1440, height: 1000 } });

	/** Appearance carries the language menu; Localization the four format menus. */
	const PAGES = [1, 2];

	for (const locale of SUPPORTED_LOCALES) {
		test(`${locale}: every choice is one line and inside its menu at every text size`, async ({
			page
		}) => {
			await quiet(page);
			await seedSignedIn(page);
			await page.goto(`/${locale}/settings`);
			const nav = page.locator('nav.settings-nav button');
			await expect(nav.first()).toBeVisible();

			let menusSeen = 0;
			for (const index of PAGES) {
				await setTextScale(page, 1);
				await nav.nth(index).click();
				const triggers = page.locator('main .dropdown .trigger');
				await expect(triggers.first()).toBeVisible();

				for (let t = 0; t < (await triggers.count()); t++) {
					await setTextScale(page, 1);
					await triggers.nth(t).click();
					const menu = page.locator('main .dropdown .menu');
					await expect(menu).toBeVisible();
					menusSeen++;

					for (const scale of TEXT_SCALES) {
						await setTextScale(page, scale);
						const found = await menu.evaluate((el) => {
							const box = el.getBoundingClientRect();
							const row = el.closest('.form-row')!.getBoundingClientRect();
							const options = [...el.querySelectorAll('.select-row')];
							// Not a height comparison between choices: this menu mixes scripts,
							// and a CJK line is taller than a Latin one without having wrapped.
							// Each label against ITSELF forced onto one line.
							const wrapped = options.filter((o) => {
								const label = o.querySelector<HTMLElement>('.label')!;
								const now = label.getBoundingClientRect().height;
								const before = label.style.whiteSpace;
								label.style.whiteSpace = 'nowrap';
								const oneLine = label.getBoundingClientRect().height;
								label.style.whiteSpace = before;
								return now > oneLine + 1;
							});
							return {
								options: options.length,
								wrapped: wrapped.map((o) => o.textContent?.trim()),
								spilled: options.flatMap((o) =>
									[...o.children]
										.filter((c) => {
											const r = c.getBoundingClientRect();
											return r.width > 0 && (r.right > box.right + 1 || r.left < box.left - 1);
										})
										.map((c) => c.textContent?.trim())
								),
								// It may grow past its trigger, but only into its own row.
								leftOfRow: box.left < row.left - 1,
								rightOfRow: box.right > row.right + 1
							};
						});
						const where = `page ${index}, menu ${t}, text scale ${scale}`;
						expect(found.wrapped, `wrapped — ${where}`).toEqual([]);
						expect(found.spilled, `spilled out of the menu — ${where}`).toEqual([]);
						expect(found.leftOfRow, `menu left its row — ${where}`).toBe(false);
						expect(found.rightOfRow, `menu left its row — ${where}`).toBe(false);
					}

					await page.keyboard.press('Escape');
					await expect(menu).toHaveCount(0);
				}
			}
			// Language + currency + number + date + time. (The currency list is
			// provider-driven and empty offline — counted, with nothing to measure.)
			expect(menusSeen).toBe(5);
		});
	}
});
