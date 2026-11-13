/**
 * The page's half of the account-wide incoming-funds check's cadence.
 *
 * r9-17: WHEN to scan is the core's (`incoming_scan::next_scan_ms`, driven by
 * `activity_feed`; r10 follow-up): a back-off while nothing arrives — 15 s,
 * 30 s, 60 s, then every 2 min; 2 min hidden — and at once when the page is
 * shown again or the browser comes back online. A touch no longer brings a
 * scan closer. This module only reports the page's facts (shown or
 * not, when it was last touched) and runs the one timer the core asks for:
 * the feed's `next_scan` is armed whenever its `seq` changes, and answered
 * with `scan_due` and the page as it stands then. One check at a time is the
 * core's too.
 */
import type { FeedNextScan } from '$lib/core/generated/FeedNextScan';
import type { FeedPresence } from '$lib/core/generated/FeedPresence';
import type { FeedPresenceChange } from '$lib/core/generated/FeedPresenceChange';

/**
 * A touch is reported at most this often: a scroll fires dozens of events a
 * second, and the core only needs to hear that someone is at the page — its
 * own window for that is two minutes. Transport, not a decision.
 */
export const INTERACTION_REPORT_MS = 1_000;

/** What the person does that counts as being at the page. */
const INTERACTIONS = ['pointerdown', 'keydown', 'wheel', 'touchstart'] as const;

/** The feed, as the cadence talks to it (`feed.svelte.ts`'s port). */
export interface CadencePort {
	/** The page's presence moved (`presence_changed`). */
	report(change: FeedPresenceChange, presence: FeedPresence): void;
	/** The timer for `next_scan.seq` ran out (`scan_due`). */
	due(seq: number, presence: FeedPresence): void;
	/** Follow the core's `next_scan`; returns the unsubscribe. */
	follow(listener: (next: FeedNextScan | null) => void): () => void;
}

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
 * Report the page to the core and run the timer it asks for, until the
 * returned stop is called. The first check is the account hand-off's; this
 * reports `started`, and the core arms the next.
 */
export function followIncomingCadence(
	port: CadencePort,
	host: CadenceHost = browserHost()
): () => void {
	let lastInteraction = host.now();
	let lastReport = -Infinity;
	let timer: unknown = null;
	let armedSeq: number | null = null;
	let stopped = false;

	const presence = (): FeedPresence => ({
		visible: host.visible(),
		since_interaction_ms: host.now() - lastInteraction,
		now_ms: host.now()
	});
	const report = (change: FeedPresenceChange) => {
		if (!stopped) port.report(change, presence());
	};

	const disarm = () => {
		if (timer !== null) host.clearTimeout(timer);
		timer = null;
	};
	const unfollow = port.follow((next) => {
		if (stopped || next?.seq === armedSeq) return;
		disarm();
		armedSeq = next?.seq ?? null;
		if (!next) return;
		const seq = next.seq;
		timer = host.setTimeout(() => {
			timer = null;
			if (!stopped) port.due(seq, presence());
		}, next.after_ms);
	});

	const offs = [
		host.on('document', 'visibilitychange', () => {
			if (host.visible()) {
				// Coming back counts as being at the page.
				lastInteraction = host.now();
				report('shown');
			} else {
				report('hidden');
			}
		}),
		host.on('window', 'online', () => report('online')),
		...INTERACTIONS.map((type) =>
			host.on('document', type, () => {
				lastInteraction = host.now();
				if (lastInteraction - lastReport < INTERACTION_REPORT_MS) return;
				lastReport = lastInteraction;
				report('interacted');
			})
		)
	];

	report('started');
	return () => {
		stopped = true;
		disarm();
		unfollow();
		for (const off of offs) off();
	};
}
