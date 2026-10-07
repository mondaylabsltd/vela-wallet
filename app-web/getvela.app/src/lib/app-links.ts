/**
 * Where this site sends people to get Vela — the web wallet, and the two
 * phone stores — with the campaign tags that let each place count the visit
 * as coming from here.
 *
 * - The web wallet (`wallet.getvela.app`) reads `utm_source=getvela.app`,
 *   `utm_medium=site`, `utm_campaign=<location>` and reports them with its own
 *   usage statistics, so a visit sent from a button here can be followed into
 *   the wallet.
 * - The stores: App Store links carry `ct=site-<location>` (App Store
 *   Connect's campaign token, at most 40 characters) and Google Play links the
 *   install referrer `utm_source=getvela.app&utm_medium=site&utm_campaign=…`.
 *   The web wallet's own links say `webwallet-…` / `wallet.getvela.app`, so the
 *   two paths stay apart in both consoles.
 *
 * The stores are not live yet: every store link is `null` — and the page keeps
 * saying "coming soon" — until the constants below are filled in. Launch is a
 * constants change: the App Store app id and provider token (`pt`, App Store
 * Connect → App Analytics → Campaigns), and `listed` for Google Play (the
 * package is the Android app's `applicationId`).
 */

export const WEB_WALLET = 'https://wallet.getvela.app/';

export interface AppStoreConfig {
	appId: string | null;
	providerToken: string | null;
}

export interface GooglePlayConfig {
	packageId: string;
	listed: boolean;
}

export const APP_STORE: AppStoreConfig = { appId: null, providerToken: null };

export const GOOGLE_PLAY: GooglePlayConfig = { packageId: 'app.getvela.wallet', listed: false };

export type Store = 'app_store' | 'google_play';

/** The web wallet, tagged with the place on this site that linked to it. */
export function webWalletLink(location: string): string {
	const query = new URLSearchParams({
		utm_source: 'getvela.app',
		utm_medium: 'site',
		utm_campaign: location
	});
	return `${WEB_WALLET}?${query}`;
}

export function appStoreLink(location: string, config: AppStoreConfig = APP_STORE): string | null {
	const { appId, providerToken } = config;
	if (appId === null || providerToken === null) return null;
	if (!/^\d+$/.test(appId) || !/^[A-Za-z0-9_-]+$/.test(providerToken)) return null;
	const query = new URLSearchParams({
		pt: providerToken,
		ct: `site-${location}`.slice(0, 40),
		mt: '8'
	});
	return `https://apps.apple.com/app/id${appId}?${query}`;
}

export function googlePlayLink(
	location: string,
	config: GooglePlayConfig = GOOGLE_PLAY
): string | null {
	if (!config.listed || !/^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)+$/i.test(config.packageId))
		return null;
	const referrer = new URLSearchParams({
		utm_source: 'getvela.app',
		utm_medium: 'site',
		utm_campaign: location
	});
	return (
		`https://play.google.com/store/apps/details?id=${config.packageId}` +
		`&referrer=${encodeURIComponent(referrer.toString())}`
	);
}

/** The live store links for `location`, App Store first — empty until launch. */
export function storeLinks(location: string): { store: Store; url: string }[] {
	const links: { store: Store; url: string }[] = [];
	const apple = appStoreLink(location);
	const play = googlePlayLink(location);
	if (apple !== null) links.push({ store: 'app_store', url: apple });
	if (play !== null) links.push({ store: 'google_play', url: play });
	return links;
}

/**
 * A named event through the page's Rybbit tag (`window.rybbit`), for a link
 * that already carries a `data-rybbit-event` of its own — Rybbit fires only
 * the nearest one, so a second event has to be sent by hand. Nothing happens
 * where the tag is not loaded (the chain-setup page).
 */
export function rybbitEvent(name: string, properties: Record<string, string>): void {
	const rybbit = (globalThis as { rybbit?: { event?: (n: string, p: object) => void } }).rybbit;
	try {
		rybbit?.event?.(name, properties);
	} catch {
		/* Analytics never breaks a link. */
	}
}
