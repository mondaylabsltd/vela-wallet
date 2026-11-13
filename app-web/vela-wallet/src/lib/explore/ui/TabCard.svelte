<script lang="ts">
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import BrandMark from '$lib/ui/BrandMark.svelte';
	import LetterAvatar from '$lib/ui/LetterAvatar.svelte';
	import type { TabModel } from '../model';

	interface Props {
		tab: TabModel;
		closeLabel: string;
		onopen?: (id: string) => void;
		onclose?: (id: string) => void;
	}

	let { tab, closeLabel, onopen, onclose }: Props = $props();
</script>

<div class="card" class:selected={tab.selected}>
	<button type="button" class="preview" onclick={() => onopen?.(tab.id)}>
		{#if tab.startPage}
			<BrandMark size={44} />
			<span class="bar wide"></span>
		{:else}
			<span class="bar"></span>
			<span class="bar"></span>
			<span class="cta" style:background={tab.site?.tint}></span>
		{/if}
	</button>
	<footer>
		{#if tab.site}
			<LetterAvatar letter={tab.site.letter} tint={tab.site.tint} size={20} />
		{/if}
		<span class="title">{tab.title}</span>
		<button type="button" class="close" aria-label={closeLabel} onclick={() => onclose?.(tab.id)}>
			<Icon icon={UTILITY_ICONS.x} size="base" />
		</button>
	</footer>
</div>

<style>
	.card {
		display: flex;
		flex-direction: column;
		border-radius: var(--radius-xl);
		background: var(--color-bg-sunken);
		overflow: hidden;
		border: var(--border-emphasis) solid transparent;
	}

	.selected {
		border-color: var(--color-fg-base);
	}

	.preview {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: var(--space-md);
		aspect-ratio: 3 / 4;
		padding: var(--space-xl);
		border: none;
		background: none;
		cursor: pointer;
	}

	.bar {
		width: 100%;
		height: var(--space-3xl);
		border-radius: var(--radius-md);
		background: var(--color-bg-raised);
	}

	.wide {
		height: var(--space-lg);
		width: 70%;
	}

	.cta {
		width: 100%;
		height: var(--space-2xl);
		border-radius: var(--radius-full);
		opacity: 0.7;
	}

	footer {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		padding: var(--space-lg);
		background: var(--color-bg-raised);
	}

	.title {
		flex: 1;
		min-width: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.close {
		display: flex;
		border: none;
		background: none;
		color: var(--color-fg-muted);
		cursor: pointer;
	}
</style>
