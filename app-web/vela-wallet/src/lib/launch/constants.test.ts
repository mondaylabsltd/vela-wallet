import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import {
	DURATION_MS,
	EXIT_CROSSFADE_MS,
	HARD_CEILING_MS,
	HOLD_MS,
	LARGE_BOX_W_RATIO,
	LARGE_CANVAS,
	LARGE_SCREEN_MIN_W,
	PHONE_BOX_W_RATIO,
	PHONE_CANVAS,
	REPLAY_AFTER_MS,
	STORAGE_KEY,
	isDue,
	SKIP_PARAM,
	assetUrl,
	boxSize,
	formFactor,
	markPlayed,
	shouldPlay
} from './constants';

/**
 * Spec 012 FR-011 and the shared time budget.
 *
 * The same tables are asserted in Rust, Swift and Kotlin. Repeating them in four
 * languages is the point: the four apps never run the same code, so a
 * transcription slip in any one of them has nothing else to catch it.
 */
describe('launch geometry', () => {
	/** Lockup width ÷ core canvas width — a property of the shipped assets. */
	const phoneLockupRatio = 314.85 / PHONE_CANVAS.w;
	const largeLockupRatio = 566.73 / LARGE_CANVAS.w;

	it('derives the box ratios from the assets rather than choosing them', () => {
		// core canvas ÷ full-bleed canvas, the same derivation
		// scripts/lint-lottie-assets.mjs re-checks against the files themselves.
		expect(PHONE_BOX_W_RATIO).toBeCloseTo(350 / 390, 6);
		expect(LARGE_BOX_W_RATIO).toBeCloseTo(680 / 1920, 6);
	});

	it('renders the core canvas one-to-one at the authored width', () => {
		const phone = boxSize(390, 'phone');
		expect(phone.w).toBeCloseTo(PHONE_CANVAS.w, 2);
		expect(phone.h).toBeCloseTo(PHONE_CANVAS.h, 2);

		const large = boxSize(1920, 'largeScreen');
		expect(large.w).toBeCloseTo(LARGE_CANVAS.w, 2);
		expect(large.h).toBeCloseTo(LARGE_CANVAS.h, 2);
	});

	it('holds the authored share of the viewport at every size', () => {
		// research D1: phone 80.73 % of width, large screen 29.52 %.
		for (const width of [320, 360, 390, 414, 430]) {
			const share = (boxSize(width, 'phone').w * phoneLockupRatio) / width;
			expect(share, `phone @${width}`).toBeCloseTo(0.8073, 3);
		}
		for (const width of [768, 1280, 1440, 1920, 3440]) {
			const share = (boxSize(width, 'largeScreen').w * largeLockupRatio) / width;
			expect(share, `large @${width}`).toBeCloseTo(0.2952, 3);
		}
	});

	it('uses the form-factor predicate shared by all four apps', () => {
		expect(formFactor(390, 844)).toBe('phone');
		expect(formFactor(430, 932)).toBe('phone');
		expect(formFactor(768, 1024)).toBe('largeScreen');
		expect(formFactor(1280, 800)).toBe('largeScreen');
		// Landscape wins regardless of width — a short wide window is a large
		// screen even below the threshold.
		expect(formFactor(700, 400)).toBe('largeScreen');
	});

	it('switches composition at its OWN threshold, not the page layout breakpoint', () => {
		// This app has two layouts and they switch at 1280 (--breakpoint-desktop),
		// but the composition switches at 768. Deliberate: between them a visitor
		// gets the mobile carousel and the large-screen lockup, which is correct —
		// the phone composition would put an ~800 px wordmark across a 1000 px
		// window. Pinned so nobody "tidies" the two thresholds into one.
		expect(LARGE_SCREEN_MIN_W).toBe(768);
		expect(formFactor(1000, 900)).toBe('largeScreen');
		const share = (boxSize(1000, 'largeScreen').w * largeLockupRatio) / 1000;
		expect(share).toBeCloseTo(0.2952, 3);
	});

	it('names an asset for every form factor and appearance', () => {
		const urls = new Set([
			assetUrl('phone', 'dark'),
			assetUrl('phone', 'light'),
			assetUrl('largeScreen', 'dark'),
			assetUrl('largeScreen', 'light')
		]);
		expect(urls.size, 'all four must be distinct assets').toBe(4);
		for (const url of urls) expect(url).toMatch(/vela-wallet-launch-.*core.*\.json/);
	});
});

