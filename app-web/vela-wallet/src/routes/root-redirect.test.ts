/**
 * `/` — the Worker's one runtime route (contracts/i18n-ssr.md). The e2e suite
 * drives it through the built worker; this pins the decision itself.
 */
import { describe, expect, it } from 'vitest';
import { GET } from './+server';

function hit(headers: Record<string, string>, path = '/'): Response {
	return GET({ request: new Request(`https://getvela.app${path}`, { headers }) });
}

describe('/ (set-12)', () => {
	it('a language pinned in Settings wins over Accept-Language', () => {
		const response = hit({ 'accept-language': 'ja', cookie: 'vela_lang=de' });
		expect(response.status).toBe(307);
		expect(response.headers.get('location')).toBe('/de');
		// The answer now turns on the cookie too, and is never shared.
		expect(response.headers.get('vary')).toBe('Accept-Language, Cookie');
		expect(response.headers.get('cache-control')).toBe('private, no-store');
	});

	it('without a pin — or with one this build does not ship — it negotiates as before', () => {
		expect(hit({ 'accept-language': 'ja' }).headers.get('location')).toBe('/ja');
		expect(hit({ 'accept-language': 'ja', cookie: 'vela_lang=xx' }).headers.get('location')).toBe(
			'/ja'
		);
	});

	it('keeps the query string across the hop', () => {
		const response = hit({ cookie: 'vela_lang=fr' }, '/?utm_source=x');
		expect(response.headers.get('location')).toBe('/fr?utm_source=x');
	});
});
