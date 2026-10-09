<script lang="ts">
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import type { GroupManageRow } from '../model';

	/**
	 * The start page's sections (E3): Favorites and Recent dApps, each with an
	 * eye. Issue 465 took the person's own groups away, and with them the
	 * grip, the trash and "New group" — neither section can be deleted or
	 * moved, and an affordance that is only ever refused is a lie about what
	 * is possible. Hiding stays: the Favorites heading's Edit is the way back
	 * to a hidden section.
	 */
	interface Props {
		title: string;
		rows: GroupManageRow[];
		closeLabel: string;
		hideLabel: string;
		showLabel: string;
		onclose?: () => void;
		ontoggle?: (id: string) => void;
	}

	let { title, rows, closeLabel, hideLabel, showLabel, onclose, ontoggle }: Props = $props();
</script>

<header>
	<h2>{title}</h2>
	<button type="button" class="close" aria-label={closeLabel} onclick={onclose}>
		<Icon icon={UTILITY_ICONS.x} size="lg" />
	</button>
</header>

<ul>
	{#each rows as row (row.id)}
		<li class="row" class:hidden={row.hidden}>
			<span class="title">{row.title}</span>
			{#if row.meta}
				<span class="meta">{row.meta}</span>
			{/if}
			<button
				type="button"
				class="icon"
				aria-label={row.hidden ? showLabel : hideLabel}
				onclick={() => ontoggle?.(row.id)}
			>
				<Icon icon={UTILITY_ICONS[row.hidden ? 'eye-off' : 'eye']} size="base" />
			</button>
		</li>
	{/each}
</ul>

<style>
	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding-block: var(--space-md) var(--space-lg);
	}

	h2 {
		margin: 0;
		font-size: calc(var(--text-2xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	.close {
		display: flex;
		border: none;
		background: none;
		color: var(--color-fg-muted);
		cursor: pointer;
	}

	ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	.row {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		padding-block: var(--space-xl);
		border-bottom: var(--border-hairline) solid var(--color-border-base);
	}

	.title {
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.meta {
		flex: 1;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	/* Hidden reads as hidden: the row itself dims, so the eye icon is a
	   confirmation rather than the only clue. */
	.hidden .title,
	.hidden .meta {
		opacity: var(--opacity-dim);
	}

	/* The eye keeps the trailing edge on a row with no meta (Recent) too. */
	.icon {
		display: flex;
		margin-inline-start: auto;
		border: none;
		background: none;
		color: var(--color-fg-muted);
		cursor: pointer;
	}
</style>
