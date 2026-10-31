/**
 * The device harness's signing page (spec 102 R8 e2e) — a page on this
 * machine's loopback that onboarding creates and signs in on, as "Use my own
 * signing page" does in the apps.
 *
 * Why it exists: the Safari package sends every ceremony and signature to a
 * page, and a simulator cannot reach a `sign.getvela.app` served from this
 * machine (no DNS mapping without root) — while a loopback page is a real
 * origin where the simulator's own authenticator makes and uses real passkeys
 * (for `localhost`). So the harness creates the wallet ON that page, which
 * makes it a `localhost` wallet whose page is that page (R1–R3), and every
 * later signature goes there too.
 *
 * Only the development package's `dev-seed.html` writes the key (the release
 * package carries no such page); only a packaged extension page reads it; and
 * only a loopback address is honoured — anything else is no page at all.
 */
import { isPackagedApp } from '$lib/extension/page-url';

export const HARNESS_PAGE_KEY = 'vela.dev.signingPage';

const LOOPBACK = /^https?:\/\/(?:localhost|127\.0\.0\.1)(?::\d+)?\/?$/;

export function harnessSigningPage(): string | null {
	if (!isPackagedApp()) return null;
	try {
		const page = localStorage.getItem(HARNESS_PAGE_KEY);
		return page !== null && LOOPBACK.test(page) ? page : null;
	} catch {
		return null;
	}
}
