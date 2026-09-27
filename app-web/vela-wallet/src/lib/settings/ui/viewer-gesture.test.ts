/**
 * The screenshot viewer's gestures, as numbers (078 §C) — built on the bottom
 * sheet's own maths, so the two answer a finger the same way.
 */
import { describe, expect, it } from 'vitest';
import { FLICK_VELOCITY, SLOP } from '$lib/wallet/ui/sheet-gesture';
import {
	clampPan,
	clampScale,
	dismissOffset,
	dismissScale,
	DOUBLE_TAP_ZOOM,
	doubleTapZoom,
	fitSize,
	isDoubleTap,
	MAX_ZOOM,
	pageAfter,
	pageOffset,
	shouldDismissViewer,
	UNZOOMED,
	viewerBackdrop,
	viewerTakesDrag,
	zoomAbout
} from './viewer-gesture';

const BOX = { width: 390, height: 700 };

describe('what a drag is for', () => {
	it('is undecided inside the sheet’s slop', () => {
		expect(viewerTakesDrag({ dx: SLOP - 1, dy: SLOP - 1, zoomed: false })).toBe('undecided');
	});

	it('pages sideways, dismisses downwards, and ignores a push up', () => {
		expect(viewerTakesDrag({ dx: -40, dy: 5, zoomed: false })).toBe('page');
		expect(viewerTakesDrag({ dx: 4, dy: 40, zoomed: false })).toBe('dismiss');
		expect(viewerTakesDrag({ dx: 4, dy: -40, zoomed: false })).toBe('none');
	});

	it('only pans while zoomed — never pages, never dismisses', () => {
		expect(viewerTakesDrag({ dx: -80, dy: 0, zoomed: true })).toBe('pan');
		expect(viewerTakesDrag({ dx: 0, dy: 80, zoomed: true })).toBe('pan');
	});
});

describe('paging', () => {
	it('follows the finger inside the strip and rubber-bands past its ends', () => {
		expect(pageOffset(-60, 1, 3, 390)).toBe(-60);
		expect(pageOffset(60, 0, 3, 390)).toBeGreaterThan(0);
		expect(pageOffset(60, 0, 3, 390)).toBeLessThan(60);
		expect(pageOffset(-60, 2, 3, 390)).toBeGreaterThan(-60);
	});

	it('snaps to the neighbour past a fifth of the width, else back', () => {
		expect(pageAfter({ index: 1, count: 3, offset: -100, width: 390, velocity: 0 })).toBe(2);
		expect(pageAfter({ index: 1, count: 3, offset: 100, width: 390, velocity: 0 })).toBe(0);
		expect(pageAfter({ index: 1, count: 3, offset: -40, width: 390, velocity: 0 })).toBe(1);
	});

	it('a flick pages — only in the direction the strip was moving', () => {
		const fast = FLICK_VELOCITY * 2;
		expect(pageAfter({ index: 0, count: 3, offset: -30, width: 390, velocity: -fast })).toBe(1);
		// A wobble back at the end is not a flick the other way.
		expect(pageAfter({ index: 1, count: 3, offset: -30, width: 390, velocity: fast })).toBe(1);
	});

	it('never leaves the strip', () => {
		expect(pageAfter({ index: 2, count: 3, offset: -300, width: 390, velocity: -9 })).toBe(2);
		expect(pageAfter({ index: 0, count: 3, offset: 300, width: 390, velocity: 9 })).toBe(0);
	});
});

describe('swiping down to dismiss', () => {
	it('follows the finger down and rubber-bands up', () => {
		expect(dismissOffset(120, 700)).toBe(120);
		expect(dismissOffset(-120, 700)).toBeLessThan(0);
		expect(dismissOffset(-120, 700)).toBeGreaterThan(-120);
	});

	it('closes past a fifth of the height, or on a downward flick; else springs back', () => {
		expect(shouldDismissViewer({ offset: 150, height: 700, velocity: 0 })).toBe(true);
		expect(shouldDismissViewer({ offset: 100, height: 700, velocity: 0 })).toBe(false);
		expect(shouldDismissViewer({ offset: 40, height: 700, velocity: FLICK_VELOCITY * 2 })).toBe(
			true
		);
		expect(shouldDismissViewer({ offset: -40, height: 700, velocity: 5 })).toBe(false);
	});

	it('fades the black and shrinks the picture as it goes', () => {
		expect(viewerBackdrop(0, 700)).toBe(1);
		expect(viewerBackdrop(140, 700)).toBeLessThan(1);
		expect(viewerBackdrop(140, 700)).toBeGreaterThan(0);
		expect(viewerBackdrop(700, 700)).toBe(0);
		expect(dismissScale(0, 700)).toBe(1);
		expect(dismissScale(140, 700)).toBeLessThan(1);
		expect(dismissScale(10_000, 700)).toBeCloseTo(0.8);
	});
});

describe('zoom', () => {
	it('fits a picture into the screen, up as well as down, aspect kept', () => {
		expect(fitSize({ width: 1170, height: 2532 }, BOX)).toEqual({
			width: (1170 * 700) / 2532,
			height: 700
		});
		expect(fitSize({ width: 100, height: 50 }, BOX)).toEqual({ width: 390, height: 195 });
	});

	it('keeps the scale between 1 and the maximum', () => {
		expect(clampScale(0.4)).toBe(1);
		expect(clampScale(9)).toBe(MAX_ZOOM);
	});

	it('never lets a zoomed picture’s edge come away from the screen’s', () => {
		const fitted = { width: 390, height: 700 };
		expect(clampPan({ scale: 2, x: 999, y: -999 }, fitted, BOX)).toEqual({
			scale: 2,
			x: 195,
			y: -350
		});
		expect(clampPan({ scale: 1, x: 30, y: 30 }, fitted, BOX)).toEqual({ scale: 1, x: 0, y: 0 });
	});

	it('zooms about the fingers: the point under them stays under them', () => {
		const focus = { x: 100, y: -50 };
		const to = zoomAbout(UNZOOMED, 2, focus);
		// The picture point under `focus` was (focus − offset) / scale; still there.
		expect((focus.x - to.x) / to.scale).toBeCloseTo(focus.x);
		expect((focus.y - to.y) / to.scale).toBeCloseTo(focus.y);
	});

	it('a double tap zooms in to 2× about the tap, and a second one back out', () => {
		const fitted = { width: 390, height: 700 };
		const zoomed = doubleTapZoom(UNZOOMED, { x: 50, y: 60 }, fitted, BOX);
		expect(zoomed.scale).toBe(DOUBLE_TAP_ZOOM);
		expect(zoomed).toEqual({ scale: 2, x: -50, y: -60 });
		expect(doubleTapZoom(zoomed, { x: 0, y: 0 }, fitted, BOX)).toEqual(UNZOOMED);
	});

	it('knows a double tap: quick and close, not slow or far', () => {
		const first = { t: 0, x: 100, y: 100 };
		expect(isDoubleTap(null, first)).toBe(false);
		expect(isDoubleTap(first, { t: 200, x: 110, y: 104 })).toBe(true);
		expect(isDoubleTap(first, { t: 450, x: 100, y: 100 })).toBe(false);
		expect(isDoubleTap(first, { t: 100, x: 180, y: 100 })).toBe(false);
	});
});
