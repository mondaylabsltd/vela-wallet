/**
 * Platform-shell plumbing for a Crux core. Deliberately blind to product
 * semantics: it knows how to dispatch an event, execute the effects that come
 * back, and hand the results to the core — nothing about wallets.
 *
 * Ported unchanged in behaviour from `src/services/crux/effect-loop.ts`, the
 * driver the shipping Expo client has run since spec 011. Every core in this
 * app shares it, which is why the failure and cancellation semantics live here
 * rather than being re-derived per screen.
 *
 * The contract that matters: `execute` MUST NOT reject for an expected failure.
 * A rejected promise is converted by `toFailure` into the result variant that
 * belongs to that operation, so classification stays in the core (Rust) instead
 * of being pattern-matched from error strings in TypeScript.
 */

export type EffectWithId = { id: number };

export type CoreResult<View, Effect> = {
	view: View;
	effects: Effect[];
	cancelled_effect_ids?: number[];
};

export type EffectCore<View, Event, Effect extends EffectWithId, Result> = {
	view(): View;
	dispatch(event: Event): CoreResult<View, Effect>;
	resolve(effectId: number, result: Result): CoreResult<View, Effect>;
	dispose?(): void;
};

export type EffectLoopOptions<View, Effect extends EffectWithId, Result> = {
	/** Called on every committed view. */
	onView(view: View): void;
	/** Perform one operation. `signal` aborts when the core cancels it. */
	execute(effect: Effect, signal: AbortSignal): Promise<Result>;
	/** Turn a thrown error into the result variant this operation answers with. */
	toFailure(effect: Effect, error: unknown): Result;
	/**
	 * A core-level fault: malformed event, serialization failure. Never a user
	 * error. Optional, but never silent (spec 048): a loop with no handler
	 * reports to the console — the one fault nobody listened to, an answer the
	 * core refused to read, hid a sign-in hang behind a busy button for a day.
	 */
	onError?(error: unknown): void;
};

export type EffectLoop<Event> = {
	start(event: Event): void;
	dispatch(event: Event): void;
	dispose(): void;
};

export function createEffectLoop<View, Event, Effect extends EffectWithId, Result>(
	core: EffectCore<View, Event, Effect, Result>,
	options: EffectLoopOptions<View, Effect, Result>
): EffectLoop<Event> {
	const controllers = new Map<number, AbortController>();
	/** The effect behind each in-flight id, so a refused answer can be re-answered as its failure. */
	const report = options.onError ?? ((error: unknown) => console.error('[core] fault:', error));
	const inFlight = new Map<number, Effect>();
	/** Effects already answered with their failure — a second refusal only reports. */
	const failed = new Set<number>();
	let disposed = false;

	function start(event: Event) {
		commit(core.view());
		dispatch(event);
	}

	function dispatch(event: Event) {
		if (disposed) return;
		try {
			apply(core.dispatch(event));
		} catch (error) {
			report(error);
		}
	}

	function commit(view: View) {
		// A view produced before disposal can still arrive after it (an effect
		// that resolves while the screen unmounts). Dropping it keeps the UI
		// from being asked to render a torn-down tree.
		if (disposed) return;
		options.onView(view);
	}

	function apply(result: CoreResult<View, Effect>) {
		commit(result.view);

		for (const effectId of result.cancelled_effect_ids ?? []) {
			controllers.get(effectId)?.abort();
			controllers.delete(effectId);
		}

		for (const effect of result.effects) void run(effect);
	}

	async function run(effect: Effect) {
		const controller = new AbortController();
		controllers.set(effect.id, controller);
		inFlight.set(effect.id, effect);

		let result: Result;
		try {
			result = await options.execute(effect, controller.signal);
		} catch (error) {
			if (controller.signal.aborted) {
				controllers.delete(effect.id);
				return; // cancelled by the core; it is not waiting for an answer
			}
			result = options.toFailure(effect, error);
		} finally {
			controllers.delete(effect.id);
		}

		resolve(effect.id, result);
	}

	function resolve(effectId: number, result: Result) {
		if (disposed) return;
		const effect = inFlight.get(effectId);
		try {
			apply(core.resolve(effectId, result));
			inFlight.delete(effectId);
			failed.delete(effectId);
		} catch (error) {
			report(error);
			// The core refused the answer — a shape it cannot read, the way the
			// retired client's records were (spec 048). The machine is still
			// waiting on this effect, so it gets the effect's own failure once:
			// login's `storage_failed`, the session's `accounts_unavailable`.
			// Anything else is a spinner that never stops.
			if (effect !== undefined && !failed.has(effectId)) {
				failed.add(effectId);
				resolve(effectId, options.toFailure(effect, error));
			} else {
				inFlight.delete(effectId);
			}
		}
	}

	function dispose() {
		disposed = true;
		for (const controller of controllers.values()) controller.abort();
		controllers.clear();
		core.dispose?.();
	}

	return { start, dispatch, dispose };
}
