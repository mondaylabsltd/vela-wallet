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
	it('negotiates Accept-Language alone — no cookie is read (spec 006)', () => {
		const response = hit({ 'accept-language': 'ja', cookie: 'vela_lang=de' });
		expect(response.status).toBe(307);
		expect(response.headers.get('location')).toBe('/ja');
		expect(response.headers.get('vary')).toBe('Accept-Language');
		expect(response.headers.get('cache-control')).toBe('private, no-store');
	});

	it('keeps the query string across the hop', () => {
		const response = hit({ 'accept-language': 'fr' }, '/?utm_source=x');
		expect(response.headers.get('location')).toBe('/fr?utm_source=x');
	});
});
