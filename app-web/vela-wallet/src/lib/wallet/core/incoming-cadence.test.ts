/**
 * The account-wide incoming-funds check's cadence (owner decision, round 5;
 * r9-17: decided by the core, `incoming_scan::next_scan_ms`): 10 s while
 * someone is at the page, 30 s idle, 60 s hidden, at once when it is shown
 * again or comes online. The page here is a fake; the cadence is the REAL
 * `activity_feed` core, through the port `feed.svelte.ts` gives it — so this
 * is the web's behaviour, end to end, with no rule of its own.
 */
import { afterEach, beforeAll, describe, expect, it } from 'vitest';
import type { FeedNextScan } from '$lib/core/generated/FeedNextScan';
import type { FeedView } from '$lib/core/generated/FeedView';
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
	return { scans, stop, followers };
}

describe('followIncomingCadence (the core decides)', () => {
	it('scans every 10 s for the two minutes after an interaction, then every 30 s', () => {
		const page = fakePage();
		const { scans } = follow(page);
		page.advance(120_000);
		// Mounting counts as being at the page: 10 s apart for two minutes.
		expect(scans).toEqual([
			10_000, 20_000, 30_000, 40_000, 50_000, 60_000, 70_000, 80_000, 90_000, 100_000, 110_000,
			120_000
		]);
		scans.length = 0;
		page.advance(90_000);
		expect(scans).toEqual([150_000, 180_000, 210_000]);
	});

	it('an interaction after idling brings the next scan within 10 s', () => {
		const page = fakePage();
		const { scans } = follow(page);
		page.advance(181_000); // idle by now: scans 30 s apart, the last at 180 s
		scans.length = 0;
		page.emit('document', 'pointerdown');
		page.advance(30_000);
		expect(scans).toEqual([191_000, 201_000, 211_000]);
	});

	it('hidden: every 60 s, counted from the last scan; shown again: at once', () => {
		const page = fakePage();
		const { scans } = follow(page);
		page.advance(15_000); // one scan at 10 s
		page.setVisible(false);
		scans.length = 0;
		page.advance(130_000);
		expect(scans).toEqual([70_000, 130_000]);
		scans.length = 0;
		page.setVisible(true);
		expect(scans).toEqual([145_000]);
		page.advance(10_000);
		expect(scans).toEqual([145_000, 155_000]);
	});

	it('online: at once, and the cadence starts over from there', () => {
		const page = fakePage();
		const { scans } = follow(page);
		page.advance(4_000);
		page.emit('window', 'online');
		expect(scans).toEqual([4_000]);
		page.advance(10_000);
		expect(scans).toEqual([4_000, 14_000]);
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
