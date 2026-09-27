/**
 * The screenshot viewer's gestures, as numbers (078 §C: "上传的截图要能点击放大
 * 预览吧"). Pure, so the rules are tested without a finger — and built on the
 * bottom sheet's own maths (`sheet-gesture.ts`): the same slop before a press
 * becomes a drag, the same velocity window, the same flick speed, the same
 * iOS rubber band. A viewer that answered a finger differently from the
 * sheet under it would feel like two apps.
 *
 * - **Sideways** pages between images, 1:1, rubber-banded past the first and
 *   the last; released past {@link PAGE_FRACTION} of the width or on a flick,
 *   it snaps to the neighbour, else back.
 * - **Down** (not zoomed) dismisses: the picture follows the finger and the
 *   black behind it fades; released past {@link VIEWER_DISMISS_FRACTION} of
 *   the height or on a downward flick, it closes, else springs back.
 * - **Zoomed**, a drag pans the picture — never pages, never dismisses — and
 *   the picture's edge never leaves the screen's edge.
 * - **Pinch** zooms about the fingers' midpoint, {@link MIN_ZOOM}–{@link MAX_ZOOM};
 *   **double tap** goes to {@link DOUBLE_TAP_ZOOM} about the tap, or back to 1.
 */
import {
	FLICK_MIN_DISTANCE,
	FLICK_VELOCITY,
	rubberBand,
	scrimOpacity,
	SLOP
} from '$lib/wallet/ui/sheet-gesture';

export { SLOP, VelocityTracker } from '$lib/wallet/ui/sheet-gesture';

/** Released past this fraction of the viewer's height: it closes (the spec's "~20%"). */
export const VIEWER_DISMISS_FRACTION = 0.2;

/** Released past this fraction of the width: the next (or previous) image. */
export const PAGE_FRACTION = 0.2;

export const MIN_ZOOM = 1;
export const MAX_ZOOM = 4;
/** Where a double tap zooms to. */
export const DOUBLE_TAP_ZOOM = 2;
/** Two taps within this long (ms)… */
export const DOUBLE_TAP_MS = 300;
/** …and this close (px) are one double tap. */
export const DOUBLE_TAP_DISTANCE = 24;
/** Below this, a pinch let go is no zoom at all. */
export const ZOOM_EPSILON = 1.02;

export interface Size {
	width: number;
	height: number;
}

export interface Point {
	x: number;
	y: number;
}

/** The picture's scale and its offset (px) from centred. */
export interface Zoom {
	scale: number;
	x: number;
	y: number;
}

export const UNZOOMED: Zoom = { scale: 1, x: 0, y: 0 };

/**
 * What a single finger's drag is for, once it has moved past the slop.
 * `none` is a push UP while not zoomed: nothing to do, and it must not page.
 */
export function viewerTakesDrag(input: {
	dx: number;
	dy: number;
	zoomed: boolean;
}): 'undecided' | 'pan' | 'page' | 'dismiss' | 'none' {
	const { dx, dy, zoomed } = input;
	if (Math.abs(dx) < SLOP && Math.abs(dy) < SLOP) return 'undecided';
	if (zoomed) return 'pan';
	if (Math.abs(dx) > Math.abs(dy)) return 'page';
	return dy > 0 ? 'dismiss' : 'none';
}

/** Where the strip of images sits while paging: 1:1, rubber-banded past either end. */
export function pageOffset(dx: number, index: number, count: number, width: number): number {
	if (dx > 0 && index <= 0) return rubberBand(dx, width);
	if (dx < 0 && index >= count - 1) return -rubberBand(-dx, width);
	return dx;
}

/**
 * On release: which image. A flick counts only in the direction the strip was
 * already moving — a finger that wobbles back at the end does not page.
 */
