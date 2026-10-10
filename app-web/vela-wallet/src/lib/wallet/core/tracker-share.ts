/**
 * One transaction tracker for the whole extension (PR 2 note 12).
 *
 * The tracker resident is a module singleton — one per DOCUMENT. In the
 * extension the side panel (the wallet) runs one, and since the correctness
 * batch the request window's signing resident boots one too (it needs the
 * account's in-flight operations to hold a second confirm). Each sweeps the
 * stored pending records and polls every one of them: with the panel open, a
 * request window doubled every relay status and receipt read for its whole
 * life, and two documents patched the same records at once.
 *
 * So the documents that run the wallet are VOICES: each runs its own tracker
 * as before, holds a shared Web Lock while it lives, and says every view on a
 * `BroadcastChannel`. The request window is a FOLLOWER: when a voice is up it
 * runs no tracker of its own — it mirrors the voice's views (so the landing,
 * the answer's `op_tracked` and the in-flight hold all read the one tracker)
 * and hands its own operations to that voice. With no voice up it runs its
 * own, as before; it steps down when a voice appears, and takes over (its
 * exclusive lock request granted) when the last voice leaves — replaying the
 * hand-offs it made, which the core takes idempotently by hash.
 *
 * Pure plumbing over two injected browser seams, so it is tested without a
 * browser. No rule about an operation lives here: what is tracked and what it
 * means stay the core's.
 */
import type { TrackEvent } from '$lib/core/generated/TrackEvent';
import type { TrackView } from '$lib/core/generated/TrackView';

export const TRACKER_CHANNEL = 'vela.tx-tracker';
export const TRACKER_LOCK = 'vela.tx-tracker';

export type ShareMessage =
	/** A document running the tracker is up. */
	| { kind: 'voice'; from: string }
	/** A voice's committed view. */
	| { kind: 'view'; from: string; view: TrackView }
	/** A follower asks the voices for their view. */
	| { kind: 'hello'; from: string }
	/** A follower's operation for a voice's tracker (`to: '*'`: any voice). */
	| { kind: 'event'; to: string; event: TrackEvent }
	/** A voice is going (its document is unloading). */
	| { kind: 'bye'; from: string };

/** The part of `BroadcastChannel` this uses. */
export interface ShareChannel {
	postMessage(message: ShareMessage): void;
	onmessage: ((event: { data: unknown }) => void) | null;
	close(): void;
}

/** The part of `navigator.locks` this uses. */
export interface ShareLocks {
	request(
		name: string,
		options: { mode: 'shared' | 'exclusive' },
		callback: () => Promise<unknown>
	): Promise<unknown>;
	query(): Promise<{ held?: { name?: string; mode?: string }[] }>;
}

export interface ShareDeps {
	channel(): ShareChannel | null;
	locks(): ShareLocks | null;
	/** This document's id on the channel. */
	id?: () => string;
	/** Fired when the document goes; returns the unsubscribe. */
	onLeave?: (fire: () => void) => () => void;
}

/** The browser's own seams: `null` where either is missing (then nothing is shared). */
export function browserShareDeps(): ShareDeps {
	return {
		channel: () =>
			typeof BroadcastChannel === 'function'
				? (new BroadcastChannel(TRACKER_CHANNEL) as unknown as ShareChannel)
				: null,
		locks: () =>
			typeof navigator !== 'undefined' &&
			(navigator as unknown as { locks?: ShareLocks }).locks !== undefined
				? (navigator as unknown as { locks: ShareLocks }).locks
				: null,
		id: () =>
			typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function'
				? crypto.randomUUID()
				: `doc-${Date.now()}-${Math.random().toString(36).slice(2)}`,
		onLeave: (fire) => {
			if (typeof addEventListener !== 'function') return () => {};
			addEventListener('pagehide', fire);
			return () => removeEventListener('pagehide', fire);
		}
	};
}

function isShareMessage(data: unknown): data is ShareMessage {
	return (
		typeof data === 'object' &&
		data !== null &&
		typeof (data as { kind?: unknown }).kind === 'string'
	);
}

export interface Voice {
	/** Say a committed view. */
	view(view: TrackView): void;
	stop(): void;
}

/**
 * Run as a voice: hold the shared lock while this document lives, say every
 * view, answer a follower's hello with the current one, and take its events
 * into this tracker. `null` when the browser has no channel or no locks.
 */
export function startVoice(
	deps: ShareDeps,
	hooks: { current(): TrackView; dispatch(event: TrackEvent): void }
): Voice | null {
	const channel = deps.channel();
	const locks = deps.locks();
	if (!channel || !locks) {
		channel?.close();
		return null;
	}
	const id = deps.id?.() ?? 'voice';
	let release: () => void = () => {};
	void locks
		.request(TRACKER_LOCK, { mode: 'shared' }, () => new Promise<void>((done) => (release = done)))
		.catch(() => {});
	let stopped = false;
	const say = (message: ShareMessage) => {
		if (!stopped) channel.postMessage(message);
	};
	channel.onmessage = ({ data }) => {
		if (!isShareMessage(data)) return;
		if (data.kind === 'hello') say({ kind: 'view', from: id, view: hooks.current() });
		else if (data.kind === 'event' && (data.to === id || data.to === '*'))
			hooks.dispatch(data.event);
	};
	say({ kind: 'voice', from: id });
	say({ kind: 'view', from: id, view: hooks.current() });
	const stop = () => {
		if (stopped) return;
		say({ kind: 'bye', from: id });
		stopped = true;
		release();
		channel.onmessage = null;
		channel.close();
		unlisten();
	};
	const unlisten = deps.onLeave?.(stop) ?? (() => {});
	return {
		view: (view) => say({ kind: 'view', from: id, view }),
		stop
	};
}

