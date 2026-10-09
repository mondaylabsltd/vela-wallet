/**
 * PR 2 note 12: one transaction tracker for the whole extension.
 *
 * The request window's signing resident booted a tracker of its own, which
 * swept every stored pending record and polled each beside the side panel's
 * tracker — every relay read twice, every record patched by two documents.
 * Now the wallet's documents are voices and the request window follows one;
 * it runs its own only while none is up, steps down when one appears, and
 * takes over (replaying its own hand-offs) when the last one goes.
 *
 * A channel hub and a lock manager stand in for `BroadcastChannel` and
 * `navigator.locks` — with the semantics this relies on: a shared hold blocks
 * an exclusive request until every shared holder releases; a document that
 * goes releases what it held.
 */
import { describe, expect, it } from 'vitest';
import type { TrackEvent } from '$lib/core/generated/TrackEvent';
import type { TrackView } from '$lib/core/generated/TrackView';
import {
	startFollower,
	startVoice,
	TRACKER_LOCK,
	type ShareChannel,
	type ShareDeps,
	type ShareLocks,
	type ShareMessage
} from './tracker-share';

/** Every document's channel, delivering to all the others (never itself). */
class Hub {
	readonly channels = new Set<ShareChannel>();
	readonly said: ShareMessage[] = [];
	open(): ShareChannel {
		const { channels, said } = this;
		const channel: ShareChannel = {
			onmessage: null,
			postMessage(message) {
				said.push(message);
				const copy = structuredClone(message);
				for (const other of channels) {
					if (other !== channel) queueMicrotask(() => other.onmessage?.({ data: copy }));
				}
			},
			close() {
				channels.delete(channel);
			}
		};
		this.channels.add(channel);
		return channel;
	}
}

/** A lock manager for one name: shared holders, then a FIFO of waiters. */
class Locks {
	shared = 0;
	exclusive = false;
	waiting: { mode: 'shared' | 'exclusive'; grant: () => void }[] = [];
	manager(): ShareLocks {
		return {
			request: (_name, { mode }, callback) =>
				new Promise((resolve) => {
					const grant = () => {
						if (mode === 'shared') this.shared += 1;
						else this.exclusive = true;
						void callback().then((value) => {
							if (mode === 'shared') this.shared -= 1;
							else this.exclusive = false;
							resolve(value);
							this.pump();
						});
					};
					this.waiting.push({ mode, grant });
					this.pump();
				}),
			query: async () => ({
				held: [
					...Array.from({ length: this.shared }, () => ({ name: TRACKER_LOCK, mode: 'shared' })),
					...(this.exclusive ? [{ name: TRACKER_LOCK, mode: 'exclusive' }] : [])
				]
			})
		};
	}
	pump(): void {
		while (this.waiting.length > 0) {
			const next = this.waiting[0];
			const free = next.mode === 'shared' ? !this.exclusive : !this.exclusive && this.shared === 0;
			if (!free) return;
			this.waiting.shift();
			next.grant();
		}
	}
}

async function settle(): Promise<void> {
	for (let i = 0; i < 20; i += 1) await Promise.resolve();
}

function world() {
	const hub = new Hub();
	const locks = new Locks();
	let n = 0;
	const deps = (): ShareDeps & { leave: () => void } => {
		let leave: () => void = () => {};
		return {
			channel: () => hub.open(),
			locks: () => locks.manager(),
			id: () => `doc-${(n += 1)}`,
			onLeave: (fire) => {
				leave = fire;
				return () => {};
			},
			leave: () => leave()
		};
	};
	return { hub, locks, deps };
}

const OP = '0x' + 'ab'.repeat(32);
const view = (status: string): TrackView =>
	({ entries: [{ user_op_hash: OP, status }] }) as unknown as TrackView;
const submitted: TrackEvent = {
	type: 'submitted',
	user_op_hash: OP,
	record_ids: ['dapp-1-tx'],
	chain_id: 100,
	maybe_sent: false,
	submit_block: null,
	admitted: true,
	sender: '0x' + '11'.repeat(20)
};

/** A wallet document running the tracker. */
function wallet(deps: ShareDeps, first: TrackView) {
	let current = first;
	const taken: TrackEvent[] = [];
	const voice = startVoice(deps, { current: () => current, dispatch: (e) => taken.push(e) })!;
	return {
		taken,
		show(next: TrackView) {
			current = next;
			voice.view(next);
		},
		voice
	};
}

