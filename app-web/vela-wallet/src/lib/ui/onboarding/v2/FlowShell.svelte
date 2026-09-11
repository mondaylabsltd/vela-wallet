<script lang="ts">
	/**
	 * The v2 flow container: a full-page stepped journey.
	 *
	 * This replaces spec 014's two containers — the bottom sheet on mobile and
	 * the in-place action-column swap on desktop. The v2 design makes the flow
	 * the whole page at every width, and keeps the sheet for FAILURES only,
	 * where an interruption genuinely is modal.
	 *
	 * A back affordance, and nothing else. The stepped bar and the flow's name
	 * that used to sit here are gone (founder call, 2026-08-25): a
	 * three-segment meter over a journey whose every screen already says what
	 * it is measured decoration, not progress, and the label repeated the
	 * heading directly under it. Every screen inside decides its own content.
	 */
	import type { Snippet } from 'svelte';
	import OnboardingRail from './OnboardingRail.svelte';
	import type { RailSlot } from './rail';

	interface Props {
		backLabel: string;
		canGoBack: boolean;
		onBack: () => void;
		/** What the rail says beside this screen. Desktop widths only. */
		railSlot: RailSlot;
		children: Snippet;
	}

	let { backLabel, canGoBack, onBack, railSlot, children }: Props = $props();
</script>

<div class="frame">
	<OnboardingRail rail={railSlot} />

	<div class="shell">
		<header class="head">
			{#if canGoBack}
				<button class="back" type="button" onclick={onBack}>
					<span class="chevron" aria-hidden="true">‹</span>
					<span>{backLabel}</span>
				</button>
			{/if}
		</header>

		<div class="body">
			{@render children()}
		</div>
	</div>
</div>

<style>
	/* Below the desktop breakpoint the rail is `display: none` and this is the
	   single column it always was. At and above it the rail takes the left and
	   the screen sits beside it. */
	.frame {
		display: flex;
		flex: 1;
		min-height: 0;
		/* Rail and screen together, capped and centred past the widest the
		   mocks were drawn for (spec 038 T078). */
		width: 100%;
		max-width: var(--layout-frameMax);
		margin-inline: auto;
	}

	.shell {
		display: flex;
		flex: 1;
		flex-direction: column;
		width: 100%;
		max-width: var(--layout-flowColumn);
		margin-inline: auto;
	}

	@media (min-width: 1280px) {
		.shell {
			/* Left-aligned beside the rail, and at its NATURAL height. Pinning
			   the CTA to the viewport's bottom is a phone pattern — the screen is
			   the height of a hand there — and stretched to a desktop window it
			   opened the hole that made this read as a phone page pulled tall. */
			/* box-sizing is border-box globally, so the measure has to carry its own
			   padding — 520 of text plus 72 a side. */
			max-width: calc(var(--layout-onboardingColumn) + var(--layout-onboardingFrameGutter) * 2);
			margin-inline: 0;
			padding: var(--space-5xl) var(--layout-onboardingFrameGutter);
		}

		.body {
			flex: 0 0 auto;
		}
	}

	.head {
		display: flex;
		align-items: center;
		/* Reserves the row's height whether or not the affordance is there, so
		   the screen below never jumps when back disappears. */
		min-height: var(--size-hitTarget);
		padding-block-end: var(--space-3xl);
	}

	.back {
		display: flex;
		gap: var(--space-md);
		align-items: center;
		padding: 0;
		border: 0;
		background: none;
		color: var(--color-fg-muted);
		font-family: var(--font-ui);
		font-size: var(--text-base);
		font-weight: var(--weight-semibold);
		cursor: pointer;
		transition: color var(--motion-duration-fast) ease;
	}

	.back:hover {
		color: var(--color-fg-base);
	}

	.chevron {
		font-size: var(--text-lg);
		line-height: var(--leading-none);
	}

	.body {
		display: flex;
		flex: 1;
		flex-direction: column;
	}
</style>
