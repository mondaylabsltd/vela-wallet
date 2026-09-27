/**
 * A control that acts on a TAP, never on a touch that turned into a scroll
 * (078, found on the iPhone 2026-09-27: a scroll that started on a
 * screenshot's remove badge deleted the screenshot — the badge's tap area
 * grows outward, right into where a thumb starts scrolling the sheet).
 *
 * Browsers usually drop the click once a touch has scrolled, but "usually" is
 * the bug: a pull the sheet claims for its own drag cancels the scroll, not
 * the click, and a slow drag can land inside the browser's own tap slop. So
 * the control decides for itself, from the pointer:
 *
 * - it moved past {@link SLOP} (the sheet's own "a press became a drag") →
 *   not a tap;
 * - the browser took the pointer for a pan or a scroll (`pointercancel`) →
 *   not a tap;
 * - anything else — a still press, the keyboard's Enter or Space (no pointer
 *   at all), a script's `click()` — is.
 *
 * The control keeps `touch-action` as it is, so a scroll that starts on it
 * still scrolls.
 */
import { SLOP } from '$lib/wallet/ui/sheet-gesture';

export interface TapGuard {
	down(event: PointerEvent): void;
	move(event: PointerEvent): void;
	cancel(event: PointerEvent): void;
	/** Is the click that just arrived a real tap (or the keyboard's)? */
	accept(event: MouseEvent): boolean;
}

export function tapGuard(slop: number = SLOP): TapGuard {
	let press: { id: number; x: number; y: number; spoiled: boolean } | null = null;
	return {
		down(event) {
			press = { id: event.pointerId, x: event.clientX, y: event.clientY, spoiled: false };
		},
		move(event) {
			if (press === null || event.pointerId !== press.id) return;
			if (Math.hypot(event.clientX - press.x, event.clientY - press.y) > slop) press.spoiled = true;
		},
		cancel(event) {
			if (press !== null && event.pointerId === press.id) press.spoiled = true;
		},
		accept(event) {
			const last = press;
			press = null;
			// The keyboard's activation carries no pointer (detail 0): always a tap.
			if (event.detail === 0 || last === null) return true;
			return !last.spoiled;
		}
	};
}
