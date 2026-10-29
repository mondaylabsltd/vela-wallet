/**
 * How often the account-wide incoming-funds check runs (the feed's
 * `live_tick`: the receipt scan and the native balance watch). The core says
 * the cadence is the shell's — which page is visible, and whether anyone is
 * at it, is render-domain state it never sees (`app/activity_feed.rs`).
 *
 * Owner decision (round 5, 2026-10-30), by best practice:
 *
 * - **10 s** while the page is visible and the person interacted with it in
 *   the last {@link ACTIVE_WINDOW_MS} — someone waiting for a payment is
 *   looking at the screen and touching it;
 * - **30 s** while it is visible but idle — a home left open on a desk;
 * - **60 s** while it is hidden — a background tab still hears of money
 *   within a minute, without the steady drain;
 * - **at once** when the page is shown again or the browser comes back
 *   online — the moments a stale list would be seen or could be fixed — and
 *   the cadence starts over from there.
 *
 * One check at a time is the feed executor's (`scanOnce`); this only says
 * when to ask.
 */

/** While the person is at the page. */
export const ACTIVE_MS = 10_000;
/** Visible, nobody at it. */
export const IDLE_MS = 30_000;
/** Hidden. */
export const HIDDEN_MS = 60_000;
/** An interaction this recent counts as "at the page". */
export const ACTIVE_WINDOW_MS = 120_000;

export type CadenceReason = 'interval' | 'visible' | 'online';

/** The wait before the next check, for the page as it stands. */
export function incomingCadenceMs(state: { visible: boolean; sinceInteractionMs: number }): number {
	if (!state.visible) return HIDDEN_MS;
	return state.sinceInteractionMs < ACTIVE_WINDOW_MS ? ACTIVE_MS : IDLE_MS;
}

/** What the person does that counts as being at the page. */
const INTERACTIONS = ['pointerdown', 'keydown', 'wheel', 'touchstart'] as const;

export interface CadenceHost {
	now: () => number;
	visible: () => boolean;
	setTimeout: (run: () => void, ms: number) => unknown;
	clearTimeout: (handle: unknown) => void;
	/** Subscribe to the page's events; returns the unsubscribe. */
	on: (target: 'document' | 'window', type: string, run: () => void) => () => void;
}

function browserHost(): CadenceHost {
	return {
		now: () => Date.now(),
		visible: () => document.visibilityState === 'visible',
		setTimeout: (run, ms) => setTimeout(run, ms),
		clearTimeout: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>),
		on: (target, type, run) => {
			const at = target === 'document' ? document : window;
			const options: AddEventListenerOptions = { passive: true, capture: true };
			at.addEventListener(type, run, options);
			return () => at.removeEventListener(type, run, options);
		}
	};
}

/**
 * Run `check` on the cadence above until the returned stop is called. The
 * first check is the caller's (the account hand-off already scans); this
 * schedules the ones after it.
 */
export function followIncomingCadence(
	check: (reason: CadenceReason) => void,
	host: CadenceHost = browserHost()
): () => void {
	let lastInteraction = host.now();
	/** The last check: the caller's first one is taken as now. */
	let lastCheck = host.now();
	let timer: unknown = null;
	let dueAt = 0;
	let stopped = false;

	const wait = () =>
		incomingCadenceMs({
			visible: host.visible(),
			sinceInteractionMs: host.now() - lastInteraction
		});

	const schedule = (ms: number) => {
		if (timer !== null) host.clearTimeout(timer);
		dueAt = host.now() + ms;
		timer = host.setTimeout(() => {
			timer = null;
			if (stopped) return;
			lastCheck = host.now();
			check('interval');
			schedule(wait());
		}, ms);
	};

	const now = (reason: CadenceReason) => {
		if (stopped) return;
		lastCheck = host.now();
		check(reason);
		schedule(wait());
	};

	const offs = [
		host.on('document', 'visibilitychange', () => {
			if (host.visible()) {
				// Coming back counts as being at the page.
				lastInteraction = host.now();
				now('visible');
			} else {
				// Hidden: the hidden pace, counted from the last check.
				schedule(Math.max(0, lastCheck + HIDDEN_MS - host.now()));
			}
		}),
		host.on('window', 'online', () => now('online')),
		...INTERACTIONS.map((type) =>
			host.on('document', type, () => {
				const wasIdle = host.now() - lastInteraction >= ACTIVE_WINDOW_MS;
				lastInteraction = host.now();
				// Back from idle: the next check within the active pace, not
				// the rest of an idle wait.
				if (wasIdle && host.visible() && dueAt - host.now() > ACTIVE_MS) schedule(ACTIVE_MS);
			})
		)
	];

	schedule(wait());
	return () => {
		stopped = true;
		if (timer !== null) host.clearTimeout(timer);
		timer = null;
		for (const off of offs) off();
	};
}
