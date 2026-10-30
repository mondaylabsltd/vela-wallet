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
	options: EffectLoopOptions<View, Effect, Result>
): EffectLoop<Event> {
	const jsonCore: EffectCore<View, Event, Effect, Result> = {
		view: () => JSON.parse(core.view()) as View,
		dispatch: (event) => JSON.parse(core.dispatch(wire(event))) as CoreResult<View, Effect>,
		resolve: (effectId, result) =>
			JSON.parse(core.resolve_effect(BigInt(effectId), wire(result))) as CoreResult<View, Effect>,
		dispose: () => core.free()
	};

	return createEffectLoop(jsonCore, options);
}
