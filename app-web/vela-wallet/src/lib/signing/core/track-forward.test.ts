/**
 * Spec 082 RJ4 (T230): the sign machine hears its in-flight op's tracker
 * entry (`OpTracked`) whenever it changes — so a refusal, a "never sent" or a
 * landing is answered at once. And the hand-off keys every shell must use.
 */
import { describe, expect, it } from 'vitest';
import type { SignEvent } from '$lib/core/generated/SignEvent';
import type { TrackEntryView } from '$lib/core/generated/TrackEntryView';
import type { TrackView } from '$lib/core/generated/TrackView';
import { createTrackForward, handoffKey, withdrawKey } from './track-forward';

const OP = '0x' + 'a1'.repeat(32);
const OTHER = '0x' + 'b2'.repeat(32);
const TX = '0x' + 'c3'.repeat(32);

function entry(over: Partial<TrackEntryView>): TrackEntryView {
	return {
		user_op_hash: OP,
		chain_id: 100,
		record_ids: ['dapp-1-tx'],
		status: 'pending',
		tx_hash: null,
		polling: true,
		submitted_at_ms: 1,
		outcome: 'landing',
		relay_tx_hash: null,
		relay_sent_at_ms: null,
		...over
	};
}

function tracker() {
	let current: TrackView = { entries: [] };
	const listeners = new Set<(view: TrackView) => void>();
	const sent: SignEvent[] = [];
	const forward = createTrackForward({
		subscribe: (listener) => {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
		current: () => current,
		dispatch: (event) => sent.push(event),
		now: () => 42
	});
	return {
		forward,
		sent,
		push(...entries: TrackEntryView[]) {
			current = { entries };
			for (const listener of listeners) listener(current);
		},
		listeners
	};
}

describe('the answer follows the tracker (RJ4)', () => {
	it('forwards the watched op’s entry on every change, once per change', () => {
		const t = tracker();
		t.forward.watch(OP);
		t.push(entry({}));
		t.push(entry({}));
		t.push(entry({ status: 'rejected', outcome: 'final' }));
		t.push(entry({ status: 'rejected', outcome: 'final' }));
		expect(t.sent).toEqual([
			{ type: 'op_tracked', user_op_hash: OP, status: 'pending', tx_hash: null, now_ms: 42 },
			{ type: 'op_tracked', user_op_hash: OP, status: 'rejected', tx_hash: null, now_ms: 42 }
		]);
	});

	it('a landing carries its tx hash (DX-W1: the page gets it as soon as the tracker has it)', () => {
		const t = tracker();
		t.forward.watch(OP);
		t.push(entry({ status: 'confirmed', tx_hash: TX, outcome: 'final' }));
		expect(t.sent.at(-1)).toMatchObject({ status: 'confirmed', tx_hash: TX });
	});

	it('only the watched op — another op’s entry is not this request’s answer', () => {
		const t = tracker();
		t.forward.watch(OP);
		t.push(entry({ user_op_hash: OTHER, status: 'not_sent', outcome: 'final' }));
		expect(t.sent).toEqual([]);
	});

	it('an op already in the tracker is forwarded the moment it is watched', () => {
		const t = tracker();
		t.push(entry({ status: 'not_sent', outcome: 'final' }));
		t.forward.watch(OP.toUpperCase().replace('0X', '0x'));
		expect(t.sent).toEqual([
			{ type: 'op_tracked', user_op_hash: OP, status: 'not_sent', tx_hash: null, now_ms: 42 }
		]);
		// Watching it again (the admitted hand-off names the same op) repeats nothing.
		t.forward.watch(OP);
		expect(t.sent).toHaveLength(1);
	});

	it('a verdict the tracker reached while the POST was out reaches the core once it takes the op', () => {
		// The write-ahead hands the op to the tracker BEFORE its POST (RJ1),
		// and the core takes `OpTracked` only past `OpSubmitted` (contract §4).
		// The chain check can find the landed op while the relay's reply is
		// still being lost (EX-W1): forwarded then, the verdict was dropped,
		// and — the entry being terminal — never changed again, so the page
		// waited out its window for the op hash and the sheet fell back to
		// 提交至网络… (D3). A fake core that drops what it cannot take yet:
		let taken = false;
		const heard: SignEvent[] = [];
		const t = tracker();
		const forward = createTrackForward({
			subscribe: (listener) => {
				t.listeners.add(listener);
				return () => t.listeners.delete(listener);
			},
			current: () => ({ entries: [entry({ status: 'confirmed', tx_hash: TX, outcome: 'final' })] }),
			dispatch: (event) => {
				if (taken) heard.push(event);
			},
			now: () => 42
		});
		forward.watch(OP);
		t.push(entry({ status: 'confirmed', tx_hash: TX, outcome: 'final' }));
		expect(heard).toEqual([]);
		// OpSubmitted: the core takes the op now.
		taken = true;
		forward.taken(OP);
		expect(heard).toEqual([
			{ type: 'op_tracked', user_op_hash: OP, status: 'confirmed', tx_hash: TX, now_ms: 42 }
		]);
	});

	it('stop: no more forwarding, and the tracker listener is gone', () => {
		const t = tracker();
		t.forward.watch(OP);
		t.forward.stop();
		t.push(entry({ status: 'rejected' }));
		expect(t.sent).toEqual([]);
		expect(t.listeners.size).toBe(0);
	});
});

describe('the hand-off keys (contract §4)', () => {
	const handoff = {
		user_op_hash: OP,
		record_ids: ['dapp-1-tx'],
		chain_id: 100,
		maybe_sent: true,
		submit_block: 7,
		admitted: false,
		sender: '0x' + 'a1'.repeat(20)
	};

	it('the admitted hand-off is a new key, though its op and records are the write-ahead’s', () => {
		const writeAhead = handoffKey(handoff);
		const admitted = handoffKey({ ...handoff, maybe_sent: false, admitted: true });
		expect(admitted).not.toBe(writeAhead);
		// The same hand-off again is the same key: fed once.
		expect(handoffKey({ ...handoff })).toBe(writeAhead);
		// New records under the same hash (a re-submit of a never-sent op) are new.
		expect(handoffKey({ ...handoff, record_ids: ['dapp-2-tx'] })).not.toBe(writeAhead);
	});

	it('a withdrawal is keyed by its op and records', () => {
		expect(withdrawKey({ user_op_hash: OP, record_ids: ['a'] })).toBe(
			withdrawKey({ user_op_hash: OP.toUpperCase().replace('0X', '0x'), record_ids: ['a'] })
		);
		expect(withdrawKey({ user_op_hash: OP, record_ids: ['a'] })).not.toBe(
			withdrawKey({ user_op_hash: OP, record_ids: ['b'] })
		);
	});
});
