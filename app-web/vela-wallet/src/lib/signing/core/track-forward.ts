/**
 * The dApp's answer follows what the tracker knows (spec 082 RJ4, G37–G39).
 *
 * The tracker (`tx_tracker`) follows every op the signing sheet hands it, and
 * it learns the op's fate first: the relay refused it, it was never sent, or
 * the chain has it. The page used to hear none of that until the sheet's own
 * receipt wait ran out — an op the relay had refused was answered "ok + op
 * hash" 100 s later (DX-W3), and a landed op's tx hash reached the page 51 s
 * after the chain check had it (EX-W1).
 *
 * So the sign machine is handed every change of its in-flight op's tracker
 * entry as `Event::OpTracked`, and the CORE decides what that means for the
 * page (`sign_request::on_op_tracked`: a terminal verdict answers at once,
 * anything else waits). Nothing is judged here: this forwards the entry of the
 * op it is told to watch, once per change of its status or tx hash, and the
 * core drops what does not concern its in-flight op.
 *
 * Also here, beside the rule they carry out: the de-duplication keys of the
 * tracker hand-off and withdrawal (contract §4, "every shell's hand-off
 * de-duplication must include `admitted`").
 */
import type { SignEvent } from '$lib/core/generated/SignEvent';
import type { SignTrackerHandoff } from '$lib/core/generated/SignTrackerHandoff';
import type { SignTrackerWithdraw } from '$lib/core/generated/SignTrackerWithdraw';
import type { TrackView } from '$lib/core/generated/TrackView';

/**
 * One hand-off, as fed once: the op, the records it closes, and what it says
 * (`maybe_sent`, `admitted`). The admitted hand-off names the SAME hash and
 * records as the write-ahead one (spec 082 RJ1) — keyed on those alone it was
 * never fed, the tracker never learned the relay took the op, and an accepted
 * op could end "not sent" with its records failed.
 */
export function handoffKey(handoff: SignTrackerHandoff): string {
	return [
		handoff.user_op_hash.toLowerCase(),
		handoff.record_ids.join(','),
		handoff.maybe_sent,
		handoff.admitted
	].join('|');
}

/** One withdrawal, as fed once (spec 082 RJ1). */
export function withdrawKey(withdraw: SignTrackerWithdraw): string {
	return `${withdraw.user_op_hash.toLowerCase()}|${withdraw.record_ids.join(',')}`;
}

export interface TrackForwardDeps {
	/** Every committed tracker view; returns the unsubscribe. */
	subscribe(listener: (view: TrackView) => void): () => void;
	/** The tracker's latest view, read when a new op is watched. */
	current(): TrackView;
	/** Into the sign machine. */
	dispatch(event: SignEvent): void;
	/** Test seam; `Date.now` by default. */
	now?: () => number;
}

export interface TrackForward {
	/** Forward this op's entry from now on (`null`: none). The last one watched is dropped. */
	watch(opHash: string | null): void;
	/** Stop listening to the tracker. */
	stop(): void;
}

export function createTrackForward(deps: TrackForwardDeps): TrackForward {
	const now = deps.now ?? Date.now;
	let watched: string | null = null;
	let lastKey = '';

	function forward(view: TrackView): void {
		if (watched === null) return;
		const entry = view.entries.find((row) => row.user_op_hash.toLowerCase() === watched);
		if (!entry) return;
		const key = `${entry.status}|${entry.tx_hash ?? ''}`;
		if (key === lastKey) return;
		lastKey = key;
		deps.dispatch({
			type: 'op_tracked',
			user_op_hash: entry.user_op_hash,
			status: entry.status,
			tx_hash: entry.tx_hash ?? null,
			now_ms: now()
		});
	}

	const unsubscribe = deps.subscribe(forward);

	return {
		watch(opHash) {
			const next = opHash ? opHash.toLowerCase() : null;
			if (next === watched) return;
			watched = next;
			lastKey = '';
			forward(deps.current());
		},
		stop() {
			watched = null;
			unsubscribe();
		}
	};
}
