/**
 * The harness's signing page (r11, Safari): which page a hand-off goes to.
 *
 * The development package takes it from the dev setting `dev-seed.html?page=`
 * writes — a loopback origin with a port, nothing else — in place of the
 * production page; a refused setting leaves production; the release package
 * never honours one, whatever its storage holds. Its "am I the development
 * package?" is its own manifest, read here from the real derivation
 * (`safariManifest`) of the real source manifest.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { isDevelopmentManifest, safariManifest } from '../../../extension/lib/safari-manifest.js';
import {
	chooseHarnessPage,
	handoffPage,
	handoffPageFor,
	harnessSigningPage,
	HARNESS_PAGE_KEY,
	loopbackPage,
	OFFICIAL_SIGNING_PAGE
} from './harness-page';

const APP_ROOT = join(import.meta.dirname, '../../..');
const SOURCE = JSON.parse(readFileSync(join(APP_ROOT, 'extension/manifest.json'), 'utf8'));
const DEV_MANIFEST = safariManifest(SOURCE, { development: true });
const RELEASE_MANIFEST = safariManifest(SOURCE, { development: false });

describe('a loopback page: the only page a dev setting may name', () => {
	it.each([
		['http://127.0.0.1:9694/', 'http://127.0.0.1:9694/'],
		['http://127.0.0.1:9694', 'http://127.0.0.1:9694/'],
		['http://localhost:5173/', 'http://localhost:5173/'],
		['https://localhost:8443', 'https://localhost:8443/'],
		['http://[::1]:9694/', 'http://[::1]:9694/'],
		['https://[::1]:8443/', 'https://[::1]:8443/']
	])('%s is accepted, as %s', (given, page) => {
		expect(loopbackPage(given)).toBe(page);
	});

	it.each([
		['the production page', 'https://sign.getvela.app/'],
		['another host', 'https://sign.example.com:8443/'],
		['a LAN address', 'http://192.168.1.20:9694/'],
		['a host that only starts like loopback', 'http://127.0.0.1.evil.example:9694/'],
		['loopback as credentials of another host', 'http://localhost:1@evil.example:9694/'],
		['no port', 'http://127.0.0.1/'],
		['a path', 'http://127.0.0.1:9694/b/abc/sign'],
		['a query', 'http://127.0.0.1:9694/?x=1'],
		['an empty query', 'http://127.0.0.1:9694/?'],
		['a fragment', 'http://127.0.0.1:9694/#x'],
		['credentials', 'http://user:pw@127.0.0.1:9694/'],
		['another scheme', 'ws://127.0.0.1:9694/'],
		['script', 'javascript:alert(1)'],
		['nothing', ''],
		['not a string', 42]
	])('%s is refused (%s)', (_what, given) => {
		expect(loopbackPage(given)).toBeNull();
	});
});

describe('the package: development honours the setting, release never does', () => {
	it('the development manifest says so; the release one, and Chrome’s, do not', () => {
		expect(isDevelopmentManifest(DEV_MANIFEST)).toBe(true);
		expect(isDevelopmentManifest(RELEASE_MANIFEST)).toBe(false);
		expect(isDevelopmentManifest(SOURCE)).toBe(false);
		expect(isDevelopmentManifest(null)).toBe(false);
		expect(isDevelopmentManifest({ web_accessible_resources: 'dev-seed.html' })).toBe(false);
	});

	it('development + loopback: that page', () => {
		expect(chooseHarnessPage('http://127.0.0.1:9694/', true)).toBe('http://127.0.0.1:9694/');
	});

	it('development + a page that is not loopback: none — production stays', () => {
		expect(chooseHarnessPage('https://evil.example:8443/', true)).toBeNull();
		expect(
			handoffPageFor(OFFICIAL_SIGNING_PAGE, chooseHarnessPage('https://evil.example:8443/', true))
		).toBe(OFFICIAL_SIGNING_PAGE);
	});

	it('release: none, even a loopback page', () => {
		expect(chooseHarnessPage('http://127.0.0.1:9694/', false)).toBeNull();
	});
});

describe('the page a hand-off goes to', () => {
	const HARNESS = 'http://127.0.0.1:9694/';

	it('the production page becomes the harness page', () => {
		expect(handoffPageFor('https://sign.getvela.app/', HARNESS)).toBe(HARNESS);
		expect(handoffPageFor('https://sign.getvela.app', HARNESS)).toBe(HARNESS);
	});

	it('with no harness page, production stays production', () => {
		expect(handoffPageFor('https://sign.getvela.app/', null)).toBe('https://sign.getvela.app/');
	});

	it('a page the account chose for itself is never replaced', () => {
		expect(handoffPageFor('https://sign.acme.example/', HARNESS)).toBe(
			'https://sign.acme.example/'
		);
		expect(handoffPageFor('http://localhost:7000/', HARNESS)).toBe('http://localhost:7000/');
	});
});

describe('in a packaged page: storage, manifest and protocol together', () => {
	class MemoryStorage {
		#map = new Map<string, string>();
		getItem(key: string) {
			return this.#map.get(key) ?? null;
		}
		setItem(key: string, value: string) {
			this.#map.set(key, value);
		}
	}

	function packaged(
		manifest: unknown,
		stored: string | null,
		protocol = __VELA_EXTENSION_PROTOCOL__
	) {
		const storage = new MemoryStorage();
		if (stored !== null) storage.setItem(HARNESS_PAGE_KEY, stored);
		vi.stubGlobal('localStorage', storage);
		vi.stubGlobal('location', { protocol });
		vi.stubGlobal('chrome', { runtime: { getManifest: () => manifest } });
	}

	afterEach(() => {
		vi.unstubAllGlobals();
	});

	it('the development package hands off to the dev page', () => {
		packaged(DEV_MANIFEST, 'http://localhost:9694/');
		expect(harnessSigningPage()).toBe('http://localhost:9694/');
		expect(handoffPage(OFFICIAL_SIGNING_PAGE)).toBe('http://localhost:9694/');
	});

	it('the development package refuses a page that is not loopback: production', () => {
		packaged(DEV_MANIFEST, 'https://sign.evil.example:443/');
		expect(harnessSigningPage()).toBeNull();
		expect(handoffPage(OFFICIAL_SIGNING_PAGE)).toBe(OFFICIAL_SIGNING_PAGE);
	});

	it('the release package ignores the setting: production', () => {
		packaged(RELEASE_MANIFEST, 'http://localhost:9694/');
		expect(harnessSigningPage()).toBeNull();
		expect(handoffPage(OFFICIAL_SIGNING_PAGE)).toBe(OFFICIAL_SIGNING_PAGE);
	});

	it('no manifest to read, or a hosted page: production', () => {
		packaged(undefined, 'http://localhost:9694/');
		expect(harnessSigningPage()).toBeNull();
		packaged(DEV_MANIFEST, 'http://localhost:9694/', 'https:');
		expect(harnessSigningPage()).toBeNull();
	});
});
