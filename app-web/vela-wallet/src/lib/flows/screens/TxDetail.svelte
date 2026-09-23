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
	import { copyText } from '$lib/services/clipboard';
	import type { TxDetailModel } from '../model';

	interface Props {
		model: TxDetailModel;
		/** The record's delete (spec 028 Phase 8). Absent in the gallery. */
		ondelete?: () => void;
	}

	let { model, ondelete }: Props = $props();

	let copiedIndex = $state(-1);
	let timer: ReturnType<typeof setTimeout> | undefined;

	function copy(index: number) {
		const fact = model.facts[index];
		void copyText(fact?.copyValue ?? fact?.value ?? '');
		copiedIndex = index;
		clearTimeout(timer);
		timer = setTimeout(() => (copiedIndex = -1), 150);
	}
</script>

<div class="detail">
	<p class="head">
		<span class="what">{model.title}</span>
		<StatusChip chip={model.status} />
	</p>

	<AmountHero amount={model.amount} fiat={model.fiat} positive={model.positive} />

	<ul>
		{#each model.facts as fact, i (fact.label)}
			<li><FactRow {fact} copied={copiedIndex === i} oncopy={() => copy(i)} /></li>
		{/each}
	</ul>

	{#if model.breakdown !== undefined}
		<div class="parts"><Breakdown rows={model.breakdown} title={model.breakdownTitle} /></div>
	{/if}

	<div class="cta">
		{#if model.explorerUrl !== undefined}
			<Button variant="secondary" href={model.explorerUrl} external>{model.viewOnExplorer}</Button>
		{:else}
			<!-- No hash yet — a send the tracker has not resolved. The record's
			     own builder calls this "inert" and nothing made it LOOK inert: it
			     was drawn exactly like the working link, with a handler no caller
			     ever passed (spec 081, dead-controls #18). Kept on screen rather
			     than hidden, because it comes back the moment the hash lands and
			     a button that appears under a thumb is its own hazard. -->
			<Button variant="secondary" disabled>{model.viewOnExplorer}</Button>
		{/if}
		{#if model.deleteLabel !== undefined}
			<!-- Removes the local record only; the chain keeps the transaction. -->
			<Button variant="danger" onclick={ondelete}>{model.deleteLabel}</Button>
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
</style>
