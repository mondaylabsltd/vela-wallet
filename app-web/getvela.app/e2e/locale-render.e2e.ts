import { expect, test } from '@playwright/test';

/**
 * What a reader actually gets (spec 059, US2/US3).
 *
 * `request.get` rather than `page.goto` for the rendering claims: it reads the
 * raw HTTP response, so a page that only becomes Chinese after hydration fails —
 * which is the point of FR-011. A browser test would happily pass on a page that
 * is blank for a crawler.
 */

test.describe('a locale renders server-side (FR-011, FR-014)', () => {
	test('/zh carries Chinese copy in the response body, not after hydration', async ({
		request
	}) => {
		const html = await (await request.get('/zh')).text();
		expect(html).toContain('<html lang="zh"');
		expect(html).toContain('真正属于你的以太坊钱包');
		// the subtitle too — a translated headline over an English page is the
		// half-translated state SC-006 forbids
		expect(html).toContain('用通行密钥签名');
	});

	test('/ is untouched English', async ({ request }) => {
		const html = await (await request.get('/')).text();
		expect(html).toContain('<html lang="en"');
		expect(html).toContain('An Ethereum wallet you actually own');
	});
});

test.describe('hreflang reflects what exists, not what is supported (FR-018, FR-021)', () => {
	test('/ advertises only the locales that genuinely have it', async ({ request }) => {
		const html = await (await request.get('/')).text();
		const tags = [...html.matchAll(/<link rel="alternate" hreflang="([^"]+)"/g)].map((m) => m[1]);
		expect(tags).toContain('en');
		expect(tags).toContain('zh');
		expect(tags).toContain('x-default');
		// Nothing else is translated yet — advertising ja here would tell Google a
		// Japanese page exists when /ja is English.
		expect(tags).not.toContain('ja');
	});

	test('a localized page canonicalizes to itself, never to English', async ({ request }) => {
		const html = await (await request.get('/zh')).text();
		expect(html).toMatch(/rel="canonical" href="https:\/\/getvela\.app\/zh"/);
	});

	test('x-default points at the unprefixed English URL', async ({ request }) => {
		const html = await (await request.get('/zh')).text();
		expect(html).toContain('hreflang="x-default" href="https://getvela.app/"');
	});

	test('exactly one og:title per page — app.html must not add a second', async ({ request }) => {
		const html = await (await request.get('/')).text();
		expect([...html.matchAll(/property="og:title"/g)]).toHaveLength(1);
	});
});

test.describe('switching language keeps the page (SC-003)', () => {
	test('from a docs page to 简体中文 lands on the same doc', async ({ page }) => {
		await page.goto('/docs/networks-and-fees');
		await page.getByRole('group').filter({ hasText: 'English' }).first().click();
		await page.getByRole('link', { name: '简体中文' }).click();
		await expect(page).toHaveURL(/\/zh\/docs\/networks-and-fees$/);
	});

	test('the switcher is absent on English-only pages', async ({ page }) => {
		// /blog has no translated version, so offering one would produce links to
		// URLs that 404 — which is what the prerender crawler caught.
		await page.goto('/blog');
		await expect(page.locator('[data-rybbit-event="language_switch"]')).toHaveCount(0);
	});
});