export function pageAfter(input: {
	index: number;
	count: number;
	offset: number;
	width: number;
	/** px/ms, positive = rightward (towards the previous image). */
	velocity: number;
}): number {
	const { index, count, offset, width, velocity } = input;
	const flick = Math.abs(velocity) > FLICK_VELOCITY && Math.abs(offset) >= FLICK_MIN_DISTANCE;
	let next = index;
	if (offset < -width * PAGE_FRACTION || (flick && velocity < 0 && offset < 0)) next = index + 1;
	else if (offset > width * PAGE_FRACTION || (flick && velocity > 0 && offset > 0))
		next = index - 1;
	return Math.max(0, Math.min(count - 1, next));
}

/** Where the picture sits while being dragged down: 1:1 down, rubber-banded back up. */
export function dismissOffset(dy: number, height: number): number {
	return dy >= 0 ? dy : -rubberBand(-dy, height);
}

/** On release: close, or spring back? */
export function shouldDismissViewer(input: {
	offset: number;
	height: number;
	/** px/ms, positive = downward. */
	velocity: number;
}): boolean {
	const { offset, height, velocity } = input;
	if (!(offset > 0)) return false;
	if (offset > height * VIEWER_DISMISS_FRACTION) return true;
	return velocity > FLICK_VELOCITY && offset >= FLICK_MIN_DISTANCE;
}

/**
 * How much of the black is left while the picture is dragged down. The
 * sheet's scrim maths over a shorter run, so the ground visibly gives way
 * well before the picture would reach the bottom.
 */
export function viewerBackdrop(offset: number, height: number): number {
	return scrimOpacity(offset, height * 0.6);
}

/** The picture shrinks a little as it is pulled away, as the platform viewers do. */
export function dismissScale(offset: number, height: number): number {
	if (!(offset > 0) || !(height > 0)) return 1;
	return 1 - Math.min(0.2, (offset / height) * 0.4);
}

/** The picture fitted into `box`, aspect kept — scaled up as well as down. */
export function fitSize(natural: Size, box: Size): Size {
	if (!(natural.width > 0 && natural.height > 0 && box.width > 0 && box.height > 0)) {
		return { width: box.width, height: box.height };
	}
	const scale = Math.min(box.width / natural.width, box.height / natural.height);
	return { width: natural.width * scale, height: natural.height * scale };
}

export function clampScale(scale: number): number {
	return Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, scale));
}

/** Keep a zoomed picture's edges at or beyond the screen's: never a gap at a side. */
export function clampPan(zoom: Zoom, fitted: Size, box: Size): Zoom {
	const maxX = Math.max(0, (fitted.width * zoom.scale - box.width) / 2);
	const maxY = Math.max(0, (fitted.height * zoom.scale - box.height) / 2);
	return {
		scale: zoom.scale,
		x: Math.max(-maxX, Math.min(maxX, zoom.x)),
		y: Math.max(-maxY, Math.min(maxY, zoom.y))
	};
}

/**
 * Zoom to `scale` keeping the picture's point under `focus` (px from the
 * box's centre) exactly where it is — what makes a pinch or a double tap
 * land on the thing the finger is on.
 */
export function zoomAbout(from: Zoom, scale: number, focus: Point): Zoom {
	const ratio = scale / from.scale;
	return {
		scale,
		x: focus.x - (focus.x - from.x) * ratio,
		y: focus.y - (focus.y - from.y) * ratio
	};
}

/** A double tap toggles: into {@link DOUBLE_TAP_ZOOM} about the tap, or back out to 1. */
export function doubleTapZoom(from: Zoom, focus: Point, fitted: Size, box: Size): Zoom {
	if (from.scale > ZOOM_EPSILON) return UNZOOMED;
	return clampPan(zoomAbout(from, DOUBLE_TAP_ZOOM, focus), fitted, box);
}

/** The second of two quick taps in nearly the same place. */
export function isDoubleTap(
	previous: { t: number; x: number; y: number } | null,
	now: { t: number; x: number; y: number }
): boolean {
	if (previous === null) return false;
	return (
		now.t - previous.t <= DOUBLE_TAP_MS &&
		Math.hypot(now.x - previous.x, now.y - previous.y) <= DOUBLE_TAP_DISTANCE
	);
}
