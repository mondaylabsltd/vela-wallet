<script lang="ts">
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import type { GroupRailModel } from '../model';

	interface Props {
		rail: GroupRailModel;
		onselect?: (group: string | undefined) => void;
		/** Right-click on a group row (DC6 context menu). */
		ongroupmenu?: (group: string, event: MouseEvent) => void;
		/** 新建分组. Absent in the gallery. */
		onnew?: () => void;
	}

	let { rail, onselect, ongroupmenu, onnew }: Props = $props();
</script>

<nav class="rail" aria-label={rail.groupsTitle}>
	<button
		type="button"
		class="rail-row all"
		class:selected={rail.allSelected}
		aria-current={rail.allSelected ? 'true' : undefined}
		onclick={() => onselect?.(undefined)}
	>
		<span class="label">{rail.allLabel}</span>
		<span class="count">{rail.allCount}</span>
	</button>

	{#if rail.groups.length > 0}
		<p class="rail-title">{rail.groupsTitle}</p>
	{/if}

	{#each rail.groups as group (group.name)}
		<button
			type="button"
			class="rail-row"
			class:selected={rail.selectedGroup === group.name}
			class:drop-target={rail.dropTarget === group.name}
			data-contacts-group={group.name}
			aria-current={rail.selectedGroup === group.name ? 'true' : undefined}
			onclick={() => onselect?.(group.name)}
			oncontextmenu={(event) => ongroupmenu?.(group.name, event)}
		>
			<Icon icon={UTILITY_ICONS['users-round']} size="base" />
			<span class="label">{group.name}</span>
			<span class="count">{group.count}</span>
		</button>
	{/each}

	<button type="button" class="rail-row new" onclick={onnew}>
		<Icon icon={UTILITY_ICONS['folder-plus']} size="base" />
		<span class="label">{rail.newGroup}</span>
	</button>
</nav>

<style>
	.rail {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		width: var(--layout-contactsRailW);
		flex-shrink: 0;
		padding-inline-end: var(--space-xl);
	}

	.rail-row {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		height: var(--size-control-sm);
		padding-inline: var(--space-lg);
		border: none;
		border-radius: var(--radius-md);
		background: none;
		color: var(--color-fg-muted);
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		text-align: start;
		cursor: pointer;
		transition: background var(--motion-hover) ease-out;
	}

	.rail-row:hover {
		background: var(--color-bg-raised);
		color: var(--color-fg-base);
	}

	.rail-row.selected,
	.rail-row.drop-target {
		background: var(--color-bg-raised);
		color: var(--color-fg-base);
		font-weight: var(--weight-semibold);
	}

	.rail-row.drop-target {
		outline: var(--border-emphasis) dashed var(--color-accent-base);
		outline-offset: calc(var(--space-xs) * -1);
	}

	.rail-row.all {
		border-bottom: var(--border-hairline) solid transparent;
	}

	.label {
		flex: 1;
		min-width: 0;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.count {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
		font-variant-numeric: tabular-nums;
	}

	.rail-title {
		margin: 0;
		padding: var(--space-xl) var(--space-lg) var(--space-sm);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		letter-spacing: var(--letterSpacing-sectionLabel);
		color: var(--color-fg-subtle);
		border-top: var(--border-hairline) solid var(--color-border-base);
		margin-top: var(--space-md);
	}

	.new {
		color: var(--color-fg-subtle);
	}

	@media (prefers-reduced-motion: reduce) {
		.rail-row {
			transition: none;
		}
	}
</style>
