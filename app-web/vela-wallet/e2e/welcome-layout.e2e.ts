/**
 * Responsive gate (spec 006 SC-003, FR-001/003): no horizontal overflow at any
 * checked width, the 1280px boundary switches layouts, and both CTAs reach the
 * flow they own.
 *
 * The container assertions changed with spec 019. Creating a wallet is a
 * stepped journey and now has its own route, so it NAVIGATES rather than
 * swapping a column — a reload mid-ceremony strands nobody and back works.
 * Signing in has no steps, so it still happens in place and speaks only
 * through the button's busy state.
 */
import { expect, test } from '@playwright/test';
import { STORAGE_KEY as INTRO_SEEN_KEY } from '../src/lib/intro/gate';

// Every test here needs the LANDING page. A fresh Playwright context is a
// first run, and a first run opens on the intro carousel (spec 020) — so the
// suite marks the intro seen before any document loads, exactly as a
// returning visitor's browser would. The intro's own behaviour has its own
// coverage (src/lib/intro/gate.test.ts).
test.beforeEach(async ({ page }) => {
	await page.addInitScript(
		([key]) => window.localStorage.setItem(key, String(Date.now())),
		[INTRO_SEEN_KEY]
	);
});

const WIDTHS = [320, 375, 768, 1279, 1280, 1440, 1920];

for (const width of WIDTHS) {
	test(`no horizontal overflow at ${width}px`, async ({ page }) => {
		await page.setViewportSize({ width, height: 900 });
		await page.goto('/en');
		const overflow = await page.evaluate(
			() => document.documentElement.scrollWidth - document.documentElement.clientWidth
		);
		expect(overflow).toBe(0);
		// A LINK since spec 019: creating a wallet is a route, not a panel swap.
		await expect(page.getByRole('link', { name: 'Create Wallet' })).toBeVisible();
	});
}

test('1279px stacks the two ways in', async ({ page }) => {
	await page.setViewportSize({ width: 1279, height: 900 });
	await page.goto('/en');
	// One column at every width (spec 019). Below the breakpoint the buttons
	// stack; the check is the axis, not a class name.
	const create = (await page.getByRole('link', { name: 'Create Wallet' }).boundingBox())!;
	const signIn = (await page
		.getByRole('button', { name: 'I already have a wallet' })
		.boundingBox())!;
	expect(signIn.y).toBeGreaterThan(create.y + create.height - 1);
});

test('1280px puts the two ways in side by side', async ({ page }) => {
	await page.setViewportSize({ width: 1280, height: 900 });
	await page.goto('/en');
	const create = (await page.getByRole('link', { name: 'Create Wallet' }).boundingBox())!;
	const signIn = (await page
		.getByRole('button', { name: 'I already have a wallet' })
		.boundingBox())!;
	expect(signIn.x).toBeGreaterThan(create.x + create.width - 1);
	expect(Math.abs(signIn.y - create.y)).toBeLessThan(2);
	await expect(page.locator('.actions')).toBeVisible();
});

test('resizing across the boundary keeps the page intact', async ({ page }) => {
	await page.setViewportSize({ width: 1440, height: 900 });
	await page.goto('/en');
	await expect(page.locator('.headline')).toBeVisible();
	await page.setViewportSize({ width: 390, height: 844 });
	await expect(page.locator('.headline')).toBeVisible();
	await expect(page.getByRole('link', { name: 'Create Wallet' })).toBeVisible();
});

test('the headline shrinks below the desktop breakpoint', async ({ page }) => {
	await page.goto('/en');
	const headline = page.locator('.headline');

	await page.setViewportSize({ width: 1440, height: 900 });
	const wide = Number.parseFloat(await headline.evaluate((el) => getComputedStyle(el).fontSize));

	await page.setViewportSize({ width: 390, height: 844 });
	const narrow = Number.parseFloat(await headline.evaluate((el) => getComputedStyle(el).fontSize));

	expect(wide).toBeGreaterThan(narrow);
});

for (const width of [1440, 390]) {
	test(`Create Wallet navigates to the flow at ${width}px`, async ({ page }) => {
		await page.setViewportSize({ width, height: 900 });
		await page.goto('/en');

		await page.getByRole('link', { name: 'Create Wallet' }).click();
		await expect(page).toHaveURL('/en/create');
		// The flow's own chrome is now the back affordance alone (the stepped
		// bar and the flow label were removed 2026-08-25). Which step is
		// showing is the core's to say, so this asserts arrival, not contents.
		await expect(page.getByRole('button', { name: 'Back' })).toBeVisible();

		await page.goBack();
		await expect(page).toHaveURL('/en');
		await expect(page.getByRole('link', { name: 'Create Wallet' })).toBeVisible();
	});
}

