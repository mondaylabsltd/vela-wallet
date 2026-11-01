/**
 * The account-wide incoming-funds check's cadence (r9-17: decided by the
 * core, `incoming_scan::next_scan_ms`; r10 follow-up): a back-off while
 * nothing arrives — 15 s, 15 s, 30 s, 60 s, then every 2 min; 2 min hidden;
 * at once when the page is shown again or comes online. The page here is a
 * fake; the cadence is the REAL `activity_feed` core, through the port
 * `feed.svelte.ts` gives it — so this is the web's behaviour, end to end,
 * with no rule of its own.
 */
import { afterEach, beforeAll, describe, expect, it } from 'vitest';
import type { FeedNextScan } from '$lib/core/generated/FeedNextScan';
import type { FeedView } from '$lib/core/generated/FeedView';
import { GESTURE_SETTLE_MS, GestureTracker } from '../list-gesture';
import { followIncomingCadence, type CadenceHost, type CadencePort } from './incoming-cadence';

type Core = {
	dispatch(event: string): string;
	resolve_effect(id: bigint, result: string): string;
	free(): void;
};
type Answer = {
	view?: FeedView;
	effects: { id: number; operation: { type: string; read_id?: number } }[];
};

let ActivityFeedCore: new () => Core;
beforeAll(async () => {
	await import('$lib/i18n/wasm-init.server');
	({ ActivityFeedCore } = (await import('$lib/core/client')) as unknown as {
		ActivityFeedCore: new () => Core;
	});
}, 30_000);

/** A page with a hand-driven clock and events. */
function fakePage() {
	let now = 0;
	let visible = true;
	let seq = 0;
	const timers = new Map<number, { at: number; run: () => void }>();
	const listeners = new Map<string, Set<() => void>>();
	const host: CadenceHost = {
		now: () => now,
		visible: () => visible,
		setTimeout: (run, ms) => {
			seq += 1;
			timers.set(seq, { at: now + ms, run });
			return seq;
		},
		clearTimeout: (handle) => void timers.delete(handle as number),
		on: (target, type, run) => {
			const key = `${target}:${type}`;
			if (!listeners.has(key)) listeners.set(key, new Set());
			listeners.get(key)!.add(run);
			return () => listeners.get(key)!.delete(run);
		}
	};
	const emit = (target: 'document' | 'window', type: string) => {
		for (const run of [...(listeners.get(`${target}:${type}`) ?? [])]) run();
	};
	/** Move the clock by `by`, firing every timer due on the way, in order. */
	const advance = (by: number) => {
		const to = now + by;
		for (;;) {
			const due = [...timers.entries()]
				.filter(([, t]) => t.at <= to)
				.sort((a, b) => a[1].at - b[1].at)[0];
			if (!due) break;
			timers.delete(due[0]);
			now = due[1].at;
			due[1].run();
		}
		now = to;
	};
	return {
		host,
		advance,
		emit,
		setVisible: (v: boolean) => {
			visible = v;
			emit('document', 'visibilitychange');
		},
		listenerCount: () => [...listeners.values()].reduce((n, s) => n + s.size, 0),
		timerCount: () => timers.size
	};
}

const cores: Core[] = [];
afterEach(() => {
	for (const core of cores.splice(0)) core.free();
});

/**
 * The core behind the port: every scan it asks for is recorded at the page's
 * time and answered at once ("found nothing"), every read with an empty store.
 */
function follow(page: ReturnType<typeof fakePage>) {
	const core = new ActivityFeedCore();
	cores.push(core);
	const scans: number[] = [];
	const followers = new Set<(next: FeedNextScan | null) => void>();
	let next: FeedNextScan | null = null;
	const take = (text: string) => {
		const answer = JSON.parse(text) as Answer;
		if (answer.view) {
			next = answer.view.next_scan ?? null;
			for (const f of followers) f(next);
		}
		for (const effect of answer.effects) {
			if (effect.operation.type === 'scan_incoming_transfers') {
				scans.push(page.host.now());
				take(
					core.resolve_effect(
						BigInt(effect.id),
						JSON.stringify({ type: 'sync_completed', new_count: 0 })
					)
				);
			} else if (effect.operation.type === 'read_tx_store') {
				take(
					core.resolve_effect(
						BigInt(effect.id),
						JSON.stringify({
							type: 'store_loaded',
							records: [],
							now_ms: page.host.now(),
							read_id: effect.operation.read_id
						})
					)
				);
			}
		}
	};
	const send = (event: unknown) => take(core.dispatch(JSON.stringify(event)));
	// The account hand-off's own check, then the page follows.
	send({ type: 'account_switched', address: '0xa11ce' });
	scans.length = 0;
	const port: CadencePort = {
		report: (change, presence) => send({ type: 'presence_changed', presence, change }),
		due: (seq, presence) => send({ type: 'scan_due', seq, presence }),
		follow: (listener) => {
			followers.add(listener);
			listener(next);
			return () => void followers.delete(listener);
		}
	};
	const stop = followIncomingCadence(port, page.host);
	/** The page as `feed.svelte.ts` reports it for the person's own moments. */
	const presence = () => ({
		visible: page.host.visible(),
		since_interaction_ms: 0,
		now_ms: page.host.now()
	});
	return { scans, stop, followers, send, presence };
}

