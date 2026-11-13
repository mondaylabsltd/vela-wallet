/**
 * How a desktop dialog comes and goes (vis-19): it rose in over a fading
 * scrim and vanished in one frame on the way out. As Svelte transitions both
 * ways are the same motion, reversed — the timing is the motion token's, and
 * a person who asked for reduced motion gets neither.
 *
 * Use with `|global`: a dialog is usually removed by its HOST's `{#if}`, and a
 * local transition only plays for the block it sits in.
 */
import { cubicOut } from 'svelte/easing';
import type { TransitionConfig } from 'svelte/transition';
import { MOTION } from '$lib/tokens/tokens';

function duration(): number {
	const still =
		typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
	return still ? 0 : MOTION.fast;
}

/** The scrim behind a dialog: a fade. (A transition takes its node; this one needs none of it.) */
// eslint-disable-next-line @typescript-eslint/no-unused-vars
export function scrimFade(_node?: Element): TransitionConfig {
	return { duration: duration(), css: (t) => `opacity: ${t}` };
}

/**
 * The dialog card: a slight scale with the fade. `centred` keeps the
 * `translate(-50%, -50%)` a card positioned by its own centre already has.
 */
export function dialogPop(_node: Element | undefined, { centred = false } = {}): TransitionConfig {
	const place = centred ? 'translate(-50%, -50%) ' : '';
	return {
		duration: duration(),
		easing: cubicOut,
		css: (t) => `opacity: ${t}; transform: ${place}scale(${0.97 + 0.03 * t})`
	};
}
