/**
 * Usage analytics for the web wallet and the browser extension — a small
 * first-party sender, not Rybbit's script.
 *
 * Nothing remote runs in a wallet page: MV3 and the Chrome Web Store forbid
 * remote code, and a wallet should not run a third party's script anyway. This
 * module builds the hit (`payload.ts`, where the privacy rules live and are
 * tested) and POSTs it to Rybbit's ingestion endpoint itself, exactly as the
 * script would — `fetch` with `keepalive`, so a hit sent as the page leaves
 * still goes. It never throws, never waits, and never retries: a lost hit is
 * a statistic, not a failure.
 *
 * The extension reaches the endpoint because its manifest's host permission
 * for every site exempts its own pages from CORS — Rybbit's CORS answer does
 * not list `chrome-extension:` origins (`extension/package.test.ts` pins both
 * the permission and the absence of any remote script).
 */
import { browser } from '$app/environment';
import { parallelFlagSet } from '$lib/dev/parallel-flag.svelte';
import type { AnalyticsEventName, AnalyticsEventProps } from './catalog';
import { analyticsConsent } from './consent.svelte';
import { analyticsAllowed } from './gate';
import { RYBBIT_TRACK_URL, buildPayload, type Hit, type PageContext } from './payload';

export type { AnalyticsEventName, AnalyticsEventProps } from './catalog';

/** The route the last page view named — events report the page they happened on. */
let currentRouteId: string | null = null;

function active(): boolean {
	if (!browser) return false;
	return analyticsAllowed({
		origin: location.origin,
		protocol: location.protocol,
		extensionBuild: __VELA_EXTENSION__,
		automated: navigator.webdriver === true,
		parallelSpace: parallelFlagSet(),
		optedOut: !analyticsConsent.allowed()
	});
}

function context(): PageContext {
	return {
		routeId: currentRouteId,
		pathname: location.pathname,
		search: location.search,
		hostname: location.hostname,
		origin: location.origin,
		language: navigator.language ?? '',
		screenWidth: screen.width,
		screenHeight: screen.height,
		referrer: document.referrer,
		surface: __VELA_EXTENSION__ ? 'extension' : 'web'
	};
}

/**
 * The same hit twice inside a second is one hit counted twice — an effect
 * that re-ran, a double tap. Keyed on the whole body, so a different chain or
 * outcome is never folded in.
 */
const DEDUPE_MS = 1000;
const recent = new Map<string, number>();

function send(hit: Hit): void {
	try {
		if (!active()) return;
		const payload = buildPayload(hit, context());
		if (payload === null) return;
		const body = JSON.stringify(payload);
		const now = Date.now();
		const last = recent.get(body);
		if (last !== undefined && now - last < DEDUPE_MS) return;
		recent.set(body, now);
		if (recent.size > 64) recent.clear();
		void fetch(RYBBIT_TRACK_URL, {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body,
			mode: 'cors',
			keepalive: true,
			credentials: 'omit',
			// The payload names the page; the request itself names nothing.
			referrerPolicy: 'no-referrer'
		}).catch(() => {});
	} catch {
		/* Analytics never breaks the wallet. */
	}
}

/** A page view — called after every navigation by the root layout. */
export function trackPageview(routeId: string | null): void {
	currentRouteId = routeId;
	send({ type: 'pageview' });
}

/** A named moment, with only the coarse properties its catalog entry allows. */
export function track<E extends AnalyticsEventName>(
	name: E,
	properties?: AnalyticsEventProps<E>
): void {
	send({ type: 'custom_event', name, properties });
}
