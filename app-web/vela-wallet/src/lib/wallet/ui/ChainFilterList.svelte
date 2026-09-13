<script lang="ts">
	import type { ChainRowModel } from '../model';
	import { UTILITY_ICONS } from '../icons';
	import Icon from './Icon.svelte';
	import RemoteLogo from './RemoteLogo.svelte';

	interface Props {
		rows: ChainRowModel[];
		/** The row itself: a live row carries its `chainId`, a drawn one only a name. */
		onselect?: (row: ChainRowModel) => void;
	}

	let { rows, onselect }: Props = $props();
</script>

<ul class="chains">
	{#each rows as row (row.name)}
		<li>
			<button type="button" aria-pressed={row.selected} onclick={() => onselect?.(row)}>
				<span class="mark" aria-hidden="true">
					<span
						class="dot"
						class:all={row.dot === 'all'}
						style:background={row.dot === 'all' ? undefined : row.dot}
					></span>
					<!-- The chain's logo from the data endpoint, over the dot the
					     boards draw; the dot shows until it loads and stays if it never does. -->
					<RemoteLogo urls={row.logoUrl === undefined ? undefined : [row.logoUrl]} />
				</span>
				<span class="name">{row.name}</span>
				{#if row.selected}
					<span class="checked"><Icon icon={UTILITY_ICONS.check} size="sm" /></span>
				{/if}
				<span class="count">{row.count}</span>
			</button>
		</li>
	{/each}
</ul>

<style>
	.chains {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	button {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		width: 100%;
		min-height: var(--size-hitTarget);
		padding-block: var(--space-sm);
		padding-inline: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		color: var(--color-fg-base);
		text-align: start;
		cursor: pointer;
		border-radius: var(--radius-md);
	}

	/* A fixed slot the size of a logo, with the dot centred in it, so rows
	   with and without a logo keep their names on one line. */
	.mark {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--icon-md);
		height: var(--icon-md);
		flex-shrink: 0;
	}

	.dot {
		width: var(--icon-xs);
		height: var(--icon-xs);
		border-radius: var(--radius-full);
	}

	.dot.all {
		background: var(--color-fg-subtle);
	}

	.name {
		flex: 1;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.checked {
		color: var(--color-accent-base);
		display: flex;
	}

	.count {
		font-family: var(--font-numeric);
		font-variant-numeric: tabular-nums;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}
</style>