/** The request window, recording what its own tracker would do. */
async function requestWindow(deps: ShareDeps) {
	const log: string[] = [];
	const mirrored: TrackView[] = [];
	const follower = await startFollower(deps, {
		own: (replay) => log.push(`own:${replay.map((e) => e.type).join(',')}`),
		disown: () => log.push('disown'),
		mirror: (v) => mirrored.push(v)
	});
	return { follower, log, mirrored };
}

describe('the request window and the wallet share one tracker (PR 2 note 12)', () => {
	it('with the wallet up, the window runs none of its own: it mirrors and hands over', async () => {
		const w = world();
		const panel = wallet(w.deps(), view('pending'));
		await settle();
		const win = await requestWindow(w.deps());
		await settle();
		expect(win.follower.mode).toBe('follow');
		// No sweep, no poll here: no tracker of its own was ever started.
		expect(win.log).toEqual([]);
		// The wallet's view is this window's (its hello answered).
		expect(win.mirrored.at(-1)).toEqual(view('pending'));
		panel.show(view('confirmed'));
		await settle();
		expect(win.mirrored.at(-1)).toEqual(view('confirmed'));
		// This window's operation goes to the wallet's tracker.
		expect(win.follower.forward(submitted)).toBe(true);
		await settle();
		expect(panel.taken).toEqual([submitted]);
		// The clock and resumes are the voice's own: nothing is handed over.
		expect(win.follower.forward({ type: 'tick' })).toBe(true);
		await settle();
		expect(panel.taken).toEqual([submitted]);
	});

	it('alone, the window runs its own tracker, as before', async () => {
		const w = world();
		const win = await requestWindow(w.deps());
		expect(win.follower.mode).toBe('own');
		expect(win.log).toEqual(['own:']);
		// Its own: the caller dispatches into it.
		expect(win.follower.forward(submitted)).toBe(false);
	});

	it('steps down when the wallet comes up, handing it what it was tracking', async () => {
		const w = world();
		const win = await requestWindow(w.deps());
		win.follower.forward(submitted);
		const panel = wallet(w.deps(), view('pending'));
		await settle();
		expect(win.follower.mode).toBe('follow');
		expect(win.log).toEqual(['own:', 'disown']);
		expect(panel.taken).toEqual([submitted]);
		expect(win.mirrored.at(-1)).toEqual(view('pending'));
	});

	it('takes over when the last wallet document goes, replaying its own hand-offs', async () => {
		const w = world();
		const panelDeps = w.deps();
		const panel = wallet(panelDeps, view('pending'));
		await settle();
		const win = await requestWindow(w.deps());
		await settle();
		win.follower.forward(submitted);
		await settle();
		expect(win.log).toEqual([]);
		// The panel closes.
		panelDeps.leave();
		await settle();
		expect(win.follower.mode).toBe('own');
		expect(win.log).toEqual(['own:submitted']);
		expect(panel.taken).toEqual([submitted]);
		// From now on it is this window's own tracker that is told.
		expect(win.follower.forward({ type: 'tick' })).toBe(false);
	});

	it('with two wallet documents up, it follows one of them, and the other after it goes', async () => {
		const w = world();
		const a = wallet(w.deps(), view('pending'));
		const b = wallet(w.deps(), view('pending'));
		await settle();
		const win = await requestWindow(w.deps());
		await settle();
		const followed = win.mirrored.length;
		expect(followed).toBeGreaterThan(0);
		// Only the followed voice's views are this window's.
		a.show(view('a'));
		b.show(view('b'));
		await settle();
		const seen = win.mirrored
			.slice(followed)
			.map((v) => (v.entries[0] as { status: string }).status);
		expect(seen.length).toBe(1);
		// The followed voice goes: the window hears the one still up, and runs none.
		const gone = seen[0] === 'a' ? a : b;
		const left = gone === a ? b : a;
		gone.voice.stop();
		await settle();
		left.show(view('after'));
		await settle();
		expect(win.mirrored.at(-1)).toEqual(view('after'));
		expect(win.follower.mode).toBe('follow');
		expect(win.log).toEqual([]);
	});

	it('a browser with no channel or no locks shares nothing: the window runs its own', async () => {
		const win = await requestWindow({ channel: () => null, locks: () => null });
		expect(win.follower.mode).toBe('own');
		expect(win.log).toEqual(['own:']);
		expect(
			startVoice(
				{ channel: () => null, locks: () => null },
				{
					current: () => view('x'),
					dispatch: () => {}
				}
			)
		).toBeNull();
	});
});
