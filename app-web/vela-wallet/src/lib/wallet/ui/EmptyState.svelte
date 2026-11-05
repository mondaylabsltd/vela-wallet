<script lang="ts">
	import Button from '$lib/ui/Button.svelte';
	import type { IconDef } from '../icons';
	import Icon from './Icon.svelte';

	interface Props {
		icon: IconDef;
		title: string;
		caption: string;
		/** §18 item 3: Try again under a read that failed; `onaction` gets its id. */
		action?: { id: string; label: string };
		onaction?: (id: string) => void;
	}

	let { icon, title, caption, action, onaction }: Props = $props();
</script>

<div class="empty">
	<span class="circle"><Icon {icon} size="lg" /></span>
	<p class="title">{title}</p>
	{#if caption}<p class="caption">{caption}</p>{/if}
	{#if action}
		<Button
			variant="secondary"
			shape="rounded"
			testid="activity-empty-retry"
			onclick={() => onaction?.(action.id)}>{action.label}</Button
		>
	{/if}
</div>

<style>
	.empty {
		display: flex;
		flex-direction: column;
		align-items: center;
		text-align: center;
		gap: var(--space-md);
		padding-block: var(--space-4xl);
	}

	.circle {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-emptyStateCircle);
		height: var(--size-emptyStateCircle);
		border-radius: var(--radius-full);
		background: var(--color-bg-sunken);
		color: var(--color-fg-subtle);
		margin-bottom: var(--space-sm);
	}

	p {
		margin: 0;
	}

	.title {
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.caption {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}
</style>
