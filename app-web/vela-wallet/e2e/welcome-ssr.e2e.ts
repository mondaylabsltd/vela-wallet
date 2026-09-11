/**
 * SSR/i18n gate (spec SC-001, FR-008/009/013): the initial HTML — no client
 * JS — must carry the localized first screen for every locale, and `/` must
 * negotiate Accept-Language into a 307. Expectations come from the generated
 * corpus catalogs, so this is a differential against the translation source.
 */
import { execSync } from 'node:child_process';
import { mkdtempSync, readFileSync, readdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { expect, test } from '@playwright/test';
import { STORAGE_KEY as INTRO_SEEN_KEY } from '../src/lib/intro/gate';

const APP_ROOT = join(import.meta.dirname, '..');
const LOCALES = [
	'en',
	'zh',
	'zh-TW',
	'zh-HK',
	'ja',
	'ko',
	'vi',
	'id',
	'tr',
	'es-MX',
	'pt-BR',
	'fr',
	'de',
	'ru',
	'it'
] as const;

interface WelcomeCorpus {
	tagline: string;
	createWallet: string;
	alreadyHaveWallet: string;
	metaTitle: string;
	heroTitle: string;
	heroTitleFit: string;
	heroSubtitle: string;
}

function corpus(locale: string): WelcomeCorpus {
	const raw = JSON.parse(
		readFileSync(join(APP_ROOT, '..', '..', 'public', 'i18n', `${locale}.json`), 'utf8')
	);
	const onboarding = raw.onboarding;
	return {
		tagline: onboarding.welcomeWeb.tagline,
		createWallet: onboarding.welcome.createWallet,
		alreadyHaveWallet: onboarding.welcome.alreadyHaveWallet,
		metaTitle: onboarding.welcomeWeb.meta.title,
		heroTitle: onboarding.welcome.heroTitle as string,
		heroTitleFit: onboarding.welcome.heroTitleFit as string,
		heroSubtitle: onboarding.welcome.heroSubtitle as string
	};
}

const escapeHtml = (s: string) =>
	s.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');

for (const locale of LOCALES) {
	test(`raw HTML of /${locale} is fully localized`, async ({ request }) => {
		const response = await request.get(`/${locale}`);
		expect(response.status()).toBe(200);
		const html = await response.text();
		const expected = corpus(locale);
		expect(html).toContain(`<html lang="${locale}" dir="ltr"`);
		// The v2 Welcome (spec 019): brand, headline, two ways in. The tagline
		// survives only in the meta description now, so the headline is what
		// proves the page itself is localized.
		expect(html).toContain(escapeHtml(expected.heroTitle));
		expect(html).toContain(escapeHtml(expected.heroSubtitle));
		expect(html).toContain(escapeHtml(expected.createWallet));
		expect(html).toContain(escapeHtml(expected.alreadyHaveWallet));
		expect(html).toContain(`<title>${escapeHtml(expected.metaTitle)}</title>`);
		// 15 locale alternates + x-default
		expect(html.match(/hreflang=/g)).toHaveLength(16);
		expect(html).toContain('hreflang="x-default"');
		expect(html).toContain('rel="canonical"');
	});
}

/**
 * The headline fits the frame it was written for — in every locale.
 *
 * The copy authors its own line break, and `heroTitleFit` authors which rung of
 * the type ladder (46/38/31) that break is meant to survive at. Both are corpus
 * values, so both can drift the moment someone edits a translation: a line two
 * words longer wraps into a third line the design has no room for, and nothing
 * else in the suite would notice. This measures the rendered box instead of
 * trusting the declaration — at 390×844, the design's own frame.
 *
 * 390 is the CONTRACT, not the floor (founder direction 2026-08-26). A 375pt
 * iPhone SE or a 360dp Android has ~15pt less column than the widest headline
 * needs, and those frames are allowed to wrap into a third line: the fix would
 * be a SECOND shrink mechanism — by viewport, layered under this one by locale
 * — and two mechanisms competing over one headline is worse than a tolerated
 * wrap on the narrow tail. So this suite pins one width on purpose. Do not
 * "fix" a narrow-device wrap by dropping a locale a rung: that shrinks it on
 * every phone to serve the smallest.
 */
test.describe('the hero headline holds its authored line count', () => {
	// The measurement needs the LANDING page's headline in the live DOM; a
	// fresh context is a first run and would open on the intro carousel
	// (spec 020) instead. Mark the intro seen before any document loads.
	test.beforeEach(async ({ page }) => {
		await page.addInitScript(
			([key]) => window.localStorage.setItem(key, String(Date.now())),
			[INTRO_SEEN_KEY]
		);
	});

	for (const locale of LOCALES) {
		test(`/${locale} at 390×844`, async ({ page }) => {
			await page.setViewportSize({ width: 390, height: 844 });
			await page.goto(`/${locale}`);
			const expected = corpus(locale);
			const authoredLines = expected.heroTitle.split('\n').length;

			const measured = await page.evaluate(async () => {
				const h1 = document.querySelector('.headline');
				if (!h1) throw new Error('no .headline on the page');
				await document.fonts.ready;
				const range = document.createRange();
				range.selectNodeContents(h1);
				// One client rect per rendered line box; sub-pixel tops of the
				// same line collapse on rounding.
				const tops = new Set(
					[...range.getClientRects()].filter((r) => r.width > 0.5).map((r) => Math.round(r.top))
				);
				return {
					lines: tops.size,
					fontSize: getComputedStyle(h1).fontSize,
					long: h1.classList.contains('long')
				};
			});

			expect(measured.long, `${locale} class matches its corpus fit`).toBe(
				expected.heroTitleFit === 'long'
			);
			expect(
				measured.lines,
				`${locale} headline wrapped past its ${authoredLines} authored lines at ${measured.fontSize} — shorten the copy or drop it a rung (heroTitleFit)`
			).toBe(authoredLines);
		});
	}
});

test('/ negotiates Accept-Language into a 307 with Vary', async ({ request }) => {
	const response = await request.get('/', {
		headers: { 'accept-language': 'ja' },
		maxRedirects: 0
	});
	expect(response.status()).toBe(307);
	expect(response.headers()['location']).toBe('/ja');
	expect(response.headers()['vary']).toBe('Accept-Language');
});

test('/ preserves the query string across the locale redirect', async ({ request }) => {
	const response = await request.get('/?utm_source=twitter&ref=launch', {
		headers: { 'accept-language': 'ja' },
		maxRedirects: 0
	});
	expect(response.status()).toBe(307);
	expect(response.headers()['location']).toBe('/ja?utm_source=twitter&ref=launch');
});

test('/ maps regional and legacy tags through the RN table', async ({ request }) => {
	for (const [header, target] of [
		['zh-CN', '/zh'],
		['zh-Hant-TW', '/zh-TW'],
		['zh-MO', '/zh-HK'],
		['pt-PT', '/pt-BR'],
		['in', '/id']
	]) {
		const response = await request.get('/', {
			headers: { 'accept-language': header },
			maxRedirects: 0
		});
		expect(response.headers()['location'], header).toBe(target);
	}
});

test('/ with an unsupported language falls back to /en', async ({ request }) => {
	const response = await request.get('/', {
		headers: { 'accept-language': 'th' },
		maxRedirects: 0
	});
	expect(response.status()).toBe(307);
	expect(response.headers()['location']).toBe('/en');
});

test('unknown locale segment is a 404', async ({ request }) => {
	const response = await request.get('/xx');
	expect(response.status()).toBe(404);
});

test.describe('with JavaScript disabled', () => {
	test.use({ javaScriptEnabled: false });

	test('the headline and both CTAs are still readable', async ({ page }) => {
		await page.setViewportSize({ width: 390, height: 844 });
		await page.goto('/zh');
		const expected = corpus('zh');
		await expect(page.getByText(expected.heroTitle)).toBeVisible();
		await expect(page.getByText(expected.createWallet)).toBeVisible();
		await expect(page.getByText(expected.alreadyHaveWallet)).toBeVisible();
	});
});

test('the DEPLOY bundle contains no wasm (the i18n engine is build-time only)', () => {
	/* .svelte-kit/cloudflare/_worker.js is only a ~4KB adapter shim — grepping
	   it proves nothing. `wrangler deploy --dry-run` produces the bundle that
	   actually ships (hooks + the / endpoint + everything manifest-reachable);
	   if anything ever imports engine.server.ts from runtime code, the wasm
	   base64 lands HERE and this test goes red.

	   Spec 019 added a SECOND wasm path — the onboarding state machines, which
	   run in the browser because that is where the passkey ceremony happens.
	   That does not weaken this test: the machines are fetched by the client
	   from a static asset, and a Worker still cannot compile wasm from bytes.
	   The client-side half of the promise (Welcome itself fetches nothing) is
	   the test below. */
	test.setTimeout(120_000);
	const outdir = mkdtempSync(join(tmpdir(), 'vela-worker-dry-run-'));
	execSync(`pnpm exec wrangler deploy --dry-run --outdir ${JSON.stringify(outdir)}`, {
		cwd: APP_ROOT,
		stdio: 'pipe'
	});
	const bundles = readdirSync(outdir).filter((name) => name.endsWith('.js'));
	expect(bundles.length).toBeGreaterThan(0);
	for (const name of bundles) {
		const bundle = readFileSync(join(outdir, name), 'utf8');
		expect(bundle.includes('WASM_BASE64'), name).toBe(false);
	}
});

test('the Welcome page loads no wasm until someone commits to a flow', async ({ page }) => {
	/* The onboarding core is 3.4 MB and carries all 25 state machines — wasm is
	   not tree-shaken, so it arrives whole or not at all. The whole reason the
	   flow lives behind a route is that this page, which is also the site's
	   landing page in 15 locales, must not pay for it. */
	const wasmRequests: string[] = [];
	page.on('request', (request) => {
		if (request.url().endsWith('.wasm')) wasmRequests.push(request.url());
	});

	await page.goto('/zh');
	await page.waitForLoadState('networkidle');

	expect(wasmRequests, 'Welcome must not fetch the onboarding core').toEqual([]);
});

/**
 * Spec 038 SC-413/414: on a FIRST run the landing page must never be the
 * first frame — the intro is decided before paint by app.html — and yet the
 * prerendered document must still BE the landing page for crawlers.
 */
test('a first run never paints Welcome before the intro', async ({ page }) => {
	// A fresh context has no `vela.intro.seen`; the launch animation is
	// skipped so it cannot mask the frame under test.
	await page.goto('/en?skipLaunch', { waitUntil: 'commit' });
	// Before hydration: the attribute is on <html> and Welcome is hidden.
	await expect(page.locator('html')).toHaveAttribute('data-intro', 'pending');
	const welcomeOpacity = await page
		.locator('main[data-intro-page]')
		.evaluate((el) => getComputedStyle(el).opacity)
		.catch(() => '0');
	expect(welcomeOpacity).toBe('0');
	// After hydration: the intro is up and the attribute is gone.
	await expect(page.locator('.intro')).toBeVisible();
	await expect(page.locator('html')).not.toHaveAttribute('data-intro', 'pending');
});

test('the prerendered document still carries the landing page', async ({ request }) => {
	const html = await (await request.get('/en')).text();
	expect(html).toContain('data-intro-page');
	expect(html).toContain(escapeHtml(corpus('en').heroTitle));
	// And the pre-paint decision is in the head, before any module.
	expect(html.indexOf(`'${INTRO_SEEN_KEY}'`)).toBeGreaterThan(-1);
	expect(html.indexOf(`'${INTRO_SEEN_KEY}'`)).toBeLessThan(html.indexOf('<body'));
});

/**
 * Spec 038 #meta / SC-440: a shared link is a real card. Every locale's
 * prerendered document carries the og/twitter set, the description fits a
 * preview, and the image is served.
 */
for (const locale of ['en', 'zh'] as const) {
	test(`/${locale} carries the share card`, async ({ request }) => {
		const html = await (await request.get(`/${locale}`)).text();
		for (const tag of [
			'property="og:site_name" content="Vela Wallet"',
			'property="og:type" content="website"',
			'property="og:title"',
			'property="og:description"',
			`property="og:url" content="https://app.getvela.app/${locale}"`,
			'property="og:image" content="https://app.getvela.app/og-image.png"',
			'property="og:image:width" content="1200"',
			'name="twitter:card" content="summary_large_image"',
			'name="twitter:image"'
		]) {
			expect(html, tag).toContain(tag);
		}
		const description = html.match(/property="og:description" content="([^"]*)"/)?.[1] ?? '';
		expect(description.length).toBeGreaterThan(0);
		expect(description.length).toBeLessThanOrEqual(111); // 110 + the ellipsis
		expect((html.match(/og:locale:alternate/g) ?? []).length).toBe(14);
	});
}

test('the share image is served as a PNG', async ({ request }) => {
	const response = await request.get('/og-image.png');
	expect(response.status()).toBe(200);
	expect(response.headers()['content-type']).toContain('image/png');
});
