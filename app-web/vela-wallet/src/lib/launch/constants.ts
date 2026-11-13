/**
 * Launch-animation constants, geometry and session gate (spec 012).
 *
 * Every value here is shared verbatim with the iOS, Android and desktop apps —
 * see `specs/012-launch-animation-lottie/data-model.md` §4. Repeating them in
 * four languages is deliberate: `launch.test.ts` asserts research D1's table, so
 * a transcription slip surfaces there rather than on a user's screen. Nothing
 * else would catch it — the four apps never run the same code.
 */

import phoneDark from '$animations/vela-wallet-launch-phone-core-dark.json?url';
import phoneLight from '$animations/vela-wallet-launch-phone-core-light.json?url';
import desktopDark from '$animations/vela-wallet-launch-desktop-core-dark.json?url';
import desktopLight from '$animations/vela-wallet-launch-desktop-core-light.json?url';

export type Appearance = 'dark' | 'light';
export type FormFactor = 'phone' | 'largeScreen';

// --- timing ----------------------------------------------------------------

/** Authored length of the animation: 102 frames ÷ 60 fps. */
export const DURATION_MS = 1700;
/**
 * Hold on the finished lockup before the hand-off, so the brand registers
 * instead of flashing past (FR-012a). Skippable by input; bypassed under
 * reduce-motion. Tried at 2000 ms and cut to 400 on seeing it run.
 */
export const HOLD_MS = 400;
/**
 * Cross-dissolve into the page — `motion.slow`. 180 ms (`motion.sheetOut`) was
 * the first choice and reads as a cut at full-screen scale, not a dissolve.
 */
export const EXIT_CROSSFADE_MS = 400;
/** FR-014: nothing presented by now → abandon the animation, show the page. */
export const FIRST_FRAME_BUDGET_MS = 400;
/**
 * FR-015: measured from the first presented frame, not from mount.
 * Nominal is 1700 play + 400 hold + 400 dissolve = 2500; the rest is slack.
 */
export const HARD_CEILING_MS = 3000;

// --- geometry --------------------------------------------------------------

/** Core canvases — the cropped framings that ship (research D0). */
export const PHONE_CANVAS = { w: 350, h: 120 } as const;
export const LARGE_CANVAS = { w: 680, h: 220 } as const;

/**
 * Form-factor threshold, in CSS pixels.
 *
 * Deliberately NOT the page's own `--breakpoint-desktop: 1280px`, even though
 * this app really does have two layouts. Those thresholds answer different
 * questions: 1280 is where six feature cards stop fitting as a 2×3 grid, while
 * this one is where the *authored composition* changes. Between 768 and 1280 a
 * visitor gets the page's mobile carousel and the large-screen lockup — which is
 * correct, because the phone composition puts the lockup at 80.7 % of viewport
 * width and at 1000 px that would be an 800 px wordmark across the screen.
 *
 * Tying the two together would make one of them wrong to keep the other tidy.
 */
export const LARGE_SCREEN_MIN_W = 768;

/**
 * Box width as a fraction of viewport width: the core canvas divided by the
 * full-bleed canvas it was cropped from. NOT a judgement call — at 390 px the
 * phone lockup lands at exactly the authored 80.7 % of screen width, and
 * `scripts/lint-lottie-assets.mjs` fails if a re-crop moves either number.
 */
export const PHONE_BOX_W_RATIO = 350 / 390;
export const LARGE_BOX_W_RATIO = 680 / 1920;

/** The shared predicate. */
export function formFactor(width: number, height: number): FormFactor {
	return width >= height || width >= LARGE_SCREEN_MIN_W ? 'largeScreen' : 'phone';
}

/**
 * Box size for a viewport, per the shared fit rule. Centred by the caller;
 * nothing is clipped or clamped, because the shipped asset is cropped to the
 * motion — the box *is* the artwork.
 */
export function boxSize(viewportWidth: number, form: FormFactor): { w: number; h: number } {
	const ratio = form === 'largeScreen' ? LARGE_BOX_W_RATIO : PHONE_BOX_W_RATIO;
	const canvas = form === 'largeScreen' ? LARGE_CANVAS : PHONE_CANVAS;
	const w = viewportWidth * ratio;
	return { w, h: (w * canvas.h) / canvas.w };
}

