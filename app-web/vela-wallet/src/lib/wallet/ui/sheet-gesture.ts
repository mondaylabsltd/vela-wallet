/**
 * The bottom sheet's drag, as numbers (founder, 2026-09-27: "web 手机版本的
 * bottom sheet 弹框不一致，有些支持下拉滑动关闭，有些不支持 … 就跟 android ios
 * 一样"). Pure, so the rules are tested without a finger:
 *
 * - **Down** follows the finger 1:1 — unless the sheet may not close right now
 *   (`dismissible: false`, e.g. a signature in flight), when it resists like
 *   an over-scroll and always comes back.
 * - **Up** past the resting position rubber-bands: the sheet is already as
 *   high as it goes, and says so by following less the further it is pulled.
 * - **Release** closes past {@link DISMISS_FRACTION} of the sheet's height, or
 *   on a downward flick faster than {@link FLICK_VELOCITY}; anything else
 *   springs back. Those are the platform sheets' own numbers, near enough:
 *   UIKit's sheet and Material's modal bottom sheet both close on "about a
 *   quarter to a third down, or a fling".
 */

/** How far (px) a finger moves before a press becomes a drag. */
export const SLOP = 8;

/** Released past this fraction of the sheet's height: it closes. */
export const DISMISS_FRACTION = 0.28;

/** Released moving down faster than this (px/ms ≈ 500 px/s): it closes. */
export const FLICK_VELOCITY = 0.5;

/** A flick still has to have moved the sheet at least this far (px). */
export const FLICK_MIN_DISTANCE = 16;

/** How far back the velocity looks (ms): the last stretch of the gesture, not all of it. */
export const VELOCITY_WINDOW = 100;

/**
 * iOS's rubber band: the sheet follows `distance` less and less, approaching
 * `dimension` but never reaching it. `c` is UIKit's constant.
 */
export function rubberBand(distance: number, dimension: number, c = 0.55): number {
	if (!(distance > 0) || !(dimension > 0)) return 0;
	return (1 - 1 / ((distance * c) / dimension + 1)) * dimension;
}

/**
 * Where the sheet sits (px below rest; negative = above) while the finger is
 * `dy` below where the drag began.
 */
export function sheetOffset(dy: number, height: number, dismissible: boolean): number {
	if (dy >= 0) return dismissible ? dy : rubberBand(dy, height);
	return -rubberBand(-dy, height);
}

/** On release: close, or spring back? */
export function shouldDismiss(input: {
	offset: number;
	height: number;
	/** px/ms, positive = downward. */
	velocity: number;
	dismissible: boolean;
}): boolean {
	const { offset, height, velocity, dismissible } = input;
	if (!dismissible || !(offset > 0)) return false;
	if (offset > height * DISMISS_FRACTION) return true;
	return velocity > FLICK_VELOCITY && offset >= FLICK_MIN_DISTANCE;
}

/** How much the scrim still dims, 1 at rest down to 0 at a full sheet's height below. */
export function scrimOpacity(offset: number, height: number): number {
	if (!(offset > 0)) return 1;
	return Math.max(0, 1 - offset / Math.max(1, height));
}

/** The finger's recent speed, from timestamped positions. */
export class VelocityTracker {
	private samples: { t: number; y: number }[] = [];

	reset(t: number, y: number): void {
		this.samples = [{ t, y }];
	}

	add(t: number, y: number): void {
		this.samples.push({ t, y });
		const cutoff = t - VELOCITY_WINDOW;
		while (this.samples.length > 2 && this.samples[0].t < cutoff) this.samples.shift();
	}

	/** px/ms over the window, positive = downward; 0 when there is nothing to measure. */
	velocity(): number {
		if (this.samples.length < 2) return 0;
		const first = this.samples[0];
		const last = this.samples[this.samples.length - 1];
		const dt = last.t - first.t;
		return dt > 0 ? (last.y - first.y) / dt : 0;
	}
}

/**
 * Should a touch on the sheet's CONTENT become a sheet drag? Only a DOWNWARD
 * pull that began while the content was at its top. Everything else stays
 * the content's: a sideways move, a scroll that has somewhere to go, and any
 * push UP — even when the content cannot scroll. An upward fling that runs
 * the content into its top must never hand its leftover momentum to the
 * sheet (the Android sheet did, and overshot and jittered at its top anchor
 * forever); the scroll area's `overscroll-behavior: contain` swallows it.
 */
export function contentTakesDrag(input: {
	dx: number;
	dy: number;
	/** The content (and every scroller under the finger) was at its top when the touch began. */
	atTop: boolean;
}): 'sheet' | 'content' | 'undecided' {
	const { dx, dy, atTop } = input;
	if (Math.abs(dx) < SLOP && Math.abs(dy) < SLOP) return 'undecided';
	if (Math.abs(dx) > Math.abs(dy)) return 'content';
	return dy > 0 && atTop ? 'sheet' : 'content';
}
