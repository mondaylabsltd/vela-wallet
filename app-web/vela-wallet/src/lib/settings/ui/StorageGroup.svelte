<script lang="ts">
	/**
	 * One group of storage lines (ST13 / DST7).
	 *
	 * The group label carries the consequence — "清除后无法找回" vs "清除后自动
	 * 重建" — which is why the 清除 action is red in the first group and plain
	 * in the second. The tone comes from the data, not from the position.
	 */
	import type { StorageGroupModel } from '../model';

	interface Props {
		group: StorageGroupModel;
		onclear?: (id: string) => void;
		ongroupaction?: () => void;
	}

	let { group, onclear, ongroupaction }: Props = $props();
</script>

<section class="group">
	<p class="label">{group.label}</p>
	<ul>
		{#each group.items as item (item.id)}
			<li>
				<span class="name">{item.label}</span>
				<span class="meta">{item.meta}</span>
				<button
					type="button"
					class="clear"
					class:destructive={item.destructive}
					onclick={() => onclear?.(item.id)}
				>
					{item.action}
				</button>
			</li>
		{/each}
	</ul>
	{#if group.action !== undefined}
		<button type="button" class="group-action" onclick={ongroupaction}>{group.action}</button>
	{/if}
</section>

<style>
	.group {
		padding-block: var(--space-xl) 0;
	}

	.label {
		margin: 0 0 var(--space-sm);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	li {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		min-height: var(--size-control-md);
		padding-block: var(--space-lg);
		border-bottom: var(--border-hairline) solid var(--color-border-base);
	}

	/* Wraps, never "…" (078 round 3: "dApp-Berechtigungen und -Sitzungen"). */
	.name {
		flex: 1;
		min-width: 0;
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		color: var(--color-fg-base);
		overflow-wrap: break-word;
	}

	.meta {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}

	.clear {
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		cursor: pointer;
		white-space: nowrap;
	}

	.clear.destructive {
		color: var(--color-error-base);
	}

	.group-action {
		display: block;
		width: 100%;
		margin-block-start: var(--space-xl);
		padding: var(--space-md);
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-info-base);
		cursor: pointer;
	}
</style>
