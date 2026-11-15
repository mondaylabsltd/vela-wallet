<script lang="ts">
	import Icon from '#lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS } from '#lib/wallet/icons.ts';
	import LetterAvatar from '#lib/ui/LetterAvatar.svelte';
	import type { TileModel } from '../model';

	interface Props {
		tile: TileModel;
		size?: number;
		onopen?: (id: string) => void;
		oncontext?: (id: string, x: number, y: number) => void;
	}

	let { tile, size = 56, onopen, oncontext }: Props = $props();
</script>

<button
	type="button"
	class="tile"
	onclick={() => onopen?.(tile.kind === 'site' ? tile.site.id : 'add')}
	oncontextmenu={(event: MouseEvent) => {
		if (tile.kind !== 'site' || !oncontext) return;
		event.preventDefault();
		oncontext(tile.site.id, event.clientX, event.clientY);
	}}
>
	{#if tile.kind === 'site'}
		<LetterAvatar letter={tile.site.letter} tint={tile.site.tint} {size} />
		<span class="label">{tile.site.name}</span>
	{:else}
		<span class="add" style:width="{size}px" style:height="{size}px">
			<Icon icon={UTILITY_ICONS.plus} size="lg" />
		</span>
		<span class="label muted">{tile.label}</span>
	{/if}
</button>

<style>
	.tile {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-md);
		width: 100%;
		border: none;
		background: none;
		padding: 0;
		font-family: var(--font-ui);
		cursor: pointer;
	}

	.tile:active {
		transform: scale(var(--motion-press-row));
	}

	.add {
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: var(--radius-full);
		background: var(--color-bg-sunken);
		color: var(--color-fg-subtle);
	}

	.label {
		/* One step down from a row label: "PancakeSwap" has to fit a quarter of
		   a 392pt frame without an ellipsis, and in the mock it does. */
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-base);
		max-width: 100%;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.muted {
		color: var(--color-fg-subtle);
	}
</style>
