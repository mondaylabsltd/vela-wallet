/**
 * A list's scroll and gesture state, as the incoming check's cadence hears
 * it (r10 follow-up, docs/multisig-machines.md §9.4 / §10): `on` when a
 * scroll, a wheel or a drag starts on the home's or the history's list, and
 * `off` once it has settled — `scrollend`, or nothing moved for
 * [`GESTURE_SETTLE_MS`] with no finger down. The core holds a scan that
 * comes due meanwhile and runs it at `off`; when to scan stays the core's.
 *
 * Transport only: each edge is reported once (a scroll fires dozens of
 * events a second; the core needs the two edges), and an `on` is always
 * followed by its `off` — when the list goes away mid-gesture too.
 */
import type { Attachment } from 'svelte/attachments';

/**
 * How long a list must be still, with no finger on it, to count as settled
 * when no `scrollend` says so (a wheel at the list's end scrolls nothing; an
 * older browser has no `scrollend`). Long enough to bridge the gaps between
 * a wheel's notches and a fling's frames, short enough that a held scan
 * runs right after.
 */
export const GESTURE_SETTLE_MS = 250;

/** What the tracker needs of the page: a clock's timers. */
export interface GestureHost {
	setTimeout: (run: () => void, ms: number) => unknown;
	clearTimeout: (handle: unknown) => void;
}

const browserHost: GestureHost = {
	setTimeout: (run, ms) => setTimeout(run, ms),
	clearTimeout: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>)
};

/** The page's events, by what they say about the gesture. */
export type GestureInput = 'moved' | 'finger-down' | 'finger-up' | 'settled';

/**
 * The edges of one list's gesture, from its events. `report(on)` hears each
 * edge once; `stop()` ends a gesture still in progress.
 */
export class GestureTracker {
	#on = false;
	#fingers = false;
	#timer: unknown = null;
	readonly #report: (on: boolean) => void;
	readonly #host: GestureHost;

	constructor(report: (on: boolean) => void, host: GestureHost = browserHost) {
		this.#report = report;
		this.#host = host;
	}

	get on(): boolean {
		return this.#on;
	}

	input(what: GestureInput): void {
		switch (what) {
			case 'moved':
				if (!this.#on) {
					this.#on = true;
					this.#report(true);
				}
				this.#arm();
				return;
			case 'finger-down':
				// A finger resting on the list is not yet a gesture; a move is.
				this.#fingers = true;
				this.#disarm();
				return;
			case 'finger-up':
				// A fling may carry on; its scroll events keep the gesture, and
				// `scrollend` or the settle ends it.
				this.#fingers = false;
				if (this.#on) this.#arm();
				return;
			case 'settled':
				if (!this.#fingers) this.#end();
				return;
		}
	}

	/** The list went away: a gesture in progress ends with it. */
	stop(): void {
		this.#fingers = false;
		this.#end();
	}

	#end(): void {
		this.#disarm();
		if (!this.#on) return;
		this.#on = false;
		this.#report(false);
	}

	#arm(): void {
		this.#disarm();
		if (this.#fingers) return;
		this.#timer = this.#host.setTimeout(() => {
			this.#timer = null;
			this.input('settled');
		}, GESTURE_SETTLE_MS);
	}

	#disarm(): void {
		if (this.#timer !== null) this.#host.clearTimeout(this.#timer);
		this.#timer = null;
	}
}

/** The nearest box that scrolls `node`, or the document when none does. */
function scrollerOf(node: Element): EventTarget {
	for (let at = node.parentElement; at; at = at.parentElement) {
		const overflow = getComputedStyle(at).overflowY;
		if (overflow === 'auto' || overflow === 'scroll') return at;
	}
	return document;
}

/**
 * Follow a list's gesture: the list's own box scrolls (`'self'`, the home's)
 * or the nearest box around it does (`'around'`, the history inside a flow's
 * screen or the desktop's column). Wheel and touch count over the list only.
 */
export function listGesture(
	report: (on: boolean) => void,
	scroller: 'self' | 'around' = 'self'
): Attachment<HTMLElement> {
	return (node) => {
		const tracker = new GestureTracker(report);
		const box = scroller === 'self' ? node : scrollerOf(node);
		const passive: AddEventListenerOptions = { passive: true };
		const offs: (() => void)[] = [];
		const on = (target: EventTarget, type: string, run: () => void) => {
			target.addEventListener(type, run, passive);
			offs.push(() => target.removeEventListener(type, run, passive));
		};
		on(box, 'scroll', () => tracker.input('moved'));
		on(box, 'scrollend', () => tracker.input('settled'));
		on(node, 'wheel', () => tracker.input('moved'));
		on(node, 'touchstart', () => tracker.input('finger-down'));
		on(node, 'touchmove', () => tracker.input('moved'));
		on(node, 'touchend', () => tracker.input('finger-up'));
		on(node, 'touchcancel', () => tracker.input('finger-up'));
		return () => {
			for (const off of offs) off();
			tracker.stop();
		};
	};
}