export interface FollowerHooks {
	/** Run this document's own tracker (and replay `events` into it). */
	own(events: TrackEvent[]): void;
	/** Stop this document's own tracker: a voice runs the one tracker now. */
	disown(): void;
	/** A voice's view, as this document's. */
	mirror(view: TrackView): void;
}

export interface Follower {
	/** `'follow'` while a voice is followed, else `'own'`. */
	readonly mode: 'follow' | 'own';
	/**
	 * An operation for the tracker. Following: handed to the voice (and kept,
	 * for a takeover's replay) — `true`. Own: `false`, the caller dispatches it
	 * into its own tracker.
	 */
	forward(event: TrackEvent): boolean;
	stop(): void;
}

/** The events a follower hands over and replays — what makes an op tracked. */
function kept(event: TrackEvent): boolean {
	return event.type === 'submitted' || event.type === 'withdrawn' || event.type === 'abort';
}

/**
 * Run as the request window: follow a voice when one is up, else run its own
 * tracker; step down when a voice appears, take over when the last one goes.
 * Resolves once the first mode is decided. With no channel or no locks it
 * simply runs its own tracker, as every document did before.
 */
export async function startFollower(deps: ShareDeps, hooks: FollowerHooks): Promise<Follower> {
	const channel = deps.channel();
	const locks = deps.locks();
	if (!channel || !locks) {
		channel?.close();
		hooks.own([]);
		return { mode: 'own', forward: () => false, stop: () => {} };
	}
	const id = deps.id?.() ?? 'follower';
	let mode: 'follow' | 'own' = 'own';
	let voice: string | null = null;
	let stopped = false;
	/** The hand-offs this document made, for a takeover's replay. */
	const handed: TrackEvent[] = [];
	/** Holding the exclusive lock (a takeover): its release. */
	let release: (() => void) | null = null;
	let waiting = false;

	const say = (message: ShareMessage) => {
		if (!stopped) channel.postMessage(message);
	};

	/** Queue for the exclusive lock: granted once no voice holds the shared one. */
	function awaitTakeover(): void {
		if (waiting || release !== null) return;
		waiting = true;
		void locks!
			.request(TRACKER_LOCK, { mode: 'exclusive' }, () => {
				waiting = false;
				if (stopped) return Promise.resolve();
				return new Promise<void>((done) => {
					release = done;
					if (mode === 'follow') {
						mode = 'own';
						voice = null;
						hooks.own([...handed]);
					}
				});
			})
			.catch(() => {
				waiting = false;
			});
	}

	function follow(): void {
		if (mode === 'own') hooks.disown();
		mode = 'follow';
		voice = null;
		// A takeover's lock goes back: the voice that appeared is the one now.
		release?.();
		release = null;
		say({ kind: 'hello', from: id });
		awaitTakeover();
	}

	channel.onmessage = ({ data }) => {
		if (stopped || !isShareMessage(data)) return;
		switch (data.kind) {
			case 'voice':
				if (mode === 'own') {
					// A voice came up (the panel opened): one tracker is enough.
					follow();
					// What this document handed its own tracker goes to the voice.
					for (const event of handed) say({ kind: 'event', to: data.from, event });
				}
				return;
			case 'view':
				if (mode !== 'follow') return;
				if (voice === null) voice = data.from;
				if (voice === data.from) hooks.mirror(data.view);
				return;
			case 'bye':
				if (mode === 'follow' && voice === data.from) {
					// Another voice may still be up; if none is, the lock says so.
					voice = null;
					say({ kind: 'hello', from: id });
				}
				return;
			default:
				return;
		}
	};

	// A lock manager that cannot answer reads as nobody up: this document
	// runs its own tracker, as every document did before.
	const voiceUp = await locks.query().then(
		(snapshot) => (snapshot.held ?? []).some((lock) => lock.name === TRACKER_LOCK),
		() => false
	);
	if (voiceUp) {
		mode = 'follow';
		say({ kind: 'hello', from: id });
		awaitTakeover();
	} else {
		mode = 'own';
		hooks.own([]);
	}

	return {
		get mode() {
			return mode;
		},
		forward(event) {
			if (kept(event)) handed.push(event);
			if (mode !== 'follow') return false;
			// The tick, a resume, a home focus: the voice keeps its own cadence.
			if (kept(event)) say({ kind: 'event', to: voice ?? '*', event });
			return true;
		},
		stop() {
			stopped = true;
			release?.();
			channel.onmessage = null;
			channel.close();
		}
	};
}
