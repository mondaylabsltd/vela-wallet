<script lang="ts">
	/**
	 * SD1's token-class filter chips (spec 021 component 6): All / Stablecoins
	 * / Gas / Other.
	 *
	 * Distinct from `SegmentedToggle` on purpose. That control divides ONE
	 * space into named halves and always fills its width; this one is a row of
	 * independent narrowings that hugs its labels and can scroll past the
	 * screen edge when a locale needs the room.
	 */
	interface Props {
		options: { id: string; label: string; selected: boolean }[];
		label: string;
		onselect?: (id: string) => void;
	}

	let { options, label, onselect }: Props = $props();
</script>

<div class="chips" role="group" aria-label={label}>
	{#each options as option (option.id)}
		<button
			type="button"
			data-tap
			aria-pressed={option.selected}
			class:on={option.selected}
			onclick={() => onselect?.(option.id)}
		>
			{option.label}
		</button>
	{/each}
</div>

<style>
	.chips {
		display: flex;
		gap: var(--space-sm);
		overflow-x: auto;
		scrollbar-width: none;
		/* The row bleeds to the screen edges so a scrolled chip disappears at
		   the edge rather than at the text margin. */
		margin-inline: calc(var(--layout-screenPaddingX) * -1);
		padding-inline: var(--layout-screenPaddingX);
		/* Room inside the scroller for each chip's full hit target (`data-tap`,
		   vis-12) — a scroller clips what leaves it — given back outside, so
		   the row sits where it did. */
		margin-block: calc(var(--space-lg) * -1);
		padding-block: var(--space-lg);
	}

	.chips::-webkit-scrollbar {
		display: none;
	}

	button {
		flex-shrink: 0;
		padding: var(--space-sm) var(--space-lg);
		border: none;
		border-radius: var(--radius-full);
		background: var(--color-bg-raised);
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-medium);
		color: var(--color-fg-muted);
		cursor: pointer;
		transition:
			background var(--motion-duration-fast) ease,
			color var(--motion-duration-fast) ease;
	}

	/* The selected chip inverts rather than taking the accent: accent means
	   "moves money" in this product, and narrowing a list does not. */
	.on {
		background: var(--color-fg-base);
		color: var(--color-bg-base);
		font-weight: var(--weight-semibold);
	}
</style>
