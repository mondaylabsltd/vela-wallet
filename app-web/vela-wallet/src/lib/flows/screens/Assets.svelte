<script lang="ts">
	/**
	 * T1 / T4 / DT1L / DT4L — everything the wallet holds.
	 *
	 * T4 is the same screen with nothing in it, and it does more than say so:
	 * an empty asset list usually means either "you haven't received anything
	 * yet" or "you have, and we can't see it". The guidance card answers the
	 * second, because the person in that case is the one who needs help.
	 */
	import AssetRow from '$lib/wallet/ui/AssetRow.svelte';
	import EmptyState from '$lib/wallet/ui/EmptyState.svelte';
	import SkeletonRow from '$lib/wallet/ui/SkeletonRow.svelte';
	import Button from '$lib/ui/Button.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import SearchField from '../ui/SearchField.svelte';
	import HintCard from '../ui/HintCard.svelte';
	import type { AssetsModel } from '../model';

	interface Props {
		model: AssetsModel;
		onselect?: (index: number) => void;
		onadd?: () => void;
		onreceive?: () => void;
	}

	let { model, onselect, onadd, onreceive }: Props = $props();

	let query = $state('');

	// Narrowed once here rather than re-tested at every use: inside the
	// `{#if}` Svelte still sees `model.empty` as the mutable member it is.
	const empty = $derived(model.empty);

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

<div class="assets">
	<SearchField placeholder={model.searchPlaceholder} bind:value={query} />

	{#if model.loading}
		<SkeletonRow />
		<SkeletonRow />
		<SkeletonRow />
	{:else if empty !== undefined}
		<!-- The empty state is tappable: its caption says "tap here to see your
		     address", so it had better be the thing that does. -->
		<button type="button" class="empty-tap" onclick={onreceive}>
			<EmptyState icon={UTILITY_ICONS['credit-card']} title={empty.title} caption={empty.caption} />
		</button>
		<HintCard title={empty.hintTitle} body={empty.hintBody}>
			{#snippet cta()}
				<Button variant="secondary" onclick={onadd}>{empty.cta}</Button>
			{/snippet}
		</HintCard>
	{:else}
		<ul>
			{#each shown as entry (entry.index)}
				<li><AssetRow row={entry.row} onclick={() => onselect?.(entry.index)} /></li>
			{/each}
		</ul>
		<button type="button" class="by-address" onclick={onadd}>{model.addByAddress}</button>
	{/if}
</div>

<style>
	.assets {
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

	.empty-tap {
		border: none;
		background: none;
		padding: 0;
		font-family: var(--font-ui);
		cursor: pointer;
	}

	/* A link, not a button: adding a token by hand is the rare path out of a
	   list that normally fills itself, and it should read as one. */
	/* T1 sets this quiet, not as a link: it is the way out of "my token is
	   missing", not an action competing with the rows above it. */
	.by-address {
		align-self: center;
		padding: var(--space-md);
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		cursor: pointer;
	}
</style>
