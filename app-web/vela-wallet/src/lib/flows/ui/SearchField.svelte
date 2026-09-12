<script lang="ts">
	/**
	 * The filled search field (spec 021 component 4) — R1's network search,
	 * T1's and SD1's token search, SD2e's contact search.
	 *
	 * Filtering is live and animation-free by design (SPEC 动效 · 收款):
	 * rows leave as the query narrows, and a transition on a list that changes
	 * on every keystroke reads as lag rather than as polish.
	 */
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';

	interface Props {
		placeholder: string;
		value?: string;
		oninput?: (value: string) => void;
	}

	let { placeholder, value = $bindable(''), oninput }: Props = $props();
</script>

<div class="field" data-field>
	<span class="glyph" aria-hidden="true"><Icon icon={UTILITY_ICONS.search} size="md" /></span>
	<input
		type="search"
		{placeholder}
		aria-label={placeholder}
		bind:value
		oninput={(event) => oninput?.(event.currentTarget.value)}
	/>
</div>

<style>
	.field {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		padding-inline: var(--space-lg);
		height: var(--size-control-lg);
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
	}

	.glyph {
		display: flex;
		color: var(--color-fg-subtle);
	}

	input {
		flex: 1;
		min-width: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	input::placeholder {
		color: var(--color-fg-subtle);
	}

	/* Focus is the field's, not the bare input's, and it is app.css's rule
	   (`data-field`): one quiet edge, the same on every field. */

	/* Safari draws its own clear affordance; the field has no room for two. */
	input::-webkit-search-cancel-button {
		appearance: none;
	}
</style>
