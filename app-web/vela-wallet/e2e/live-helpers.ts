/**
 * Shared plumbing for the live-wiring e2e suites (spec 024 T040/T041).
 *
 * A fresh Playwright context is a first-run browser: no intro flag, no
 * wallet. These helpers make it a RETURNING one — the intro marked seen and
 * a minimal legacy-shape account seeded under the session core's own keys —
 * so the guarded routes rule `wallet` and the live screens render. The
 * account is storage-only test identity: nothing on these screens calls a
 * chain.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import type { Page } from '@playwright/test';

const APP_ROOT = join(import.meta.dirname, '..');

/** Dotted-path reader over the generated EN catalog — assertions speak the
 *  corpus, never hardcoded copy. */
export function en(path: string): string {
	const raw = JSON.parse(
		readFileSync(join(APP_ROOT, '..', '..', 'assets', 'i18n', 'en.json'), 'utf8')
	) as Record<string, unknown>;
	const value = path.split('.').reduce<unknown>((node, key) => {
		if (node === null || typeof node !== 'object') return undefined;
		return (node as Record<string, unknown>)[key];
	}, raw);
	if (typeof value !== 'string') throw new Error(`en corpus has no string at ${path}`);
	return value;
}

/** A legacy single-key account record, exactly the stored shape. */
const TEST_ACCOUNT = {
	id: 'e2e-credential-id',
	name: 'E2E Wallet',
	address: '0x14fb1f4e2b9c7a5d8e3f6a1b4c7d9e2f5a8b1d1e',
	public_key_hex: '04' + 'ab'.repeat(64),
	created_at_iso: '2026-01-01T00:00:00.000Z',
	keys: []
};

/**
 * The address the core DERIVES for TEST_ACCOUNT (address = f(keys), spec 019
 * invariant ②) — the stored `address` field above is ignored on load. What the
 * live surfaces show is this, never the fixture's.
 */
export const TEST_ACCOUNT_SHORT = '0x0cE19C…084e2e';

/**
 * The same address in full. It lives here because the shortened form is a
 * TRAP: spec 028's receive-code test was first written against a middle
 * reconstructed from `0x0cE19C…084e2e`, which is unguessable by construction.
 * Anything asserting the whole address reads it from here.
 */
export const TEST_ACCOUNT_ADDRESS = '0x0cE19Cc09A0b561B1AB9ee3B88C93685F5084e2e';

/** Runs before every document in the context: intro seen + wallet present. */
export async function seedSignedIn(page: Page): Promise<void> {
	await page.addInitScript((account) => {
		window.localStorage.setItem('vela.intro.seen', String(Date.now()));
		// Only when the profile has no wallet yet. This script runs before EVERY
		// document, so an unconditional write would re-impose this account on
		// every navigation — including one that deliberately swapped the wallet
		// (the parallel space), which then looked like the swap had failed.
		if (window.localStorage.getItem('vela.accounts') === null) {
			window.localStorage.setItem('vela.accounts', JSON.stringify([account]));
			window.localStorage.setItem('vela.activeAccountIndex', '0');
		}
	}, TEST_ACCOUNT);
}

/**
 * Every `.js` chunk the page loads from now on, by URL.
 *
 * A budget assertion needs to know what a visit actually PAID for, not what
 * the bundler could have split — so the list comes from the network, and the
 * bodies come from disk (below).
 */
export function collectScripts(page: Page): string[] {
	const scripts: string[] = [];
	page.on('response', (response) => {
		const url = response.url();
		if (url.endsWith('.js')) scripts.push(url);
	});
	return scripts;
}

/**
 * A loaded chunk's source, straight off the build output.
 *
 * Read from `.svelte-kit/output/client` rather than re-fetched: the preview
 * worker is single-threaded, and a burst of body fetches from six parallel
 * workers starves the other suites (found in the 026 full matrix). It is also
 * the stronger assertion — this is the artifact, not a response about it.
 */
export function chunkSource(url: string): string {
	const path = new URL(url).pathname.replace(/^\//, '');
	// `.svelte-kit/cloudflare` is what the preview SERVES. `output/client` is
	// not: since 027 the extension build runs after the web build and re-emits
	// it under `app/` (Chrome refuses `_`-prefixed paths), so a served
	// `/_app/…` URL never resolved there and every chunk read back as '' —
	// which made every `chunksCarrying` budget in this suite pass vacuously
	// for two specs. Found by 028's decoder budget, whose positive control
	// (open the scanner, the decoders MUST appear) is the only reason it could
	// not pass by accident.
	for (const root of ['.svelte-kit/cloudflare', '.svelte-kit/output/client']) {
		try {
			return readFileSync(join(process.cwd(), root, path), 'utf8');
		} catch {
			/* try the next root */
		}
	}
	return '';
}

/** The subset of `urls` whose chunk source carries `needle` — a budget leak. */
export function chunksCarrying(urls: string[], needle: RegExp | string): string[] {
	const match =
		typeof needle === 'string' ? (s: string) => s.includes(needle) : (s: string) => needle.test(s);
	return urls.filter((url) => match(chunkSource(url)));
}
