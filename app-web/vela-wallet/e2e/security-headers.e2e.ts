/**
 * §35 (review 1-1): the built wallet, served as the deploy serves it
 * (`e2e/preview.mjs`: the Worker bundle on workerd, `_headers` applied),
 * carries its headers on a prerendered page and on a Worker response, its
 * page CSP is hash-based, and the wallet runs under it with no violation.
 */
import { expect, test } from '@playwright/test';
import { WALLET_HEADERS } from '../src/lib/security-headers';

function expectWalletHeaders(headers: Record<string, string>): void {
	for (const [name, value] of Object.entries(WALLET_HEADERS)) {
		const got = headers[name.toLowerCase()] ?? '';
		if (name === 'Content-Security-Policy') expect(got, name).toContain("frame-ancestors 'none'");
		else expect(got, name).toBe(value);
	}
}

test('a prerendered page carries the headers and a hash-based CSP', async ({ request }) => {
	const response = await request.get('/en');
	expect(response.status()).toBe(200);
	expectWalletHeaders(response.headers());
	const html = await response.text();
	const meta = /<meta http-equiv="content-security-policy" content="([^"]+)"/i.exec(html);
	expect(meta, 'the page CSP').not.toBeNull();
	const csp = meta![1].replaceAll('&#39;', "'").replaceAll('&quot;', '"');
	expect(csp).toMatch(/script-src 'self' 'wasm-unsafe-eval'( 'sha256-[A-Za-z0-9+/=]+')+/);
	expect(csp).not.toContain("'unsafe-eval'");
	expect(csp).toContain("object-src 'none'");
});

test('a Worker response carries the same headers (the language redirect)', async ({ request }) => {
	const response = await request.get('/', { maxRedirects: 0 });
	expect(response.status()).toBe(307);
	expectWalletHeaders(response.headers());
});

test('the wallet boots its core under the CSP with no violation', async ({ page }) => {
	const violations: string[] = [];
	page.on('console', (message) => {
		if (/Content.Security.Policy|Refused to (execute|load|connect|apply)/i.test(message.text())) {
			violations.push(message.text());
		}
	});
	await page.addInitScript(() => {
		localStorage.setItem('vela.intro.seen', String(Date.now()));
		document.addEventListener('securitypolicyviolation', (event) => {
			console.error(
				`Content-Security-Policy violation: ${event.violatedDirective} ${event.blockedURI}`
			);
		});
	});
	await page.goto('/en');
	// The core is WebAssembly ('wasm-unsafe-eval'): the welcome screen's
	// actions are drawn once it has booted.
	await expect(page.locator('main')).toBeVisible();
	await page.waitForLoadState('networkidle');
	expect(violations).toEqual([]);
});
