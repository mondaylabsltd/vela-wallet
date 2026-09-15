import { expect, test } from '@playwright/test';

/**
 * The URL contract (spec 059, contracts/routing.md). Every expectation here is
 * a row of that table — including the ones that must NOT have changed, because
 * the whole feature moved every localizable route file into a new subtree and
 * the only honest proof that English survived is asking for the English URLs.
 *
 * Redirects are checked with `maxRedirects: 0`: following them would turn a
 * wrong 302 into a passing test.
 */

const ENGLISH_PAGES = [
	'/',
	'/docs',
	'/docs/faq',
	'/docs/whitepaper',
	'/docs/networks-and-fees',
	'/about',
	'/roadmap',
	'/blog',
	'/blog/hello-world',
	'/privacy',
	'/terms',
	'/registry',
	'/sitemap.xml',
	'/blog/rss.xml'
];

test.describe('English URLs are untouched (FR-009, SC-005)', () => {
	for (const path of ENGLISH_PAGES) {
		test(`${path} still answers 200 with no redirect`, async ({ request }) => {
			const res = await request.get(path, { maxRedirects: 0 });
			expect(res.status(), path).toBe(200);
		});
	}
});

const LOCALIZED = [
	'/ja',
	'/zh',
	'/zh-TW',
	'/zh-HK',
	'/ko',
	'/vi',
	'/id',
	'/tr',
	'/es-MX',
	'/pt-BR',
	'/fr',
	'/de',
	'/ru',
	'/it'
];

test.describe('every prefixed locale exists (FR-008)', () => {
	for (const path of LOCALIZED) {
		test(`${path} renders`, async ({ request }) => {
			const res = await request.get(path, { maxRedirects: 0 });
			expect(res.status(), path).toBe(200);
			expect(await res.text(), path).toContain(`<html lang="${path.slice(1)}"`);
		});
	}

	test('a localized docs page renders', async ({ request }) => {
		const res = await request.get('/ja/docs/faq', { maxRedirects: 0 });
		expect(res.status()).toBe(200);
	});
});

const MUST_404 = [
	// English has exactly one URL, and it is unprefixed.
	'/en',
	'/en/docs',
	// Not a tag, not an alias.
	'/xx/',
	'/zzz',
	// Outside the locale subtree by R5 — blog and legal stay English-only.
	'/ja/blog',
	'/ja/privacy',
	'/ja/terms',
	'/ja/registry'
];

test.describe('what must not exist (FR-010, R5)', () => {
	for (const path of MUST_404) {
		test(`${path} is 404`, async ({ request }) => {
			const res = await request.get(path, { maxRedirects: 0 });
			expect(res.status(), path).toBe(404);
		});
	}
});

const MUST_308: [string, string][] = [
	['/pt', '/pt-BR'],
	['/pt/docs/faq', '/pt-BR/docs/faq'],
	['/pt-br', '/pt-BR'],
	['/PT-BR', '/pt-BR'],
	['/es', '/es-MX'],
	['/zh-CN', '/zh'],
	['/zh-Hans', '/zh'],
	['/zh-Hant', '/zh-TW'],
	['/fr-CA/about', '/fr/about']
];

test.describe('one canonical URL per page (contracts/routing.md §Must 308)', () => {
	for (const [from, to] of MUST_308) {
		test(`${from} → ${to}`, async ({ request }) => {
			const res = await request.get(from, { maxRedirects: 0 });
			expect(res.status(), from).toBe(308);
			expect(new URL(res.headers()['location'], 'http://x').pathname, from).toBe(to);
		});
	}

	test('a redirect keeps the campaign query string', async ({ request }) => {
		const res = await request.get('/pt?utm_source=x&ref=y', { maxRedirects: 0 });
		expect(res.status()).toBe(308);
		expect(res.headers()['location']).toContain('utm_source=x');
		expect(res.headers()['location']).toContain('ref=y');
	});
});