describe('launch timing', () => {
	/**
	 * These are numbers the founder set by feel on running builds; a silent
	 * change to any of them changes the product, and nothing else would notice.
	 */
	it('matches the agreed transition shape', () => {
		expect(DURATION_MS).toBe(1700);
		expect(HOLD_MS).toBe(400);
		expect(EXIT_CROSSFADE_MS).toBe(400);

		const nominal = DURATION_MS + HOLD_MS + EXIT_CROSSFADE_MS;
		expect(nominal).toBe(2500);
		expect(HARD_CEILING_MS, 'the ceiling must leave room for the nominal sequence').toBeGreaterThan(
			nominal
		);
		expect(HARD_CEILING_MS - nominal, 'too little slack for a slow machine').toBeGreaterThanOrEqual(
			400
		);
	});
});

describe('replay gate', () => {
	function fakeWindow(search = '', storage: Record<string, string> | null = {}, pathname = '/en') {
		return {
			location: { search, pathname },
			localStorage: storage
				? {
						getItem: (k: string) => storage[k] ?? null,
						setItem: (k: string, v: string) => {
							storage[k] = v;
						}
					}
				: {
						getItem() {
							throw new Error('blocked');
						},
						setItem() {
							throw new Error('blocked');
						}
					}
		} as unknown as Window;
	}

	it('plays, then stays quiet for the cooldown, then plays again', () => {
		const store: Record<string, string> = {};
		const win = fakeWindow('', store);
		const t0 = 1_700_000_000_000;

		expect(shouldPlay(win, t0)).toBe(true);
		markPlayed(win, t0);
		expect(store[STORAGE_KEY]).toBe(String(t0));

		// A regular visitor never sits through it twice.
		expect(shouldPlay(win, t0 + 1000)).toBe(false);
		expect(shouldPlay(win, t0 + REPLAY_AFTER_MS - 1)).toBe(false);
		// Someone returning after a while still gets the brand moment.
		expect(shouldPlay(win, t0 + REPLAY_AFTER_MS)).toBe(true);
	});

	it('treats a corrupt timestamp as due rather than suppressing forever', () => {
		// A bad write must not silently disable the animation for good.
		expect(isDue('not-a-number', Date.now())).toBe(true);
		expect(isDue('', Date.now())).toBe(true);
		expect(isDue(null, Date.now())).toBe(true);
	});

	it('honours the deterministic skip used by tests (FR-029)', () => {
		expect(shouldPlay(fakeWindow(`?${SKIP_PARAM}`))).toBe(false);
		expect(shouldPlay(fakeWindow(`?locale=en&${SKIP_PARAM}=1`))).toBe(false);
	});

	it('never plays on the connect page, an outside owner’s first visit', () => {
		expect(shouldPlay(fakeWindow('', {}, '/en/connect'))).toBe(false);
		expect(shouldPlay(fakeWindow('', {}, '/zh-TW/connect/'))).toBe(false);
		expect(shouldPlay(fakeWindow('', {}, '/en/wallet'))).toBe(true);
	});

	/**
	 * The replay rule exists in TWO places: here, and inline in `app.html`, where
	 * the decision must be made before first paint and no module can be imported.
	 *
	 * A divergence is not a cosmetic bug — the inline script hides the page, and
	 * if the component then declines to play, the visitor gets a blank screen.
	 * So the duplication is allowed only because this test forbids drift.
	 */
	it('keeps the pre-paint copy of the rule in step with this one', () => {
		const html = readFileSync(new URL('../../app.html', import.meta.url), 'utf8');

		expect(html, 'the inline script must read the same key').toContain(STORAGE_KEY);
		expect(html, 'the inline script must honour the same skip param').toContain(SKIP_PARAM);
		expect(
			html,
			`the inline cooldown must be ${REPLAY_AFTER_MS} ms — update BOTH copies together`
		).toContain(String(REPLAY_AFTER_MS));
	});

	it('plays rather than breaking when storage throws', () => {
		// Private modes and sandboxed frames throw on sessionStorage. A wallet's
		// onboarding page must not fail because a decoration could not read a
		// flag; replaying is a cosmetic cost, a thrown error is not.
		expect(() => shouldPlay(fakeWindow('', null))).not.toThrow();
		expect(shouldPlay(fakeWindow('', null))).toBe(true);
		expect(() => markPlayed(fakeWindow('', null))).not.toThrow();
	});
});
