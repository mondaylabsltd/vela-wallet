<script lang="ts">
	/**
	 * ST9b / DST4's expanded editor — the chain's identity line, its RPC URL
	 * with a latency pill, the "why this might be refused" hint, and the
	 * explorer. The mismatch callout sits between the two fields, next to the
	 * field it is about.
	 */
	import type { NetworkDetailModel } from '../model';
	import Callout from './Callout.svelte';
	import ChainMark from './ChainMark.svelte';
	import StatusPill from './StatusPill.svelte';
	import SwitchRow from './SwitchRow.svelte';
	import UrlField from './UrlField.svelte';

	interface Props {
		detail: NetworkDetailModel;
		/** The desktop's inline editor drops the identity line — the row above
		 *  it already says which chain this is. */
		showIdentity?: boolean;
		/** Live wiring (spec 024). Absent = the gallery's pure picture. */
		onfield?: (field: 'rpc' | 'explorer', value: string) => void;
		onfieldblur?: (field: 'rpc' | 'explorer') => void;
		/** Owner decision 2026-10-28: the "Only use my node" switch. */
		ononlymine?: (on: boolean) => void;
	}

	let { detail, showIdentity = true, onfield, onfieldblur, ononlymine }: Props = $props();
</script>

<div class="detail">
	{#if showIdentity}
		<div class="identity">
			<ChainMark mark={detail.mark} />
			<span class="text">
				<span class="name">{detail.name}</span>
				<span class="note">{detail.note}</span>
			</span>
			<StatusPill pill={detail.badge} />
		</div>
	{/if}

	<UrlField
		field={detail.rpc}
		oninput={(value) => onfield?.('rpc', value)}
		onblur={() => onfieldblur?.('rpc')}
	/>
	{#if detail.onlyMine !== undefined}
		<!-- R22: right under the network's own RPC, never buried — and what it
		     means for the safety checks, said under it. -->
		<div class="only-mine">
			<SwitchRow row={detail.onlyMine} onchange={ononlymine} />
			{#if detail.onlyMineTrust !== undefined}
				<p class="trust" data-testid="only-mine-trust">{detail.onlyMineTrust}</p>
			{/if}
		</div>
	{/if}
	{#if detail.callout !== undefined}
		<Callout callout={detail.callout} />
	{/if}
	{#if detail.nodeCallout !== undefined}
		<Callout callout={detail.nodeCallout} />
	{/if}
	<UrlField
		field={detail.explorer}
		oninput={(value) => onfield?.('explorer', value)}
		onblur={() => onfieldblur?.('explorer')}
	/>
</div>

<style>
	.detail {
		display: flex;
		flex-direction: column;
		gap: var(--space-3xl);
	}

	.only-mine {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	.trust {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.identity {
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

	.note {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}
</style>
