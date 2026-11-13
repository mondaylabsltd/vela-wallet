/**
 * R22 (review 1-3): the rule of a held confirm, apart from its drawing — the
 * one reusable control every "… anyway" and danger confirm goes through (the
 * dApp sheet's, the hand-off card's Open, every multisig sign / propose /
 * approve / execute). HOW LONG it is held and whether it is held at all are
 * the core's (`ConfirmState.hold_ms`, `SignDoor.hold_ms`,
 * `holdToConfirmMs()`, `confirmAgainWindowMs()`); this only runs the gesture.
 *
 * - **A pointer or a touch holds.** A press starts the hold; the fill grows
 *   over `holdMs`; letting go early does nothing at all; a hold that
 *   completes confirms — and the click the browser sends after it is not a
 *   second confirm.
 * - **Assistive technology presses twice** (WCAG 2.5.1 / 2.5.7: a held
 *   gesture is never the only way). An activation no pointer press preceded
 *   — Enter or Space, a screen reader's or a switch's click — ARMS the
 *   confirm and says "Press again to confirm"; a second activation within
 *   `windowMs` confirms; past the window it disarms.
 * - `holdMs <= 0`: a plain button — one activation confirms.
 *
 * Time is injected (`timers`), so a test drives it by hand.
 */

export type HoldPhase = 'idle' | 'holding' | 'armed';

export interface HoldTimers {
	set(run: () => void, ms: number): unknown;
	clear(handle: unknown): void;
}

export const WALL_TIMERS: HoldTimers = {
	set: (run, ms) => setTimeout(run, ms),
	clear: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>)
};

export interface HoldConfirmOptions {
	holdMs: () => number;
	windowMs: () => number;
	onconfirm: () => void;
	/** Every change of phase, for the drawing (the fill, the armed words). */
	onphase?: (phase: HoldPhase) => void;
	timers?: HoldTimers;
}

export interface HoldConfirm {
	readonly phase: HoldPhase;
	/** A primary pointer (mouse button, touch, pen) went down on the control. */
	pressStart(): void;
	/** It came up, was cancelled, or left the control — before or after the hold. */
	pressEnd(): void;
	/** The control was activated (`click`): after a pointer press, or by assistive technology. */
	activate(): void;
	/** Disabled, or gone: nothing held, nothing armed. */
	reset(): void;
}

export function holdConfirm(options: HoldConfirmOptions): HoldConfirm {
	const timers = options.timers ?? WALL_TIMERS;
	let phase: HoldPhase = 'idle';
	let timer: unknown = null;
	/** A pointer press began since the last activation: its click is the pointer's. */
	let pointerGesture = false;

	function stop(): void {
		if (timer !== null) timers.clear(timer);
		timer = null;
	}

	function to(next: HoldPhase): void {
		if (phase === next) return;
		phase = next;
		options.onphase?.(next);
	}

	return {
		get phase() {
			return phase;
		},
		pressStart() {
			pointerGesture = true;
			if (options.holdMs() <= 0) return;
			stop();
			to('holding');
			timer = timers.set(() => {
				timer = null;
				to('idle');
				options.onconfirm();
			}, options.holdMs());
		},
		pressEnd() {
			// Released early: nothing happens, the fill goes back.
			if (phase !== 'holding') return;
			stop();
			to('idle');
		},
		activate() {
			const fromPointer = pointerGesture;
			pointerGesture = false;
			if (options.holdMs() <= 0) {
				options.onconfirm();
				return;
			}
			// A pointer's click: the hold confirmed already, or was let go of.
			if (fromPointer) return;
			if (phase === 'armed') {
				stop();
				to('idle');
				options.onconfirm();
				return;
			}
			stop();
			to('armed');
			timer = timers.set(() => {
				timer = null;
				to('idle');
			}, options.windowMs());
		},
		reset() {
			stop();
			pointerGesture = false;
			to('idle');
		}
	};
}