/**
 * Asset URLs. `?url` rather than a JSON import: the animations are emitted as
 * hashed static assets served from our own origin and fetched on demand, so
 * they never enter the JS bundle — and never come from a third-party CDN, which
 * is what the dotLottie runtime would have done by default (research D2).
 */
const ASSETS: Record<FormFactor, Record<Appearance, string>> = {
	phone: { dark: phoneDark, light: phoneLight },
	largeScreen: { dark: desktopDark, light: desktopLight }
};

export function assetUrl(form: FormFactor, appearance: Appearance): string {
	return ASSETS[form][appearance];
}

// --- replay gate -----------------------------------------------------------

/**
 * Where the "already seen it" timestamp lives.
 *
 * `localStorage`, not `sessionStorage`: a launch animation on every new tab of a
 * site someone visits often is the annoyance this gate exists to prevent
 * (founder direction, 2026-08-05).
 */
export const STORAGE_KEY = 'vela.launch.played';

/**
 * How long before it plays again. Long enough that a regular visitor never sits
 * through it twice; short enough that someone returning after a while still
 * gets the brand moment.
 *
 * The rule is the core's (`vela_core::launch`, `LAUNCH_ANIMATION_REPLAY_AFTER_MS`);
 * the native apps ask it. The web keeps two copies because it decides before
 * the core can load: here, and inline in `app.html` before first paint.
 * `constants.test.ts` runs both against the core's vectors, because a
 * divergence would mean the pre-paint script hides the page for an animation
 * the component then declines to play: a blank screen.
 */
export const REPLAY_AFTER_MS = 7 * 24 * 60 * 60 * 1000;

/** The deterministic disable for tests (FR-029). */
export const SKIP_PARAM = 'skipLaunch';

/**
 * The stored text as the core reads it (`stored_last_played_ms`): the epoch
 * milliseconds, or `null` when nothing was stored or what was stored is not a
 * finite number. A corrupt value is "never played" rather than a reason to
 * suppress the animation forever on a bad write.
 */
export function storedLastPlayedMs(storedValue: string | null): number | null {
	if (storedValue === null) return null;
	const last = Number(storedValue);
	return Number.isFinite(last) ? last : null;
}

/**
 * `vela_core::launch::launch_animation_due`, the rule every shell asks the
 * core. The web cannot ask it here: the decision is made before first paint
 * (`app.html`) and on Welcome, which never loads the core (`$lib/core/client`).
 * So this, like the inline copy, is held to the core's own vectors by
 * `constants.test.ts`, case for case.
 */
export function launchAnimationDue(
	lastPlayedMs: number | null,
	nowMs: number,
	disabled: boolean
): boolean {
	if (disabled) return false;
	if (lastPlayedMs === null || !Number.isFinite(lastPlayedMs)) return true;
	return nowMs - lastPlayedMs >= REPLAY_AFTER_MS;
}

/** The stored text against the core's rule. The inline script mirrors this exactly. */
export function isDue(storedValue: string | null, now: number): boolean {
	return launchAnimationDue(storedLastPlayedMs(storedValue), now, false);
}

export function shouldPlay(win: Window = window, now: number = Date.now()): boolean {
	try {
		if (new URLSearchParams(win.location.search).has(SKIP_PARAM)) return false;
		// The connect page (an outside owner's wallet) is no front door: no splash.
		if (/\/connect\/?$/.test(win.location.pathname)) return false;
		return isDue(win.localStorage.getItem(STORAGE_KEY), now);
	} catch {
		// Storage throws in private modes and sandboxed frames. A wallet's
		// onboarding page must not break because a decoration could not read a
		// flag — play it and move on.
		return true;
	}
}

export function markPlayed(win: Window = window, now: number = Date.now()): void {
	try {
		win.localStorage.setItem(STORAGE_KEY, String(now));
	} catch {
		// Same reasoning: unwritable storage means it may replay, which is a
		// cosmetic cost, not a failure.
	}
}
