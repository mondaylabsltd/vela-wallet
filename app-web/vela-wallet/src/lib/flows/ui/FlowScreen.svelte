<script lang="ts">
	/**
	 * The mobile page frame for every non-sheet screen in the wallet flows
	 * (spec 021 component 1).
	 *
	 * Back chevron on its own line, then a large title that may carry a
	 * trailing text action, a network pill, or neither. R1, A1, T1, SD1, SD2,
	 * SD3 and SD4 are all this frame with a different body — the mocks differ
	 * in what sits under the title, not in how the title sits.
	 */
	import type { Snippet } from 'svelte';
	import type { FlowHeaderModel } from '../model';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';

	interface Props {
		header: FlowHeaderModel;
		onback?: () => void;
		onaction?: () => void;
		onpill?: () => void;
		/** Pinned to the bottom of the screen, outside the scroller (SD2's CTA). */
		footer?: Snippet;
		children: Snippet;
	}

	let { header, onback, onaction, onpill, footer, children }: Props = $props();

	let barHeight = $state(0);
	let titleHeight = $state(0);

	/*
	 * vis-14: Escape is the ‹ — one step back (a send's confirm returns to its
	 * form), as the desktop column's Escape is. Not while a sheet or dialog is
	 * up over the screen: that Escape closes the sheet.
	 */
	function onkeydown(event: KeyboardEvent) {
		if (event.key !== 'Escape' || event.isComposing || event.defaultPrevented) return;
		if (onback === undefined) return;
		if (document.querySelector('[aria-modal="true"]') !== null) return;
		onback();
	}
</script>

<svelte:window {onkeydown} />

<!-- `--flow-screen-head`: how tall the back bar and the title are, for a
     screen that fills what is left under them (the receipt, vis-15). -->
<div class="screen">
	<div class="scroll" style:--flow-screen-head="{barHeight + titleHeight}px">
		<div class="bar" bind:offsetHeight={barHeight}>
			<button type="button" class="back" data-tap aria-label={header.backLabel} onclick={onback}>
				<Icon icon={UTILITY_ICONS['chevron-left']} size="lg" />
			</button>
			{#if header.action !== undefined}
				<button type="button" class="action" onclick={onaction}>{header.action}</button>
			{/if}
		</div>

		<div class="title-row" bind:offsetHeight={titleHeight}>
			<h1>{header.title}</h1>
			{#if header.pill !== undefined}
				<button type="button" class="pill" onclick={onpill}>
					<span class="dots" aria-hidden="true">
						{#each header.pill.dots as color, i (i)}
							<span class="dot" style:background={color}></span>
						{/each}
					</span>
					<span class="pill-label">{header.pill.label}</span>
					<Icon icon={UTILITY_ICONS['chevron-down']} size="sm" />
				</button>
			{/if}
		</div>

		{@render children()}
	</div>

	{#if footer}
		<div class="footer">{@render footer()}</div>
	{/if}
</div>

<style>
	.screen {
		display: flex;
		flex-direction: column;
		height: 100%;
		background: var(--color-bg-base);
		overflow: hidden;
	}

	.scroll {
		flex: 1;
		min-height: 0;
		overflow-y: auto;
		padding-inline: var(--layout-screenPaddingX);
	}

	.bar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding-block: var(--space-xl) var(--space-md);
	}

	.back {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-control-sm);
		height: var(--size-control-sm);
		/* The chevron's own glyph inset already reads as padding; pulling the
		   button back by it puts the STROKE on the screen margin, where the
		   title below it starts. */
		margin-inline-start: calc(var(--space-md) * -1);
		border: none;
		background: none;
		color: var(--color-fg-base);
		cursor: pointer;
	}

	.action {
		border: none;
		background: none;
		padding: var(--space-sm);
		margin-inline-end: calc(var(--space-sm) * -1);
		font-family: var(--font-ui);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-medium);
		color: var(--color-fg-base);
		cursor: pointer;
		border-radius: var(--radius-sm);
	}

	/* vis-8: the pill drops under a title that leaves it no room (a large
	   text size, a long word) and keeps to the right edge — the two never
	   sit on top of each other. */
	.title-row {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-md) var(--space-lg);
		padding-block: var(--space-md) var(--space-xl);
	}

	h1 {
		margin: 0;
		font-family: var(--font-display);
		font-size: calc(var(--text-4xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		line-height: var(--leading-tight);
		color: var(--color-fg-base);
		min-width: 0;
		overflow-wrap: break-word;
	}

	.pill {
		display: inline-flex;
		align-items: center;
		gap: var(--space-sm);
		flex-shrink: 0;
		margin-inline-start: auto;
		padding: var(--space-sm) var(--space-lg);
		border: none;
		border-radius: var(--radius-full);
		background: var(--color-bg-raised);
		font-family: var(--font-ui);
		color: var(--color-fg-base);
		cursor: pointer;
	}

	.dots {
		display: inline-flex;
	}

	/* Overlapped, not spaced: the cluster stands for "several networks", and
	   three separate dots would read as three separate controls. */
	.dot {
		width: var(--icon-sm);
		height: var(--icon-sm);
		border-radius: var(--radius-full);
		border: var(--border-emphasis) solid var(--color-bg-raised);
	}

	.dot:not(:first-child) {
		margin-inline-start: calc(var(--space-sm) * -1);
	}

	.pill-label {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
	}

	.footer {
		padding: var(--space-lg) var(--layout-screenPaddingX) var(--space-3xl);
	}
</style>
