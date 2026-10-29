/**
 * The account-wide incoming-funds check's cadence (owner decision, round 5):
 * 10 s while someone is at the page, 30 s idle, 60 s hidden, at once when it
 * is shown again or comes online.
 */
import { describe, expect, it } from 'vitest';
import {
	ACTIVE_MS,
	ACTIVE_WINDOW_MS,
	HIDDEN_MS,
	IDLE_MS,
	followIncomingCadence,
	incomingCadenceMs,
	type CadenceHost,
	type CadenceReason
} from './incoming-cadence';

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
	/** Move the clock to `to`, firing every timer due on the way, in order. */
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

function follow(page: ReturnType<typeof fakePage>) {
	const checks: { at: number; reason: CadenceReason }[] = [];
	const stop = followIncomingCadence((reason) => {
		checks.push({ at: page.host.now(), reason });
	}, page.host);
	return { checks, stop };
}

describe('incomingCadenceMs', () => {
	it('is 10 s at the page, 30 s idle, 60 s hidden', () => {
		expect(incomingCadenceMs({ visible: true, sinceInteractionMs: 0 })).toBe(10_000);
		expect(incomingCadenceMs({ visible: true, sinceInteractionMs: 119_999 })).toBe(10_000);
		expect(incomingCadenceMs({ visible: true, sinceInteractionMs: 120_000 })).toBe(30_000);
		expect(incomingCadenceMs({ visible: false, sinceInteractionMs: 0 })).toBe(60_000);
		expect([ACTIVE_MS, IDLE_MS, HIDDEN_MS, ACTIVE_WINDOW_MS]).toEqual([
			10_000, 30_000, 60_000, 120_000
		]);
	});
});

describe('followIncomingCadence', () => {
	it('checks every 10 s for the two minutes after an interaction, then every 30 s', () => {
		const page = fakePage();
		const { checks } = follow(page);
		page.advance(120_000);
		// Mounting counts as being at the page: 10 s apart for two minutes.
		expect(checks.map((c) => c.at)).toEqual([
			10_000, 20_000, 30_000, 40_000, 50_000, 60_000, 70_000, 80_000, 90_000, 100_000, 110_000,
			120_000
		]);
		checks.length = 0;
		page.advance(90_000);
		expect(checks.map((c) => c.at)).toEqual([150_000, 180_000, 210_000]);
		expect(checks.every((c) => c.reason === 'interval')).toBe(true);
	});

	it('an interaction keeps it at 10 s, and one after idling brings the next check within 10 s', () => {
		const page = fakePage();
		const { checks } = follow(page);
		page.advance(181_000); // idle by now: checks 30 s apart, the last at 180 s
		checks.length = 0;
		// 1 s into an idle wait of 30 s: the next check comes 10 s from now.
		page.emit('document', 'pointerdown');
		page.advance(30_000);
		expect(checks.map((c) => c.at)).toEqual([191_000, 201_000, 211_000]);
	});

	it('hidden: every 60 s, counted from the last check; shown again: at once', () => {
		const page = fakePage();
		const { checks } = follow(page);
		page.advance(15_000); // one check at 10 s
		page.setVisible(false);
		checks.length = 0;
		page.advance(130_000);
		expect(checks.map((c) => c.at)).toEqual([70_000, 130_000]);
		checks.length = 0;
		page.setVisible(true);
		expect(checks).toEqual([{ at: 145_000, reason: 'visible' }]);
		page.advance(10_000);
		expect(checks.at(-1)).toEqual({ at: 155_000, reason: 'interval' });
	});

	it('online: at once, and the cadence starts over from there', () => {
		const page = fakePage();
		const { checks } = follow(page);
		page.advance(4_000);
		page.emit('window', 'online');
		expect(checks).toEqual([{ at: 4_000, reason: 'online' }]);
		page.advance(10_000);
		expect(checks.map((c) => c.at)).toEqual([4_000, 14_000]);
	});

	it('stopping removes the timer and every listener', () => {
		const page = fakePage();
		const { checks, stop } = follow(page);
		stop();
		expect(page.timerCount()).toBe(0);
		expect(page.listenerCount()).toBe(0);
		page.emit('window', 'online');
		page.advance(600_000);
		expect(checks).toEqual([]);
	});
});
