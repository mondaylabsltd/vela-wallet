<script lang="ts">
	/**
	 * Spec 100: a page asks Vela to add a network — the card the request
	 * surface draws (the side panel's sheet, the request window's page), from
	 * `addNetworkCard`: who asks, what would be added, Settings' own check and
	 * only the buttons the core allows. Every judgement is the core's
	 * (`NetView.dapp_add`).
	 */
	import Button from '$lib/ui/Button.svelte';
	import Callout from '$lib/settings/ui/Callout.svelte';
	import CheckList from '$lib/settings/ui/CheckList.svelte';
	import KeyValueRow from '$lib/settings/ui/KeyValueRow.svelte';
	import StatusPill from '$lib/settings/ui/StatusPill.svelte';
	import type { AddNetworkCard } from './add-network';

	interface Props {
		card: AddNetworkCard;
		/** The approval is being claimed and saved. */
		busy?: boolean;
		/** Draw the lead as the heading (the window, which has no sheet title). */
		heading?: boolean;
		onadd: () => void;
		onretry: () => void;
		ondismiss: () => void;
		/** "Open Chain Setup Tool": the core's address for it, on the chain checked. */
		onsetup: (url: string) => void;
	}

	let { card, busy = false, heading = false, onadd, onretry, ondismiss, onsetup }: Props = $props();
</script>

<div class="card" data-testid="add-network-card">
	{#if heading}
		<h1>{card.title}</h1>
	{/if}
	<p class="lead">{card.lead}</p>
	<div class="rows">
		{#each card.rows as row (row.label)}
			<KeyValueRow row={{ label: row.label, value: row.value }} />
		{/each}
	</div>
	{#if card.fromSite}
		<Callout callout={{ tone: 'warning', text: card.fromSite }} />
	{/if}
	{#if card.pill}
		<div><StatusPill pill={card.pill} /></div>
	{/if}
	{#if card.checksTitle}
		<CheckList title={card.checksTitle} items={card.checks} />
	{/if}
	{#if card.note}
		<Callout callout={{ tone: 'warning', text: card.note }} />
	{/if}
	<div class="stack">
		{#if card.add}
			<Button variant="primary" shape="rounded" loading={busy} onclick={onadd}>{card.add}</Button>
		{/if}
		{#if card.retry}
			<Button variant="primary" shape="rounded" onclick={onretry}>{card.retry}</Button>
		{/if}
		{#if card.setupTool}
			{@const setup = card.setupTool}
			<Button variant="secondary" shape="rounded" onclick={() => onsetup(setup.url)}>
				{setup.label}
			</Button>
		{/if}
		<Button variant="secondary" shape="rounded" disabled={busy} onclick={ondismiss}>
			{card.dismiss}
		</Button>
	</div>
</div>

<style>
	.card {
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
		padding-bottom: var(--space-xl);
	}
	h1 {
		font-size: calc(var(--text-2xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		margin: 0;
	}
	.lead {
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		margin: 0;
	}
	.rows {
		display: flex;
		flex-direction: column;
	}
	.stack {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}
</style>
