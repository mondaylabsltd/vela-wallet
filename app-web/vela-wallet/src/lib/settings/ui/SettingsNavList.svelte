<script lang="ts">
	/**
	 * The desktop's second-level nav (DST1–DST8): the nav column between the
	 * app sidebar and the panel. It is the phone's settings list with the rows
	 * collapsed to their titles — same ids, same order, so a person who learned
	 * one knows the other.
	 */
	import type { SettingsNavItemModel, SettingsPageId } from '../model';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';

	interface Props {
		title: string;
		items: SettingsNavItemModel[];
		selected: SettingsPageId;
		onselect?: (id: SettingsPageId) => void;
	}

	let { title, items, selected, onselect }: Props = $props();
</script>

<nav class="settings-nav" aria-label={title}>
	<h2>{title}</h2>
	{#each items as item (item.id)}
		<button
			type="button"
			class:selected={item.id === selected}
			aria-current={item.id === selected ? 'page' : undefined}
			onclick={() => onselect?.(item.id)}
		>
			<Icon icon={UTILITY_ICONS[item.icon]} size="md" />
			<span>{item.label}</span>
		</button>
	{/each}
</nav>

<style>
	.settings-nav {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		/* As wide as its longest label, and never narrower than the 216 the
		   mocks measured (issue 198). A fixed 216 only ever fit English at the
		   standard text size: "Service Endpoints" wrapped at the largest size,
		   and ja/ru wrapped at the standard one — one two-line item in a list of
		   one-line items. The labels are a closed set from the corpus, so the
		   column is bounded by them; multiplying the width by --text-scale was
		   tried first and still wrapped ru, ja, es, pt and fr. */
		width: max-content;
		min-width: var(--layout-settingsNavW);
		flex-shrink: 0;
		height: 100%;
		padding: var(--space-xl) var(--space-lg);
		background: var(--color-bg-sunken);
		border-inline-end: var(--border-hairline) solid var(--color-border-base);
	}

	h2 {
		margin: 0 0 var(--space-xl);
		padding-inline: var(--space-lg);
		font-size: calc(var(--text-2xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	button {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		min-height: var(--size-control-sm);
		padding-inline: var(--space-lg);
		border: none;
		border-radius: var(--radius-lg);
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		text-align: start;
		cursor: pointer;
	}

	button span {
		white-space: nowrap;
	}

	button:hover {
		color: var(--color-fg-base);
	}

	/* raised + a hairline: on dark the raised fill alone is barely a step off
	   sunken, and the selected row has to be unmistakable (SPEC 暗色注意). */
	.selected {
		background: var(--color-bg-raised);
		border: var(--border-hairline) solid var(--color-border-base);
		color: var(--color-fg-base);
		font-weight: var(--weight-semibold);
	}
</style>
