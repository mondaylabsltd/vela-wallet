<script lang="ts">
	/**
	 * SD2b's three ways to add a recipient (spec 021 component 24): by hand,
	 * from contacts, or from a spreadsheet — and SD2c's two ways to bring a
	 * list that is not pasted: pick a file, or take the template first.
	 *
	 * Outline pills, never accent: they add a ROW to a form, and the accent in
	 * this product is reserved for the button that actually moves the money.
	 *
	 * Each pill may lead with a glyph (issue 205: two grey words under a paste
	 * box did not read as things to press, let alone as "start here"). A pill
	 * that is `busy` keeps its emphasis and ignores a second press; one that is
	 * `done` wears the success colour for the thing it just did. Every pill
	 * gives under the finger, like everything else here that can be pressed.
	 */
	import { UTILITY_ICONS, type UtilityIconId } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';

	interface Item {
		id: string;
		label: string;
		icon?: UtilityIconId;
		busy?: boolean;
		done?: boolean;
	}

	interface Props {
		items: Item[];
		onselect?: (id: string) => void;
	}

	let { items, onselect }: Props = $props();
</script>

<div class="pills">
	{#each items as item (item.id)}
		<button
			type="button"
			class:done={item.done}
			aria-busy={item.busy ? 'true' : undefined}
			onclick={() => {
				if (!item.busy) onselect?.(item.id);
			}}
		>
			{#if item.icon !== undefined}
				<Icon icon={UTILITY_ICONS[item.icon]} size="sm" />
			{/if}
			<span>{item.label}</span>
		</button>
	{/each}
</div>

<style>
	.pills {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-sm);
	}

	button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-sm);
		flex: 1;
		/* Grows to share the row, but never below its own label: three locales
		   in, "From contacts" is longer than the third of a phone it would
		   otherwise get, and the row wraps instead of clipping. */
		min-width: max-content;
		min-height: var(--size-control-sm);
		/* Three of these share a phone's width in English with their glyphs;
		   any wider and the third drops to a line of its own. */
		padding: var(--space-md);
		border: var(--border-hairline) solid var(--color-border-strong);
		border-radius: var(--radius-full);
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-medium);
		color: var(--color-fg-base);
		cursor: pointer;
		transition:
			transform var(--motion-duration-fast) ease-out,
			background-color var(--motion-duration-fast) ease-out;
	}

	button:hover {
		background: var(--color-bg-raised);
	}

	button:active {
		transform: scale(var(--motion-press-button));
	}

	button[aria-busy='true'] {
		cursor: progress;
	}

	.done {
		border-color: var(--color-success-base);
		color: var(--color-success-base);
	}

	@media (prefers-reduced-motion: reduce) {
		button {
			transition: none;
		}
	}
</style>
