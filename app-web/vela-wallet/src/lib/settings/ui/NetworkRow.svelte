<script lang="ts">
	/**
	 * One network in the list (ST9 / DST4 / ST10's search results).
	 *
	 * Mark, name, chain-id line, an optional latency pill, an optional tag
	 * (自定义 / 测试网), and a trailing affordance that is a chevron on the
	 * phone and a disclosure caret on the desktop. Custom networks add the bin.
	 */
	import type { NetworkRowModel } from '../model';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import ChainMark from './ChainMark.svelte';
	import StatusPill from './StatusPill.svelte';

	interface Props {
		row: NetworkRowModel;
		/** `caret` is the desktop's expand-in-place; `chevron` pushes a page. */
		trailing?: 'chevron' | 'caret' | 'none';
		deleteLabel?: string;
		onselect?: (id: string) => void;
		ondelete?: (id: string) => void;
		/** Spec 105: a network the person added, to the other space. */
		onmove?: (id: string) => void;
	}

	let { row, trailing = 'chevron', deleteLabel, onselect, ondelete, onmove }: Props = $props();
</script>

<div class="wrap">
	<button type="button" class="row" onclick={() => onselect?.(row.id)}>
		<ChainMark mark={row.mark} />
		<span class="text">
			<span class="name">
				{row.name}
				{#if row.tag !== undefined}<span class="tag">{row.tag}</span>{/if}
			</span>
			<span class="meta">{row.meta}</span>
		</span>
		{#if row.badge !== undefined}
			<StatusPill pill={row.badge} />
		{/if}
	</button>

	{#if row.move !== undefined && onmove !== undefined}
		<button type="button" class="move" data-testid="move-network" onclick={() => onmove(row.id)}>
			{row.move}
		</button>
	{/if}

	{#if row.removable && deleteLabel !== undefined}
		<button type="button" class="icon" aria-label={deleteLabel} onclick={() => ondelete?.(row.id)}>
			<Icon icon={UTILITY_ICONS['trash-2']} size="md" />
		</button>
	{/if}

	{#if trailing === 'chevron'}
		<span class="trailing"><Icon icon={UTILITY_ICONS['chevron-right']} size="sm" /></span>
	{:else if trailing === 'caret'}
		<span class="trailing" class:up={row.expanded}>
			<Icon icon={UTILITY_ICONS['chevron-down']} size="sm" />
		</span>
	{/if}
</div>

<style>
	.wrap {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		border-bottom: var(--border-hairline) solid var(--color-border-base);
	}

	.row {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		flex: 1;
		min-width: 0;
		padding-block: var(--space-lg);
		padding-inline: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		text-align: start;
		cursor: pointer;
	}

	.text {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		flex: 1;
		min-width: 0;
	}

	.name {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.tag {
		padding-inline: var(--space-md);
		padding-block: var(--space-xs);
		border-radius: var(--radius-sm);
		background: var(--color-warning-soft);
		color: var(--color-warning-base);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		font-weight: var(--weight-medium);
	}

	.meta {
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.move {
		flex-shrink: 0;
		min-height: var(--size-hitTarget);
		padding-inline: var(--space-sm);
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-info-base);
		cursor: pointer;
	}

	.icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-hitTarget);
		height: var(--size-hitTarget);
		border: none;
		background: none;
		color: var(--color-fg-subtle);
		cursor: pointer;
	}

	.trailing {
		display: flex;
		color: var(--color-fg-subtle);
		flex-shrink: 0;
		transition: transform var(--motion-duration-fast) ease-out;
	}

	.trailing.up {
		transform: rotate(180deg);
	}

	@media (prefers-reduced-motion: reduce) {
		.trailing {
			transition: none;
		}
	}
</style>
