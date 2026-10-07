/**
 * Whether the "Get Vela on your phone" prompt shows, and what it offers —
 * decided here, purely, so the gating is a test rather than a hope.
 *
 * - an iPhone or iPad is offered the App Store, an Android phone Google Play;
 * - a desktop browser and the extension (always a desktop: Chrome on a phone
 *   runs no extensions) are offered both, each with a code to scan, because
 *   the app is for the phone in the person's pocket, not this machine;
 * - no link for the platform, no prompt — which is every platform today
 *   (`config.ts`);
 * - closed once, never again on this device (localStorage, best-effort: a
 *   browser that cannot remember it shows it again, which is a nuisance and
 *   not a failure).
 */
import type { StoreLinks, StorePlacement } from './config';

export type AppPlatform = 'ios' | 'android' | 'desktop';
export type AppStore = 'app_store' | 'google_play';

export interface PromptPlan {
	platform: AppPlatform;
	stores: { store: AppStore; url: string }[];
	/** Show a code per store — the person scans it with their phone. */
	qr: boolean;
}

/** The platform the prompt speaks to. */
export function detectPlatform(
	userAgent: string,
	maxTouchPoints: number,
	extension: boolean
): AppPlatform {
	if (extension) return 'desktop';
	// iPadOS asks for the desktop site and says "Macintosh"; its touch points
	// give it away.
	if (/iPhone|iPad|iPod/.test(userAgent) || (/Macintosh/.test(userAgent) && maxTouchPoints > 1))
		return 'ios';
	if (/Android/.test(userAgent)) return 'android';
	return 'desktop';
}

/** A listing link worth drawing: https, and nothing else. */
function listing(url: string | null): string | null {
	if (url === null) return null;
	try {
		return new URL(url).protocol === 'https:' ? url : null;
	} catch {
		return null;
	}
}

/** What the prompt offers on `platform`, or `null` when it must not show. */
export function promptPlan(platform: AppPlatform, links: StoreLinks): PromptPlan | null {
	const stores: PromptPlan['stores'] = [];
	const appStore = listing(links.appStore);
	const googlePlay = listing(links.googlePlay);
	if (platform !== 'android' && appStore !== null)
		stores.push({ store: 'app_store', url: appStore });
	if (platform !== 'ios' && googlePlay !== null)
		stores.push({ store: 'google_play', url: googlePlay });
	if (stores.length === 0) return null;
	return { platform, stores, qr: platform === 'desktop' };
}

/** Closed on this device — from any placement — and so never shown again. */
export const DISMISSED_KEY = 'vela.appPrompt.dismissed';
/** The creation screen's suggestion was shown once; it is not shown twice. */
export const SUGGESTED_KEY = 'vela.appPrompt.suggested';

function readFlag(storage: Storage | undefined, key: string): boolean {
	try {
		return storage !== undefined && storage.getItem(key) !== null;
	} catch {
		return false;
	}
}

function writeFlag(storage: Storage | undefined, key: string): void {
	try {
		storage?.setItem(key, String(Date.now()));
	} catch {
		/* Unwritable storage: it may show again — cosmetic, not a failure. */
	}
}

/**
 * The wallet home (`home` on the web, `extension` in the extension's wallet)
 * or the screen that says a new wallet is ready.
 */
export type Placement = StorePlacement;

/**
 * Shown this visit by the creation screen: the home it hands over to does not
 * ask again a moment later.
 */
let suggestedThisVisit = false;

/** May the prompt show at `placement` (given there is a plan at all)? */
export function promptAllowed(placement: Placement, storage: Storage | undefined): boolean {
	if (readFlag(storage, DISMISSED_KEY)) return false;
	if (placement === 'create_done') return !readFlag(storage, SUGGESTED_KEY);
	return !suggestedThisVisit;
}

/** The prompt was shown at `placement`. */
export function notePromptShown(placement: Placement, storage: Storage | undefined): void {
	if (placement !== 'create_done') return;
	suggestedThisVisit = true;
	writeFlag(storage, SUGGESTED_KEY);
}

/**
 * The home card remounts every time the home comes back (a flow closing on
 * the phone); it is counted as shown once a visit, not once a mount.
 */
const countedThisVisit = new Set<Placement>();
export function firstShowThisVisit(placement: Placement): boolean {
	if (countedThisVisit.has(placement)) return false;
	countedThisVisit.add(placement);
	return true;
}

/** The person closed it: never again on this device. */
export function dismissPrompt(storage: Storage | undefined): void {
	writeFlag(storage, DISMISSED_KEY);
}

/** Tests only. */
export function resetPromptVisitForTests(): void {
	suggestedThisVisit = false;
	countedThisVisit.clear();
}
