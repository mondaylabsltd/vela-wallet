<script lang="ts">
	import Icon from '#lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS } from '#lib/wallet/icons.ts';
	import type { GroupModel } from '../model';

	interface Props {
		group: GroupModel;
		divider?: boolean;
		onclick?: () => void;
	}

	let { group, divider = false, onclick }: Props = $props();
</script>

<button type="button" class="group-row" class:divider {onclick}>
	<span class="tile" aria-hidden="true">
		<Icon icon={UTILITY_ICONS['users-round']} size="md" />
	</span>
	<span class="name">{group.name}</span>
	<span class="count">{group.countLabel}</span>
	<Icon icon={UTILITY_ICONS['chevron-right']} size="sm" />
</button>

<style>
	.group-row {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		width: 100%;
		padding-block: var(--space-lg);
		padding-inline: 0;
		border: none;
		background: none;
		color: var(--color-fg-subtle);
		font-family: var(--font-ui);
		text-align: start;
		cursor: pointer;
	}

	.group-row.divider {
		border-bottom: var(--border-hairline) solid var(--color-border-base);
	}

	.group-row:active {
		transform: scale(var(--motion-press-row));
	}

	.tile {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-control-sm);
		height: var(--size-control-sm);
		border-radius: var(--radius-md);
		background: var(--color-bg-sunken);
		color: var(--color-fg-muted);
		flex-shrink: 0;
	}

	.name {
		flex: 1;
		min-width: 0;
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.count {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}
</style>
