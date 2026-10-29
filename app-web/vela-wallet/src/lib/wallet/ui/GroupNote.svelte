<script lang="ts">
	/**
	 * The line under an activity group's label about the group itself — a
	 * multisig's waiting transactions that could not be refreshed — with the
	 * one thing to do about it, when there is one (the service is away: Retry).
	 */
	import type { ActivityGroupModel } from '../model';

	interface Props {
		group: ActivityGroupModel;
		/** The group's action, by its id. Absent in the gallery. */
		onaction?: (id: string) => void;
	}

	let { group, onaction }: Props = $props();
</script>

{#if group.note}
	<div class="line">
		<p class="note" role="status">{group.note}</p>
		{#if group.action !== undefined && onaction !== undefined}
			{@const action = group.action}
			<button type="button" class="action" onclick={() => onaction(action.id)}
				>{action.label}</button
			>
		{/if}
	</div>
{/if}

<style>
	.line {
		display: flex;
		align-items: baseline;
		gap: var(--space-md);
		margin: 0 0 var(--space-sm);
	}

	.note {
		flex: 1;
		min-width: 0;
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	/* The neutral ink every text action carries (vis-21). */
	.action {
		flex-shrink: 0;
		min-height: var(--size-hitTarget);
		padding: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
		cursor: pointer;
		white-space: nowrap;
	}
</style>
