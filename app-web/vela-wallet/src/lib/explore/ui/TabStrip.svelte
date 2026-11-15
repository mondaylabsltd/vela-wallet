<script lang="ts">
	import Icon from '#lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS } from '#lib/wallet/icons.ts';
	import LetterAvatar from '#lib/ui/LetterAvatar.svelte';
	import BrandMark from '#lib/ui/BrandMark.svelte';
	import type { TabModel } from '../model';

	/**
	 * The desktop tab strip (DE1–DE4). It sits in the window's drag strip, so
	 * the selected tab is drawn in the same colour as the toolbar below it —
	 * the two read as one surface, and the unselected tabs sink behind it.
	 */
	interface Props {
		tabs: TabModel[];
		newTabLabel: string;
		closeLabel: string;
		onselect?: (id: string) => void;
		onclose?: (id: string) => void;
		onnew?: () => void;
	}

	let { tabs, newTabLabel, closeLabel, onselect, onclose, onnew }: Props = $props();
</script>

<div class="strip">
	{#each tabs as tab (tab.id)}
		<div class="tab" class:selected={tab.selected}>
			<button type="button" class="face" onclick={() => onselect?.(tab.id)}>
				{#if tab.site}
					<LetterAvatar letter={tab.site.letter} tint={tab.site.tint} size={16} />
				{:else}
					<BrandMark size={14} />
				{/if}
				<span class="title">{tab.title}</span>
			</button>
			<button type="button" class="close" aria-label={closeLabel} onclick={() => onclose?.(tab.id)}>
				<Icon icon={UTILITY_ICONS.x} size="xs" />
			</button>
		</div>
	{/each}
	<button type="button" class="new" aria-label={newTabLabel} onclick={onnew}>
		<Icon icon={UTILITY_ICONS.plus} size="sm" />
	</button>
</div>

<style>
	.strip {
		display: flex;
		align-items: flex-end;
		gap: var(--space-xs);
		height: var(--size-desktopTabStrip);
		padding-inline: var(--space-lg);
		background: var(--color-bg-sunken);
	}

	.tab {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		width: var(--layout-desktopTabW);
		height: var(--size-desktopTab);
		padding-inline: var(--space-lg);
		border-start-start-radius: var(--radius-md);
		border-start-end-radius: var(--radius-md);
		color: var(--color-fg-muted);
	}

	.selected {
		background: var(--color-bg-base);
		color: var(--color-fg-base);
	}

	.face {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		flex: 1;
		min-width: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: inherit;
		cursor: pointer;
		text-align: start;
	}

	.title {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.close,
	.new {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--space-2xl);
		height: var(--space-2xl);
		border: none;
		border-radius: var(--radius-sm);
		background: none;
		color: var(--color-fg-muted);
		cursor: pointer;
	}

	.close:hover,
	.new:hover {
		background: var(--color-bg-raised);
		color: var(--color-fg-base);
	}

	.new {
		margin-bottom: var(--space-md);
	}
</style>
