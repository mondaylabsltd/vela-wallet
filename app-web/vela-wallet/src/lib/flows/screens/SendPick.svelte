<script lang="ts">
	/**
	 * SD1 / SD1b / DSD1L — which token (or tokens) to send.
	 *
	 * SD1b is the same list in multi-select. Once the first token is chosen the
	 * network is decided, and rows on other chains grey out rather than
	 * disappearing — the person still owns them, and a list that silently
	 * shortened would read as a bug.
	 */
	import Button from '$lib/ui/Button.svelte';
	import AssetRow from '$lib/wallet/ui/AssetRow.svelte';
	import FilterChipRow from '../ui/FilterChipRow.svelte';
	import NoticeBanner from '../ui/NoticeBanner.svelte';
	import SearchField from '../ui/SearchField.svelte';
	import type { SendPickModel } from '../model';

	interface Props {
		model: SendPickModel;
		onfilter?: (id: string) => void;
		onselect?: (index: number) => void;
		onselectall?: () => void;
		oncta?: () => void;
	}

	let { model, onfilter, onselect, onselectall, oncta }: Props = $props();

	let query = $state('');

	const shown = $derived(
		query.trim() === ''
			? model.rows.map((row, index) => ({ row, index }))
			: model.rows
					.map((row, index) => ({ row, index }))
					.filter(({ row }) =>
						`${row.ticker} ${row.chain}`.toLowerCase().includes(query.trim().toLowerCase())
					)
	);
</script>

<div class="pick">
	<SearchField placeholder={model.searchPlaceholder} bind:value={query} />
	<FilterChipRow options={model.filters} label={model.searchPlaceholder} onselect={onfilter} />

	{#if model.notice !== undefined}
		<NoticeBanner text={model.notice.text} mark={model.notice.mark} />
	{/if}

	<ul>
		{#each shown as entry (entry.index)}
			<li>
				<AssetRow
					row={entry.row}
					selected={model.selection?.selected[entry.index] ?? false}
					dimmed={model.selection?.dimmed[entry.index] ?? false}
					onclick={() => onselect?.(entry.index)}
				/>
			</li>
		{/each}
	</ul>

	<!-- A list with nothing in it says so rather than showing a blank panel. -->
	{#if shown.length === 0 && model.empty !== undefined}
		<p class="empty">{model.empty}</p>
	{/if}

	{#if model.selection !== undefined}
		<button type="button" class="select-all" onclick={onselectall}>
			{model.selection.selectAll}
		</button>
	{/if}

	<div class="cta">
		<Button
			variant={model.cta.accent ? 'primary' : 'secondary'}
			shape={model.cta.accent ? 'rounded' : 'pill'}
			onclick={oncta}
		>
			{model.cta.label}
		</Button>
	</div>
</div>

<style>
	.pick {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
	}

	ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	li + li {
		border-top: var(--border-hairline) solid var(--color-border-base);
	}

	.empty {
		margin: 0;
		padding-block: var(--space-lg);
		text-align: center;
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.select-all {
		align-self: flex-start;
		padding: var(--space-sm) 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		cursor: pointer;
	}

	.cta {
		padding-block: var(--space-md) var(--space-xl);
	}
</style>
