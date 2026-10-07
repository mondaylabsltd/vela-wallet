import { describe, expect, it } from 'vitest';
import { SITE_ORIGIN } from '$lib/site';
import { analyticsAllowed, type AnalyticsEnvironment } from './gate';

const HOSTED: AnalyticsEnvironment = {
	origin: SITE_ORIGIN,
	protocol: 'https:',
	extensionBuild: false,
	automated: false,
	parallelSpace: false,
	optedOut: false
};
const EXTENSION: AnalyticsEnvironment = {
	...HOSTED,
	origin: 'chrome-extension://kbhkgllmfhaneaaplcbnmilgdfbmlmhk',
	protocol: 'chrome-extension:',
	extensionBuild: true
};

describe('analyticsAllowed', () => {
	it('is the production wallet origin', () => {
		expect(SITE_ORIGIN).toBe('https://wallet.getvela.app');
		expect(analyticsAllowed(HOSTED)).toBe(true);
	});

	it('is the packaged extension', () => {
		expect(analyticsAllowed(EXTENSION)).toBe(true);
	});

	it.each([
		'http://localhost:4173',
		'http://localhost:5173',
		'http://127.0.0.1:4173',
		'https://vela-wallet.pages.dev',
		'https://preview.vela-wallet.workers.dev',
		'https://getvela.app',
		'https://app.getvela.app'
	])('is never %s', (origin) => {
		expect(analyticsAllowed({ ...HOSTED, origin, protocol: new URL(origin).protocol })).toBe(false);
	});

	it('is never the extension build served from anywhere but the extension', () => {
		expect(analyticsAllowed({ ...EXTENSION, origin: SITE_ORIGIN, protocol: 'https:' })).toBe(false);
		expect(
			analyticsAllowed({ ...EXTENSION, origin: 'http://localhost:4173', protocol: 'http:' })
		).toBe(false);
	});

	it('is never the hosted build opened as an extension page', () => {
		expect(analyticsAllowed({ ...EXTENSION, extensionBuild: false })).toBe(false);
	});

	it('is never under automation — the extension e2e runs the real package', () => {
		expect(analyticsAllowed({ ...HOSTED, automated: true })).toBe(false);
		expect(analyticsAllowed({ ...EXTENSION, automated: true })).toBe(false);
	});

	it('is never in the parallel space', () => {
		expect(analyticsAllowed({ ...EXTENSION, parallelSpace: true })).toBe(false);
	});

	it('is never once switched off', () => {
		expect(analyticsAllowed({ ...HOSTED, optedOut: true })).toBe(false);
		expect(analyticsAllowed({ ...EXTENSION, optedOut: true })).toBe(false);
	});
});
