<script lang="ts">
	/**
	 * A2 / A3 / DA2L / DA3L — one transaction.
	 *
	 * A2 is a received ERC-20 and A3 a sent native coin; the difference between
	 * them is entirely in the fact list (a native coin has no contract row), so
	 * this component takes the facts as data rather than branching on a kind.
	 */
	import Button from '$lib/ui/Button.svelte';
	import AmountHero from '../ui/AmountHero.svelte';
	import Breakdown from '../ui/Breakdown.svelte';
	import FactRow from '../ui/FactRow.svelte';
	import StatusChip from '../ui/StatusChip.svelte';
	import TxTechnical from '../ui/TxTechnical.svelte';
	import { copyText } from '$lib/services/clipboard';
	import type { FactRowModel, TxDetailModel } from '../model';

	interface Props {
		model: TxDetailModel;
		/** The record's delete (spec 028 Phase 8). Absent in the gallery. */
		ondelete?: () => void;
	}

	let { model, ondelete }: Props = $props();

	let copiedFact = $state<FactRowModel | null>(null);
	let timer: ReturnType<typeof setTimeout> | undefined;

	function copy(fact: FactRowModel) {
		void copyText(fact.copyValue ?? fact.value);
		copiedFact = fact;
		clearTimeout(timer);
		timer = setTimeout(() => (copiedFact = null), 150);
	}
</script>

<div class="detail">
	<p class="head">
		<span class="what">{model.title}</span>
		<!-- A signature has no lifecycle to settle (spec 093): no chip. -->
		{#if model.status !== undefined}
			<StatusChip chip={model.status} />
		{/if}
	</p>
	{#if model.note !== undefined}
		<p class="note">{model.note}</p>
	{/if}

	<!-- A dApp call that moved and granted nothing has no figure (083 H2): no empty hero. -->
	{#if model.amount !== '' || model.fiat !== ''}
		<AmountHero
			amount={model.amount}
			fiat={model.fiat}
			positive={model.positive}
			danger={model.danger}
			received={model.received}
		/>
	{/if}

	<ul>
		<!-- Keyed by position: a balance change's second line has no label of its own. -->
		{#each model.facts as fact, i (i)}
			<li><FactRow {fact} copied={copiedFact === fact} oncopy={() => copy(fact)} /></li>
		{/each}
	</ul>

	{#if model.breakdown !== undefined}
		<div class="parts"><Breakdown rows={model.breakdown} title={model.breakdownTitle} /></div>
	{/if}

	{#if model.technical !== undefined}
		{#key model.technical.key}
			<TxTechnical model={model.technical} oncopy={copy} copied={copiedFact} />
		{/key}
	{/if}

	<div class="cta">
		<!-- No transaction hash, no explorer control (spec 082 RJ16): an op the
		     relay refused never reached the chain, and a greyed link offered it
		     anyway (G52). A pending send's control appears once its hash lands. -->
		{#if model.explorerUrl !== undefined}
			<Button variant="secondary" href={model.explorerUrl} external>{model.viewOnExplorer}</Button>
		{/if}
		{#if model.deleteLabel !== undefined}
			<!-- Removes the local record only; the chain keeps the transaction. On
			     a pending record it is a quiet control (RJ18): the record is the
			     "don't send it again" trace, not something to clear first. -->
			{#if model.deleteQuiet}
				<button type="button" class="quiet" onclick={ondelete}>{model.deleteLabel}</button>
			{:else}
				<Button variant="danger" onclick={ondelete}>{model.deleteLabel}</Button>
			{/if}
		{/if}
	</div>
</div>

<style>
	.detail {
		display: flex;
		flex-direction: column;
	}

	.head {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		margin: 0;
	}

	.what {
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.note {
		margin: 0;
		padding-top: var(--space-sm);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
	}

	ul {
		list-style: none;
		margin: 0;
		padding: 0;
		border-top: var(--border-hairline) solid var(--color-border-base);
	}

	li + li {
		border-top: var(--border-hairline) solid var(--color-border-base);
	}

	.parts {
		padding-top: var(--space-lg);
	}

	.cta {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
		padding-top: var(--space-xl);
	}

	.quiet {
		align-self: center;
		min-height: var(--size-control-md);
		padding-inline: var(--space-lg);
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		cursor: pointer;
	}

	.quiet:hover {
		color: var(--color-fg-base);
	}
</style>
