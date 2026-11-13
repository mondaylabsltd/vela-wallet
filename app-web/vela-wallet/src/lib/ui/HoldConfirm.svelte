<script lang="ts">
	/**
	 * R22 (review 1-3): the confirm a person HOLDS — the one control for every
	 * "… anyway" and danger confirm: the dApp sheet's, the hand-off card's
	 * Open, every multisig sign / propose / approve / execute. `holdMs` is the
	 * core's (`ConfirmState.hold_ms`, `SignDoor.hold_ms`); `0` draws the plain
	 * button, exactly as before.
	 *
	 * A pointer or a touch holds: the fill grows across the button over
	 * `holdMs`, letting go early does nothing, a hold that completes
	 * confirms. Assistive technology presses twice (WCAG 2.5.1 / 2.5.7) —
	 * Enter, Space, a screen reader's or a switch's activation arms it and it
	 * says "Press again to confirm"; a second within the core's window
	 * (`confirmAgainWindowMs()`) confirms; past it the button disarms. The
	 * rule is `hold-confirm.ts`; this draws it.
	 */
	import type { Snippet } from 'svelte';
	import { onDestroy } from 'svelte';
	import { confirmAgainWindowMs } from '$lib/core/kernels';
	import Button from './Button.svelte';
	import { holdConfirm, WALL_TIMERS, type HoldPhase, type HoldTimers } from './hold-confirm';

	interface Props {
		variant: 'primary' | 'value' | 'secondary' | 'danger';
		shape?: 'pill' | 'rounded';
		/** The core's hold, in ms; `0` (or absent) is a plain press. */
		holdMs?: number;
		/** "Hold to confirm" — beside the button, and its accessible hint. */
		holdLabel: string;
		/** "Press again to confirm" — the armed state of the assistive path. */
		againLabel: string;
		/** The assistive path's window; the core's when absent. */
		windowMs?: number;
		disabled?: boolean;
		loading?: boolean;
		testid?: string;
		/** A test's clock. */
		timers?: HoldTimers;
		/**
		 * The words are said by the host, in a line it already keeps (the
		 * signing sheet's note under its confirm — so nothing moves when the
		 * hold arrives): the element's id, and `phase` told on every change.
		 * Absent: the hint is drawn here, under the button.
		 */
		describedby?: string;
		onphase?: (phase: HoldPhase) => void;
		onconfirm?: () => void;
		children: Snippet;
	}

	let {
		variant,
		shape = 'rounded',
		holdMs = 0,
		holdLabel,
		againLabel,
		windowMs,
		disabled = false,
		loading = false,
		testid,
		timers,
		describedby,
		onphase,
		onconfirm,
		children
	}: Props = $props();

	const hintId = $props.id();
	let phase = $state<HoldPhase>('idle');

	const control = holdConfirm({
		holdMs: () => holdMs,
		windowMs: () => windowMs ?? confirmAgainWindowMs(),
		onconfirm: () => onconfirm?.(),
		onphase: (next) => {
			phase = next;
			onphase?.(next);
		},
		// A test's clock, else the wall's — read when a timer is set.
		timers: {
			set: (run, ms) => (timers ?? WALL_TIMERS).set(run, ms),
			clear: (handle) => (timers ?? WALL_TIMERS).clear(handle)
		}
	});

	// Shut, busy or no longer held: nothing is held and nothing stays armed.
	$effect(() => {
		if (disabled || loading || holdMs <= 0) control.reset();
	});
	onDestroy(() => control.reset());

	function down(event: PointerEvent): void {
		if (!event.isPrimary || event.button !== 0 || disabled || loading) return;
		// The hold is the button's, wherever the finger drifts on it.
		try {
			(event.currentTarget as Element | null)?.setPointerCapture?.(event.pointerId);
		} catch {
			// A pointer the browser no longer tracks: the hold still runs.
		}
		control.pressStart();
	}
</script>

{#if holdMs <= 0}
	<Button {variant} {shape} {testid} {disabled} {loading} onclick={() => onconfirm?.()}>
		{@render children()}
	</Button>
{:else}
	<div class="hold" data-phase={phase} data-hold-ms={holdMs}>
		<div class="press" style:--hold-ms={`${holdMs}ms`}>
			<Button
				{variant}
				{shape}
				{testid}
				{disabled}
				{loading}
				describedby={describedby ?? hintId}
				onpointerdown={down}
				onpointerend={() => control.pressEnd()}
				onclick={() => control.activate()}
			>
				{@render children()}
			</Button>
			<span
				class="fill"
				class:rounded={shape === 'rounded'}
				data-variant={variant}
				aria-hidden="true"
			></span>
		</div>
		{#if describedby === undefined}
			<!-- Said beside the button and read as its hint; the armed words are
			     announced when they replace it. -->
			<p
				class="hint"
				id={hintId}
				aria-live="polite"
				data-testid={testid ? `${testid}-hint` : undefined}
			>
				{phase === 'armed' ? againLabel : holdLabel}
			</p>
		{/if}
	</div>
{/if}

<style>
	.hold {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
		width: 100%;
	}

	.press {
		position: relative;
		display: flex;
	}

	.press > :global(*:first-child) {
		flex: 1;
	}

	/* The hold's progress: a sweep across the button, from where it starts. */
	.fill {
		position: absolute;
		inset: 0;
		border-radius: var(--radius-full);
		background: var(--color-onAccent);
		opacity: var(--opacity-backdrop);
		transform: scaleX(0);
		transform-origin: left center;
		pointer-events: none;
	}

	/* A bordered, unfilled button: the sweep in the ink, not the white. */
	.fill[data-variant='secondary'] {
		background: var(--color-fg-base);
	}

	.fill.rounded {
		border-radius: var(--radius-lg);
	}

	:global(:dir(rtl)) .fill {
		transform-origin: right center;
	}

	[data-phase='holding'] .fill {
		transform: scaleX(1);
		transition: transform var(--hold-ms) linear;
	}

	.hint {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
		text-align: center;
	}

	[data-phase='armed'] .hint {
		color: var(--color-fg-base);
		font-weight: var(--weight-semibold);
	}
</style>