test('sign-in stays on Welcome — it has no steps to show', async ({ page }) => {
	await page.setViewportSize({ width: 390, height: 844 });
	await page.goto('/en');

	// No virtual authenticator here, so the ceremony will fail — the assertion
	// is only that activating it does not navigate away. What the failure looks
	// like is e2e/onboarding-signin.spec.ts's job, with an authenticator.
	await page.getByRole('button', { name: 'I already have a wallet' }).click();
	await expect(page).toHaveURL('/en');
});

/**
 * Spec 038 SC-428 (T054): "I already have a wallet" opens the three ways in
 * by name — a sheet on the phone, a dialog at desktop width — each row with
 * its glyph, so a security key or a phone is reachable without the browser's
 * own sheet deciding.
 */
for (const [width, height] of [
	[390, 844],
	[1440, 900]
] as const) {
	test(`sign-in offers the three ways in, with icons, at ${width}px`, async ({ page }) => {
		await page.setViewportSize({ width, height });
		await page.goto('/en');
		await page.getByRole('button', { name: 'I already have a wallet' }).click();
		await expect(page.getByRole('heading', { name: 'Sign In' })).toBeVisible();
		const rows = page.locator('.methods .method');
		await expect(rows).toHaveCount(3);
		for (let i = 0; i < 3; i += 1) {
			await expect(rows.nth(i).locator('svg')).toBeVisible();
		}
		await expect(page).toHaveURL('/en');
	});
}

test('mobile brand mark and wordmark share one row', async ({ page }) => {
	await page.setViewportSize({ width: 390, height: 844 });
	await page.goto('/en');
	// .column scope: the desktop rail carries its own header.brand in the DOM
	// at every width (hidden below the breakpoint), so an unscoped `.brand svg`
	// resolves to two elements. The mobile mark is the one inside the column.
	const mark = page.locator('.column header.brand svg');
	const wordmark = page.locator('.column .wordmark');
	const markBox = (await mark.boundingBox())!;
	const wordBox = (await wordmark.boundingBox())!;
	const markMid = markBox.y + markBox.height / 2;
	expect(markMid).toBeGreaterThan(wordBox.y);
	expect(markMid).toBeLessThan(wordBox.y + wordBox.height);
});

/**
 * Spec 038 SC-411/412: the intro is composed like the two screens either side
 * of it. `?intro` forces it regardless of the seen flag the suite sets above.
 */
test.describe('the first-run intro', () => {
	for (const width of [1280, 1440]) {
		test(`stands beside the rail at ${width}px`, async ({ page }) => {
			await page.setViewportSize({ width, height: 900 });
			await page.goto('/en?intro');
			await expect(page.locator('.intro .rail')).toBeVisible();
			// No hole: the column ends where its content ends, not at the
			// viewport's bottom. The dots sit within the content, not pinned.
			const column = (await page.locator('.intro .column').boundingBox())!;
			expect(column.height).toBeLessThan(900 - 1);
			const overflow = await page.evaluate(
				() => document.documentElement.scrollWidth - document.documentElement.clientWidth
			);
			expect(overflow).toBe(0);
		});
	}

	test('keeps the phone composition below the breakpoint', async ({ page }) => {
		await page.setViewportSize({ width: 390, height: 844 });
		await page.goto('/en?intro');
		await expect(page.locator('.intro .rail')).toBeHidden();
		await expect(page.getByRole('button', { name: 'Skip' })).toBeVisible();
		// The column fills the height: the footer rides the bottom.
		const column = (await page.locator('.intro .column').boundingBox())!;
		expect(column.height).toBeGreaterThan(844 - 2);
	});

	test('the last slide puts the two ways in side by side at 1440px', async ({ page }) => {
		await page.setViewportSize({ width: 1440, height: 900 });
		await page.goto('/en?intro');
		await page.keyboard.press('ArrowRight');
		await page.keyboard.press('ArrowRight');
		const create = (await page.getByRole('link', { name: 'Create Wallet' }).boundingBox())!;
		const signIn = (await page
			.getByRole('button', { name: 'I already have a wallet' })
			.boundingBox())!;
		expect(signIn.x).toBeGreaterThan(create.x + create.width - 1);
		expect(Math.abs(signIn.y - create.y)).toBeLessThan(2);
	});
});
