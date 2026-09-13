<script lang="ts">
	/**
	 * ST10 / ST10b / ST10c / DST4b — search, then verdict.
	 *
	 * One panel for all three because they are one flow: the search results and
	 * the chosen candidate never coexist, and the compatible/incompatible
	 * verdict differs only in which marks the checklist carries and which CTA
	 * the panel ends on. The incompatible state gets an outline button and a
	 * re-check link rather than a disabled accent one — an action you cannot
	 * take should not be dressed as the action you came for.
	 */
	import type { AddNetworkModel } from '../model';
	import Button from '$lib/ui/Button.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import Callout from './Callout.svelte';
	import ChainMark from './ChainMark.svelte';
	import CheckList from './CheckList.svelte';
	import NetworkRow from './NetworkRow.svelte';
	import StatusPill from './StatusPill.svelte';
	import UrlField from './UrlField.svelte';

	interface Props {
		panel: AddNetworkModel;
		onselect?: (id: string) => void;
		onprimary?: () => void;
		onsecondary?: () => void;
		onrecheck?: () => void;
		/** Live wiring (spec 024). Absent = the gallery's pure picture. */
		onsearch?: (query: string) => void;
		oncustomrpc?: (value: string) => void;
	}

	let { panel, onselect, onprimary, onsecondary, onrecheck, onsearch, oncustomrpc }: Props =
		$props();
</script>

<div class="add-network">
	{#if panel.candidate === undefined}
		<label class="search" data-field>
			<Icon icon={UTILITY_ICONS.search} size="md" />
			<input
				type="search"
				value={panel.query ?? ''}
				placeholder={panel.searchPlaceholder}
				aria-label={panel.searchPlaceholder}
				oninput={(event) => onsearch?.(event.currentTarget.value)}
			/>
		</label>
		<div class="results">
			{#each panel.results as row (row.id)}
				<NetworkRow {row} {onselect} />
			{/each}
		</div>
	{:else}
		<div class="candidate">
			<ChainMark mark={panel.candidate.mark} />
			<span class="text">
				<span class="name">{panel.candidate.name}</span>
				<span class="meta">{panel.candidate.meta}</span>
			</span>
			<StatusPill pill={panel.candidate.badge} />
		</div>

		{#if panel.checks !== undefined && panel.checksTitle !== undefined}
			<CheckList title={panel.checksTitle} items={panel.checks} />
		{/if}

		{#if panel.customRpc !== undefined}
			<UrlField field={panel.customRpc} oninput={(value) => oncustomrpc?.(value)} />
		{/if}

		{#if panel.callout !== undefined}
			<Callout callout={panel.callout} />
		{/if}

		{#if panel.primary !== undefined}
			<Button variant="primary" shape="rounded" onclick={onprimary}>{panel.primary}</Button>
		{/if}
		{#if panel.secondary !== undefined}
			<Button variant="secondary" shape="rounded" onclick={onsecondary}>{panel.secondary}</Button>
		{/if}
		{#if panel.recheck !== undefined}
			<button type="button" class="recheck" onclick={onrecheck}>
				<Icon icon={UTILITY_ICONS['refresh-cw']} size="sm" />
				<span>{panel.recheck}</span>
			</button>
		{/if}
	{/if}
</div>

<style>
	.add-network {
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
	}

	.search {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		min-height: var(--size-control-md);
		padding-inline: var(--space-lg);
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
		color: var(--color-fg-subtle);
	}

	input {
		flex: 1;
		min-width: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
		outline: none;
	}

	.results {
		display: flex;
		flex-direction: column;
	}

	.candidate {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
	}

	.text {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		flex: 1;
		min-width: 0;
	}

	.name {
		font-size: calc(var(--text-xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	.meta {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.recheck {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-md);
		min-height: var(--size-control-md);
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-info-base);
		cursor: pointer;
	}
</style>
