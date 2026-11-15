<script lang="ts">
	/**
	 * ST10b/ST10c's compatibility checklist: a green check or a red cross per
	 * requirement. Both states are the same list — only the marks change, which
	 * is what makes "not compatible" readable as "these three, not all four".
	 */
	import type { CheckItemModel } from '../model';
	import { UTILITY_ICONS } from '#lib/wallet/icons.ts';
	import Icon from '#lib/wallet/ui/Icon.svelte';

	interface Props {
		title: string;
		items: CheckItemModel[];
	}

	let { title, items }: Props = $props();
</script>

<section class="checks">
	<p class="title">{title}</p>
	<ul>
		{#each items as item (item.label)}
			<li class:ok={item.ok}>
				<Icon icon={item.ok ? UTILITY_ICONS.check : UTILITY_ICONS.x} size="md" />
				<span>{item.label}</span>
			</li>
		{/each}
	</ul>
</section>

<style>
	.checks {
		padding: var(--space-xl);
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
		border: var(--border-hairline) solid var(--color-border-base);
	}

	.title {
		margin: 0 0 var(--space-lg);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	ul {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
	}

	li {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-error-base);
	}

	li.ok {
		color: var(--color-success-base);
	}

	li span {
		color: var(--color-fg-base);
	}
</style>
