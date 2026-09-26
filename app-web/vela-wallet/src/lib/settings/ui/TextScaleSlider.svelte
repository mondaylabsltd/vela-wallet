<script lang="ts">
	/**
	 * A ——●—— A: the text-size slider, pressed and dragged the way a platform
	 * slider is — the desktop's `ui::StepSlider` on the web.
	 *
	 * - The whole track is the target, `--size-hitTarget` tall. It used to be
	 *   the range input alone, which is as tall as its 4px line: a press a few
	 *   pixels above or below the dots did nothing.
	 * - Press anywhere and the thumb goes to the nearest stop; keep the button
	 *   down and it steps from stop to stop under the pointer. The pointer is
	 *   captured, so the drag holds off the track — and iOS Safari, which moves
	 *   a range only from its thumb, behaves like every other browser.
	 * - The thumb answers the pointer: over the track it grows and a soft ring
	 *   comes up, pressed both grow again, and the tick a press would land on
	 *   is marked before the press.
	 *
	 * A mouse or pen hands each stop over as it is reached, so the page is its
	 * own preview. A finger hands its stop over when it lifts, as Android and
	 * iOS do: a touch that turns into a scroll of the page is cancelled by the
	 * browser, and the size it crossed on the way must not stick.
	 *
	 * The native `input[type=range]` stays, transparent over the track and out
	 * of the pointer's way, so the keyboard, screen readers and the e2e suite's
	 * `fill()` still speak to a real slider. Ticks, thumb and pointer share one
	 * geometry: stops evenly spaced along `.rail`.
	 */
	import type { TextScaleModel } from '../model';

	interface Props {
		model: TextScaleModel;
		onchange?: (index: number) => void;
	}

	let { model, onchange }: Props = $props();

	/**
	 * The stop under the pointer while it is pressed. Its own state, not an
	 * override of `model.index`: the settings page rebuilds its model while a
	 * finger is down, and a rebuild must not put the thumb back under it.
	 */
	let held = $state<number | null>(null);
	/** The stop on screen: the held one during a press, the stored one otherwise. */
	const value = $derived(held ?? model.index);
	let hovered = $state(false);
	let pressed = $state(false);
	/** The stop a press would land on, while a mouse or pen is over the track. */
	let aimed = $state<number | null>(null);
	/** The pointer that pressed is a finger: hand the stop over on lift. */
	let touch = false;
	/** Focus came from a press, not the keyboard: no focus ring until a key
	 * is used — a native range clicked is focused and shows none either. */
	let pointerFocus = $state(false);
	let rail: HTMLDivElement | undefined = $state();
	let input: HTMLInputElement | undefined = $state();

	const last = $derived(Math.max(1, model.steps - 1));
	const ticks = $derived(Array.from({ length: model.steps }, (_, i) => i));
	const look = $derived(pressed ? 'pressed' : hovered ? 'hover' : 'rest');

	/** The stop nearest `clientX`; before the first is the first, past the last the last. */
	function stopAt(clientX: number): number {
		if (!rail) return value;
		const box = rail.getBoundingClientRect();
		const share = box.width > 0 ? (clientX - box.left) / box.width : 0;
		return Math.round(Math.min(1, Math.max(0, share)) * last);
	}

	function reach(stop: number) {
		if (stop === held) return;
		held = stop;
		if (!touch && stop !== model.index) onchange?.(stop);
	}

	function down(event: PointerEvent) {
		if (event.button !== 0) return;
		// No text selection, no native drag of the input underneath.
		event.preventDefault();
		(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
		// Arrow keys carry on from here, as they do after clicking a native range.
		pointerFocus = true;
		input?.focus({ preventScroll: true });
		touch = event.pointerType === 'touch';
		pressed = true;
		aimed = null;
		reach(stopAt(event.clientX));
	}

	function move(event: PointerEvent) {
		if (pressed) {
			reach(stopAt(event.clientX));
		} else if (event.pointerType !== 'touch') {
			hovered = true;
			aimed = stopAt(event.clientX);
		}
	}

	function up() {
		if (!pressed) return;
		pressed = false;
		if (touch && held !== null && held !== model.index) onchange?.(held);
		held = null;
		touch = false;
	}

	/** The browser took the touch for a scroll: the stops it crossed never happened. */
	function cancel() {
		if (!pressed) return;
		pressed = false;
		held = null;
		touch = false;
	}

	function leave() {
		hovered = false;
		aimed = null;
	}
</script>

<div class="scale">
	<span class="glyph small" aria-hidden="true">A</span>
	<!-- `role="group"`: the pointer handling is for the pointer only — the
	     range input inside is what the keyboard and assistive tech move — but
	     a listener still owes the machine a role, and the box holds one control. -->
	<div
		class="track"
		role="group"
		data-look={look}
		style:--place={value / last}
		onpointerdown={down}
		onpointermove={move}
		onpointerup={up}
		onpointercancel={cancel}
		onlostpointercapture={up}
		onpointerleave={leave}
	>
		<input
			bind:this={input}
			class="native"
			type="range"
			min="0"
			max={model.steps - 1}
			step="1"
			{value}
			aria-label={model.label}
			class:pointer-focus={pointerFocus}
			onkeydown={() => (pointerFocus = false)}
			onblur={() => (pointerFocus = false)}
			oninput={(event) => onchange?.(Number(event.currentTarget.value))}
		/>
		<div class="rail" bind:this={rail} aria-hidden="true">
			{#each ticks as tick (tick)}
				<span class="tick" class:aimed={aimed === tick && tick !== value} style:--at={tick / last}
				></span>
			{/each}
			<span class="ring"></span>
			<span class="thumb"></span>
		</div>
	</div>
	<span class="glyph large" aria-hidden="true">A</span>
</div>

<style>
	.scale {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		padding-block: var(--space-lg);
	}

	.glyph {
		color: var(--color-fg-base);
		font-weight: var(--weight-bold);
		flex-shrink: 0;
	}

	.small {
		font-size: var(--text-base);
	}

	.large {
		font-size: var(--text-2xl);
	}

	.track {
		position: relative;
		flex: 1;
		min-width: 0;
		height: var(--size-hitTarget);
		cursor: pointer;
		/* A vertical swipe that starts here still scrolls the page. */
		touch-action: pan-y;
		-webkit-tap-highlight-color: transparent;
		user-select: none;
	}

	.native {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		margin: 0;
		opacity: 0;
		pointer-events: none;
	}

	/* The end stops sit this far in, so the thumb and its ring are whole there. */
	.rail {
		position: absolute;
		inset-block: 0;
		inset-inline: var(--space-lg);
	}

	.tick,
	.ring,
	.thumb {
		position: absolute;
		top: 50%;
		translate: -50% -50%;
		border-radius: var(--radius-full);
	}

	.tick {
		left: calc(var(--at) * 100%);
		width: var(--space-sm);
		height: var(--space-sm);
		background: var(--color-border-strong);
		transition:
			scale var(--motion-duration-fast) ease-out,
			background-color var(--motion-duration-fast) ease-out;
	}

	.tick.aimed {
		scale: 2;
		background: var(--color-fg-subtle);
	}

	/* The ring is drawn at its pressed size and scaled down: at rest it is the
	   thumb's own size and invisible, over the track 0.9, pressed 1. */
	.ring {
		left: calc(var(--place) * 100%);
		width: var(--size-hitTarget);
		height: var(--size-hitTarget);
		background: var(--color-fg-muted);
		opacity: 0;
		scale: 0.45;
	}

	.thumb {
		left: calc(var(--place) * 100%);
		width: var(--icon-lg);
		height: var(--icon-lg);
		background: var(--color-fg-muted);
		box-shadow: var(--shadow-md);
	}

	.ring,
	.thumb {
		transition:
			left var(--motion-duration-fast) cubic-bezier(0.2, 0, 0, 1),
			scale var(--motion-duration-fast) cubic-bezier(0.2, 0, 0, 1),
			opacity var(--motion-duration-fast) ease-out;
	}

	.track[data-look='hover'] .thumb {
		scale: 1.2;
	}

	.track[data-look='hover'] .ring {
		opacity: 0.1;
		scale: 0.9;
	}

	.track[data-look='pressed'] .thumb {
		scale: 1.3;
	}

	.track[data-look='pressed'] .ring {
		opacity: 0.16;
		scale: 1;
	}

	/* The input is transparent, so keyboard focus is drawn on the thumb, in
	   the app's own focus ring. */
	.native:focus-visible:not(.pointer-focus) ~ .rail .thumb {
		box-shadow:
			0 0 0 var(--space-xs) var(--color-fixed-focusRingInner),
			0 0 0 var(--space-sm) var(--color-fixed-focusRingOuter);
	}

	@media (prefers-reduced-motion: reduce) {
		.tick,
		.ring,
		.thumb {
			transition: none;
		}
	}
</style>