describe('followIncomingCadence (the core decides)', () => {
	it('backs off while nothing arrives: 15 s, 15 s, 30 s, 60 s, then every 2 min', () => {
		const page = fakePage();
		const { scans } = follow(page);
		page.advance(360_000);
		expect(scans).toEqual([15_000, 30_000, 60_000, 120_000, 240_000, 360_000]);
	});

	it('a touch no longer brings a scan closer (r10: scanning while someone scrolls is what jank is)', () => {
		const page = fakePage();
		const { scans } = follow(page);
		page.advance(181_000); // scans at 15, 30, 60 and 120 s; the next is due at 240 s
		scans.length = 0;
		page.emit('document', 'pointerdown');
		page.advance(30_000);
		expect(scans).toEqual([]);
		page.advance(30_000);
		expect(scans).toEqual([240_000]);
	});

	it('hidden: 2 min counted from the last scan; shown again: at once, and the back-off starts over', () => {
		const page = fakePage();
		const { scans } = follow(page);
		page.advance(20_000); // one scan at 15 s
		page.setVisible(false);
		scans.length = 0;
		page.advance(130_000);
		expect(scans).toEqual([135_000]);
		scans.length = 0;
		page.setVisible(true);
		expect(scans).toEqual([150_000]);
		page.advance(15_000);
		expect(scans).toEqual([150_000, 165_000]);
	});

	it('online: at once, and the back-off starts over from there', () => {
		const page = fakePage();
		const { scans } = follow(page);
		page.advance(4_000);
		page.emit('window', 'online');
		expect(scans).toEqual([4_000]);
		page.advance(15_000);
		expect(scans).toEqual([4_000, 19_000]);
	});

	it('a burst of touches is reported once a second', () => {
		const page = fakePage();
		const reports: string[] = [];
		const stop = followIncomingCadence(
			{ report: (change) => void reports.push(change), due: () => {}, follow: () => () => {} },
			page.host
		);
		for (let i = 0; i < 20; i++) {
			page.emit('document', 'wheel');
			page.advance(100);
		}
		expect(reports).toEqual(['started', 'interacted', 'interacted']);
		stop();
	});

	it('a list scrolling holds a due scan; it runs once the list settles (r10 follow-up)', () => {
		const page = fakePage();
		const { scans, send, presence } = follow(page);
		// `feed.interacting`, from the list's gesture (`list-gesture.ts`).
		const list = new GestureTracker(
			(on) => send({ type: 'interacting', on, presence: presence() }),
			page.host
		);
		page.advance(10_000); // the first scan is due at 15 s
		// A long scroll: a scroll event every 100 ms from 10 s to 20 s.
		for (let at = 10_000; at < 20_000; at += 100) {
			list.input('moved');
			page.advance(100);
		}
		expect(scans).toEqual([]);
		// The list settles: `GESTURE_SETTLE_MS` after its last move, at 19.9 s.
		page.advance(GESTURE_SETTLE_MS);
		expect(scans).toEqual([19_900 + GESTURE_SETTLE_MS]);
	});

	it('a finger on the list keeps the hold; scrollend after a fling releases it', () => {
		const page = fakePage();
		const { scans, send, presence } = follow(page);
		const list = new GestureTracker(
			(on) => send({ type: 'interacting', on, presence: presence() }),
			page.host
		);
		page.advance(14_000);
		list.input('finger-down');
		list.input('moved');
		page.advance(3_000); // the finger rests past the due scan
		expect(scans).toEqual([]);
		list.input('finger-up');
		list.input('moved'); // the fling
		page.advance(100);
		expect(scans).toEqual([]);
		list.input('settled'); // scrollend
		expect(scans).toEqual([17_100]);
	});

	it('refreshed and sent scan at once and start the back-off over', () => {
		const page = fakePage();
		const { scans, send, presence } = follow(page);
		page.advance(100_000); // scans at 15, 30 and 60 s; the next waits 60 s
		scans.length = 0;
		send({ type: 'presence_changed', presence: presence(), change: 'refreshed' });
		expect(scans).toEqual([100_000]);
		page.advance(15_000);
		expect(scans).toEqual([100_000, 115_000]);
		send({ type: 'presence_changed', presence: presence(), change: 'sent' });
		expect(scans).toEqual([100_000, 115_000, 115_000]);
		page.advance(15_000);
		expect(scans).toEqual([100_000, 115_000, 115_000, 130_000]);
	});

	it('stopping removes the timer, every listener and the follow', () => {
		const page = fakePage();
		const { scans, stop, followers } = follow(page);
		stop();
		expect(page.timerCount()).toBe(0);
		expect(page.listenerCount()).toBe(0);
		expect(followers.size).toBe(0);
		page.emit('window', 'online');
		page.advance(600_000);
		expect(scans).toEqual([]);
	});
});
