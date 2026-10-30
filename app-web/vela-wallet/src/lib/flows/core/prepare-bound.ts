/**
 * The wait before the passkey, bounded and said (r5reg-3).
 *
 * A send reads several things from the network before its passkey sheet can
 * open — the account's nonce, whether it is deployed, the gas, the fee. With
 * Arc's nodes answering 429 and the fallback hosts failing, those reads held
 * the confirm on a spinner for minutes with nothing said, and what ended it
 * was worded as the passkey's ("The passkey prompt timed out") although no
 * prompt had opened yet.
 *
 * This runs the submit with the reads bounded up to the moment the passkey
 * opens (`gate`): past {@link PREPARE_SLOW_MS} it reports how long it has
 * been, every second, and whether the network's nodes say they are busy; past
 * {@link PREPARE_LIMIT_MS} it gives up with a {@link PrepareTimeoutError} and
 * the gate shuts for good — a passkey never opens after the wait was given
 * up, so nothing can be signed behind the person's back. Once the gate has
 * opened, the bound is over: the ceremony has its own deadline.
 *
 * What a failure means is the core's (`SendSubmitFailure::ChainUnreachable`);
 * this only measures, and says which side of the gate a failure came from.
 */

/** How long the reads may take before the confirm says what it is waiting for. */
export const PREPARE_SLOW_MS = 5_000;
/** How long the reads may take before the submit gives up — nothing signed or sent. */
export const PREPARE_LIMIT_MS = 45_000;
/** How often the time said is brought up to date. */
const TICK_MS = 1_000;

/** The reads before the passkey did not answer within {@link PREPARE_LIMIT_MS}. */
export class PrepareTimeoutError extends Error {
	constructor(readonly elapsedMs: number) {
		super(`the network did not answer within ${elapsedMs} ms; nothing was signed`);
		this.name = 'PrepareTimeoutError';
	}
}

/** A failure before the passkey opened, with what the network said meanwhile. */
export class BeforePasskeyError extends Error {
	constructor(
		readonly cause: unknown,
		readonly busy: boolean
	) {
		super(cause instanceof Error ? cause.message : String(cause));
		this.name = 'BeforePasskeyError';
	}
}

export interface PrepareClock {
	now: () => number;
	setTimeout: (run: () => void, ms: number) => unknown;
	clearTimeout: (handle: unknown) => void;
}

const realClock: PrepareClock = {
	now: () => Date.now(),
	setTimeout: (run, ms) => setTimeout(run, ms),
	clearTimeout: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>)
};

/**
 * Run `work`, which calls `gate()` right before the passkey opens. Until it
 * does: `onSlow(elapsedMs, busy)` each second past {@link PREPARE_SLOW_MS},
 * and a {@link PrepareTimeoutError} past {@link PREPARE_LIMIT_MS}. A failure
 * of `work` before the gate is a {@link BeforePasskeyError} (unless it is one
 * of `passThrough`'s, which keeps its own meaning); after it, as thrown.
 */
export function boundedUntilPasskey<T>(
	work: (gate: () => void) => Promise<T>,
	options: {
		onSlow: (elapsedMs: number, busy: boolean) => void;
		/** Whether the network's nodes are answering "too many requests" now. */
		busy: () => boolean;
		/** A failure that is not the network's (a refusal, a venue block). */
		passThrough?: (error: unknown) => boolean;
		clock?: PrepareClock;
	}
): Promise<T> {
	const clock = options.clock ?? realClock;
	const started = clock.now();
	let opened = false;
	let givenUp = false;
	let timer: unknown = null;

	return new Promise<T>((resolve, reject) => {
		const stop = () => {
			if (timer !== null) clock.clearTimeout(timer);
			timer = null;
		};
		const tick = () => {
			timer = null;
			if (opened || givenUp) return;
			const elapsed = clock.now() - started;
			if (elapsed >= PREPARE_LIMIT_MS) {
				givenUp = true;
				reject(new BeforePasskeyError(new PrepareTimeoutError(elapsed), options.busy()));
				return;
			}
			if (elapsed >= PREPARE_SLOW_MS) options.onSlow(elapsed, options.busy());
			timer = clock.setTimeout(tick, Math.min(TICK_MS, PREPARE_LIMIT_MS - elapsed));
		};
		timer = clock.setTimeout(tick, PREPARE_SLOW_MS);

		const gate = () => {
			if (givenUp) throw new PrepareTimeoutError(clock.now() - started);
			opened = true;
			stop();
		};
		work(gate).then(
			(value) => {
				stop();
				if (!givenUp) resolve(value);
			},
			(error: unknown) => {
				stop();
				if (givenUp) return;
				if (opened || options.passThrough?.(error)) reject(error);
				else reject(new BeforePasskeyError(error, options.busy()));
			}
		);
	});
}
