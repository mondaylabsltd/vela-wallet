<script lang="ts">
	/**
	 * The send receipt's centrepiece (spec 021 component 20) — SD4a's spinner,
	 * SD4b's clock, SD4c's tick, and the failure cross.
	 *
	 * One disc size for all four so the mark does not resize as the
	 * transaction moves between them: the person is watching this circle, and
	 * a circle that jumps when the state changes reads as a new screen rather
	 * than as progress on the one they were already looking at.
	 *
	 * Issue 199: the wait can run to minutes, and a still grey clock over a
	 * still page reads as a hang. So the submitted disc wears a ring that
	 * fills as the chain's usual time passes, and breathes while it does. The
	 * ring is drawn OUTSIDE the disc — the disc keeps its one size — and it is
	 * the same ring that closes and turns green on confirmation, so the tick
	 * arrives as the end of what the person was watching.
	 */
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import type { ReceiptStage } from '../model';

	interface Props {
		stage: ReceiptStage;
		title: string;
		captions: string[];
		/**
		 * 0–1, how much of the ring is drawn. The screen owns the clock and
		 * the curve; undefined while submitted means "no estimate for this
		 * chain", and the ring circles instead of filling.
		 */
		progress?: number;
	}

	let { stage, title, captions, progress }: Props = $props();

	const ringed = $derived(stage === 'submitted' || stage === 'confirmed');
	const drawn = $derived(stage === 'confirmed' ? 1 : (progress ?? 0.25));
</script>

<div class="hero">
	<span class="disc {stage}" aria-hidden="true">
		{#if ringed}
			<svg
				class="ring"
				class:roaming={stage === 'submitted' && progress === undefined}
				viewBox="0 0 104 104"
			>
				<circle class="track" cx="52" cy="52" r="50" />
				<circle
					class="arc"
					cx="52"
					cy="52"
					r="50"
					pathLength="1"
					stroke-dasharray="1"
					stroke-dashoffset={1 - drawn}
				/>
			</svg>
		{/if}
		{#if stage === 'submitting'}
			<span class="spinner"></span>
		{:else if stage === 'submitted'}
			<Icon icon={UTILITY_ICONS.clock} size="xl" />
		{:else if stage === 'confirmed'}
			<span class="landed"><Icon icon={UTILITY_ICONS.check} size="xl" /></span>
		{:else}
			<Icon icon={UTILITY_ICONS.x} size="xl" />
		{/if}
	</span>
	<p class="title">{title}</p>
	{#each captions as caption, i (i)}
		<p class="caption" class:faint={i > 0}>{caption}</p>
	{/each}
</div>

<style>
	.hero {
		display: flex;
		flex-direction: column;
		align-items: center;
		text-align: center;
		gap: var(--space-sm);
		padding-block: var(--space-5xl) var(--space-3xl);
	}

	.disc {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-statusHero);
		height: var(--size-statusHero);
		border-radius: var(--radius-full);
		margin-bottom: var(--space-xl);
	}

	.submitting {
		background: var(--color-bg-sunken);
		color: var(--color-accent-base);
	}

	.submitted {
		background: var(--color-bg-sunken);
		color: var(--color-fg-muted);
	}

	.confirmed {
		background: var(--color-success-soft);
		color: var(--color-success-base);
	}

	.failed {
		background: var(--color-error-soft);
		color: var(--color-error-base);
	}

	/* 104 units across the 88-unit disc plus a --space-md gutter each side: the
	   ring clears the disc rather than outlining it. */
	.ring {
		position: absolute;
		inset: calc(var(--space-md) * -1);
		width: calc(100% + var(--space-md) * 2);
		height: calc(100% + var(--space-md) * 2);
		transform: rotate(-90deg);
		fill: none;
		stroke-width: 2.5;
		stroke-linecap: round;
		overflow: visible;
	}

	.track {
		stroke: var(--color-border-base);
	}

	/* One second per step, linear, because the screen ticks once a second:
	   the arc is always mid-move, which is the point. */
	.arc {
		stroke: var(--color-accent-base);
		transition:
			stroke-dashoffset 1s linear,
			stroke var(--motion-duration-slow) ease-out;
	}

	.confirmed .arc {
		stroke: var(--color-success-base);
		transition:
			stroke-dashoffset var(--motion-duration-slow) ease-out,
			stroke var(--motion-duration-slow) ease-out;
	}

	.confirmed .track {
		stroke: transparent;
	}

	.roaming {
		animation: roam 2.4s linear infinite;
	}

	.submitted {
		animation: breathe 2.4s ease-in-out infinite;
	}

	.landed {
		display: flex;
		animation: land var(--motion-duration-slow) cubic-bezier(0.2, 1.4, 0.4, 1) both;
	}

	@keyframes roam {
		from {
			transform: rotate(-90deg);
		}
		to {
			transform: rotate(270deg);
		}
	}

	@keyframes breathe {
		50% {
			transform: scale(1.04);
		}
	}

	@keyframes land {
		from {
			opacity: 0;
			transform: scale(0.6);
		}
	}

	.spinner {
		width: var(--icon-2xl);
		height: var(--icon-2xl);
		border: var(--border-emphasis) solid currentColor;
		border-top-color: transparent;
		border-radius: var(--radius-full);
		/* The same 800ms revolution the CTA spinner turns at: one wait speed
		   in the product, not one per surface. */
		animation: spin 800ms linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(1turn);
		}
	}

	@keyframes pulse {
		50% {
			opacity: 1;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		/* The ring still fills — that is information — but in steps, and
		   nothing breathes, circles or bounces. */
		.arc,
		.confirmed .arc {
			transition: none;
		}

		.submitted,
		.roaming,
		.landed {
			animation: none;
		}

		.spinner {
			border-top-color: currentColor;
			opacity: var(--opacity-dim);
			animation: pulse 1.2s ease-in-out infinite;
		}
	}

	p {
		margin: 0;
	}

	.title {
		font-size: calc(var(--text-2xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	.caption {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	/* The second caption is the one that says "you can leave" — true, useful,
	   and not what the person is waiting to read. */
	.faint {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}
</style>
