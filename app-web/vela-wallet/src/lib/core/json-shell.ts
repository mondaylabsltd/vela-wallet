/**
 * Adapter between the JSON/wasm core surface and the product-agnostic effect
 * loop.
 *
 * The wasm classes speak strings: `dispatch(json) -> json`. This turns that
 * into the typed `EffectCore` the loop expects, and nothing else — no product
 * knowledge, no error interpretation.
 */

import {
	createEffectLoop,
	type CoreResult,
	type EffectCore,
	type EffectLoop,
	type EffectLoopOptions,
	type EffectWithId
} from './effect-loop';

/** The shape every generated Crux core class exposes. */
export type JsonWasmCore = {
	dispatch(event: string): string;
	resolve_effect(effectId: bigint, result: string): string;
	view(): string;
	free(): void;
	/** Present on the machines the bridge lets opt in (`vela_core::app::view_host`). */
	skip_unchanged_views?(includeVolatile: boolean): void;
};

/** How this shell takes the core's answers. */
export type JsonShellOptions = {
	/**
	 * Opt in to answers without an unchanged view: the core then sends `view`
	 * only when it differs from the last one it sent, and the loop hands
	 * `onView` nothing for an answer that changed nothing.
	 * `includeVolatile`: the shell reads the machine's volatile fields (the
	 * RPC pool's `pending_urls`); they come apart and are put back into the
	 * view here. Without it they are not sent at all.
	 */
	skipUnchangedViews?: { includeVolatile: boolean };
};

/** An answer as the opted-in core writes it. */
type LeanAnswer<View, Effect> = {
	view?: View;
	view_version?: number;
	volatile?: Partial<View>;
	effects: Effect[];
	cancelled_effect_ids?: number[];
};

/**
 * The wire's integers: a `bigint` (an `i64` the generated types spell so,
 * e.g. a wallet's EIP-1193 code) goes out as the JSON number it is.
 */
function wire(value: unknown): string {
	return JSON.stringify(value, (_key, v) => (typeof v === 'bigint' ? Number(v) : v));
}

export function createJsonWasmShell<View, Event, Effect extends EffectWithId, Result>(
	core: JsonWasmCore,
	options: EffectLoopOptions<View, Effect, Result>,
	shell: JsonShellOptions = {}
): EffectLoop<Event> {
	const lean =
		shell.skipUnchangedViews && typeof core.skip_unchanged_views === 'function'
			? shell.skipUnchangedViews
			: null;
	if (lean) core.skip_unchanged_views?.(lean.includeVolatile);
	// What the core last sent, held so an answer without a view keeps it.
	let stable: View | undefined;
	let volatile: Partial<View> = {};
	const take = (text: string): CoreResult<View, Effect> => {
		if (!lean) return JSON.parse(text) as CoreResult<View, Effect>;
		const answer = JSON.parse(text) as LeanAnswer<View, Effect>;
		let moved = false;
		if (answer.view !== undefined) {
			stable = answer.view;
			moved = true;
		}
		if (answer.volatile !== undefined) {
			volatile = answer.volatile;
			moved = true;
		}
		return {
			view: moved && stable !== undefined ? ({ ...stable, ...volatile } as View) : undefined,
			effects: answer.effects,
			cancelled_effect_ids: answer.cancelled_effect_ids
		};
	};
	const jsonCore: EffectCore<View, Event, Effect, Result> = {
		view: () => JSON.parse(core.view()) as View,
		dispatch: (event) => take(core.dispatch(wire(event))),
		resolve: (effectId, result) => take(core.resolve_effect(BigInt(effectId), wire(result))),
		dispose: () => core.free()
	};

	return createEffectLoop(jsonCore, options);
}
