<script lang="ts">
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';

	/**
	 * The one way to confirm a signature (spec 022 §4, product contract).
	 *
	 * There is no reject button beside it: closing the sheet IS the rejection,
	 * so the only deliberate act on this screen is the affirmative one. The
	 * gesture asks for 88% of the track, which is far more than a mis-tap and
	 * far less than a fight.
	 *
	 * Keyboard and assistive tech get the same power without the gesture —
	 * Enter or Space on the focused track confirms — because a confirmation
	 * only a thumb can perform is a confirmation some people can never give.
	 */
	interface Props {
		hint: string;
		action: string;
		enabled: boolean;
		onconfirm?: () => void;
	}

	let { hint, action, enabled, onconfirm }: Props = $props();

	/** Fraction of the track the knob must cross to commit. */
	const COMMIT = 0.88;

	let track: HTMLDivElement | undefined = $state();
	/** Bound, so the knob's travel survives a resize without a listener. */
	let trackWidth = $state(0);
	let progress = $state(0);
	let dragging = $state(false);
	let done = $state(false);

	const label = $derived(`${hint} · ${action}`);

	/** Knob 48 inside 4 of end padding either side, so the travel is W − 56. */
	const travel = $derived(Math.max(1, trackWidth - 56));

	function start(event: PointerEvent) {
		if (!enabled || done) return;
		dragging = true;
		(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
	}

	function move(event: PointerEvent) {
		if (!dragging || !track) return;
		const x = event.clientX - track.getBoundingClientRect().left - 28;
		progress = Math.min(1, Math.max(0, x / travel));
	}

	function end() {
		if (!dragging) return;
		dragging = false;
		if (progress >= COMMIT) {
			progress = 1;
			done = true;
			onconfirm?.();
		} else {
			progress = 0;
		}
	}

	function key(event: KeyboardEvent) {
		if (!enabled || done) return;
		if (event.key === 'Enter' || event.key === ' ') {
			event.preventDefault();
			progress = 1;
			done = true;
			onconfirm?.();
		}
	}
</script>

<div
	class="track"
	class:disabled={!enabled}
	class:dragging
	bind:this={track}
	bind:clientWidth={trackWidth}
	role="button"
	tabindex={enabled ? 0 : -1}
	aria-disabled={!enabled}
	aria-label={label}
	onpointerdown={start}
	onpointermove={move}
	onpointerup={end}
	onpointercancel={end}
	onkeydown={key}
>
	<span class="fill" style:width="{progress * 100}%"></span>
	<span class="label" style:opacity={1 - progress}>{label}</span>
	<span
		class="knob"
		class:snapping={!dragging}
		style:transform="translateX({progress * travel}px)"
		aria-hidden="true"
	>
		<Icon icon={UTILITY_ICONS['arrow-right']} size="md" />
	</span>
</div>

<style>
	/*
	  The track grows with its label rather than clip it (spec 089): at the
	  side panel's 360 px, and in long locales, the label is two lines or more.
	*/
	.track {
		position: relative;
		display: flex;
		align-items: center;
		min-height: var(--size-slideTrack);
		border-radius: var(--radius-full);
		background: var(--color-bg-sunken);
		overflow: hidden;
		cursor: grab;
		touch-action: none;
		user-select: none;
	}

	.track:focus-visible {
		outline: var(--border-emphasis) solid var(--color-fixed-focusRingOuter);
		outline-offset: var(--space-xs);
	}

	.dragging {
		cursor: grabbing;
	}

	/* Disabled is DIM, not hidden: cs5 leaves it visible so the reason it
	   cannot be used (the unlimited request above) stays connected to it. */
	.disabled {
		opacity: var(--opacity-disabled);
		cursor: not-allowed;
	}

	.fill {
		position: absolute;
		inset-block: 0;
		inset-inline-start: 0;
		background: var(--color-accent-soft);
	}

	/*
	  Laid out in the track BESIDE the knob, never under it (spec 089): centred
	  across the whole track, "Slide to confirm · Confirm send" ran under the
	  knob at the side panel's 360 px, and ru's two lines hid their first
	  letters there. It starts past the knob and its inset, and wraps.
	*/
	.label {
		flex: 1;
		min-width: 0;
		padding-block: var(--space-sm);
		padding-inline: calc(var(--size-slideKnob) + var(--space-sm) * 2) var(--space-lg);
		text-align: center;
		line-height: var(--leading-tight);
		overflow-wrap: anywhere;
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-muted);
	}

	.knob {
		position: absolute;
		inset-inline-start: var(--space-sm);
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-slideKnob);
		height: var(--size-slideKnob);
		border-radius: var(--radius-full);
		background: var(--color-accent-base);
		color: var(--color-onAccent);
	}

	/* The release spring, expressed as the token pair the SPEC board names. */
	.snapping {
		transition: transform var(--motion-duration-normal) cubic-bezier(0.2, 1.1, 0.4, 1);
	}

	@media (prefers-reduced-motion: reduce) {
		.snapping {
			transition: none;
		}
	}
</style>
