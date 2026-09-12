<script lang="ts">
	import { untrack } from 'svelte';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import type { SearchModel } from '../model';

	interface Props {
		search: SearchModel;
		/** 'mobile' = full-width well; 'desktop' = header field with the ⌘F badge. */
		layout?: 'mobile' | 'desktop';
		clearLabel: string;
		/** Live wiring (spec 024): every keystroke and the clear, reported. */
		onquery?: (value: string) => void;
	}

	let { search, layout = 'mobile', clearLabel, onquery }: Props = $props();

	// Pure UI state: the fixture seeds the query once, typing/clearing stays
	// local (filtering itself is fixture-side — FR-005), so the seed is read
	// untracked on purpose.
	let query = $state(untrack(() => search.query) ?? '');
</script>

<label class="field {layout}" data-field>
	<Icon icon={UTILITY_ICONS.search} size="sm" />
	<input
		type="search"
		placeholder={search.placeholder}
		bind:value={query}
		oninput={() => onquery?.(query)}
	/>
	{#if query !== ''}
		<button
			type="button"
			aria-label={clearLabel}
			onclick={() => {
				query = '';
				onquery?.('');
			}}
		>
			<Icon icon={UTILITY_ICONS.x} size="sm" />
		</button>
	{:else if search.shortcut !== undefined}
		<kbd>{search.shortcut}</kbd>
	{/if}
</label>

<style>
	.field {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		width: 100%;
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
		color: var(--color-fg-subtle);
	}

	.mobile {
		height: var(--size-control-md);
		padding-inline: var(--space-lg);
	}

	.desktop {
		height: var(--size-control-sm);
		padding-inline: var(--space-lg);
		border: var(--border-hairline) solid var(--color-border-base);
	}

	input {
		flex: 1;
		min-width: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
		outline: none;
	}

	input::placeholder {
		color: var(--color-fg-subtle);
	}

	input::-webkit-search-cancel-button {
		display: none;
	}

	button {
		display: flex;
		align-items: center;
		justify-content: center;
		border: none;
		background: none;
		color: var(--color-fg-subtle);
		cursor: pointer;
		padding: 0;
	}

	button:hover {
		color: var(--color-fg-base);
	}

	kbd {
		font-family: var(--font-mono);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}
</style>
