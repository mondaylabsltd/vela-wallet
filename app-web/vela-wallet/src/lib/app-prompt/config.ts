/**
 * Where the phone apps are listed — the ONE switch for the "Get Vela on your
 * phone" prompt (`AppPrompt.svelte`), and the campaign tags its links carry.
 *
 * Nothing here is filled in until the listings exist, and a prompt with no
 * link for the person's platform is never drawn (`plan.ts`): an iPhone needs
 * the App Store link, an Android phone the Google Play one, and a desktop
 * browser or the extension either (it shows what there is, with a code to
 * scan). Turning the prompt on at launch is setting these constants and
 * deploying — nothing else:
 *
 *   - App Store: `appId` (the number in `apps.apple.com/app/id<number>`) AND
 *     `providerToken` (App Store Connect → App Analytics → Campaigns, the
 *     `pt` value). Either missing, and there is no App Store link.
 *   - Google Play: flip `listed`. The package is the Android app's
 *     `applicationId` (`app-android/vela-wallet/app/build.gradle.kts`).
 *
 * Every link names the web wallet and the placement it was followed from, so
 * App Store Connect and the Play Console can tell a download that started
 * here from one that started on getvela.app (whose links say `site-…` /
 * `utm_source=getvela.app` instead):
 *
 *   - App Store: `ct=webwallet-<placement>` (Apple caps `ct` at 40 characters);
 *   - Google Play: the install referrer `utm_source=wallet.getvela.app`,
 *     `utm_medium=webwallet`, `utm_campaign=<placement>`.
 */
export type StorePlacement = 'home' | 'create_done' | 'extension';

export interface AppStoreConfig {
	appId: string | null;
	providerToken: string | null;
}

export interface GooglePlayConfig {
	packageId: string;
	listed: boolean;
}

export const APP_STORE: AppStoreConfig = {
	appId: null,
	providerToken: null
};

export const GOOGLE_PLAY: GooglePlayConfig = {
	packageId: 'app.getvela.wallet',
	listed: false
};

/** The two listings, for one placement — `null` where a store is not live. */
export interface StoreLinks {
	appStore: string | null;
	googlePlay: string | null;
}

/** Apple's limit on a campaign token. */
const CT_MAX = 40;

export function appStoreLink(
	placement: StorePlacement,
	config: AppStoreConfig = APP_STORE
): string | null {
	const { appId, providerToken } = config;
	if (appId === null || providerToken === null) return null;
	if (!/^\d+$/.test(appId) || !/^[A-Za-z0-9_-]+$/.test(providerToken)) return null;
	const ct = `webwallet-${placement}`.slice(0, CT_MAX);
	const query = new URLSearchParams({ pt: providerToken, ct, mt: '8' });
	return `https://apps.apple.com/app/id${appId}?${query}`;
}

export function googlePlayLink(
	placement: StorePlacement,
	config: GooglePlayConfig = GOOGLE_PLAY
): string | null {
	if (!config.listed || !/^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)+$/i.test(config.packageId))
		return null;
	const referrer = `utm_source=wallet.getvela.app&utm_medium=webwallet&utm_campaign=${placement}`;
	return (
		`https://play.google.com/store/apps/details?id=${config.packageId}` +
		`&referrer=${encodeURIComponent(referrer)}`
	);
}

/** Both listings for `placement`, as configured. */
export function storeLinks(placement: StorePlacement): StoreLinks {
	return { appStore: appStoreLink(placement), googlePlay: googlePlayLink(placement) };
}
