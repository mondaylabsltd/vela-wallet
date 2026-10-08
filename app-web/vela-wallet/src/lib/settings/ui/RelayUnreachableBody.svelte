<script lang="ts">
	/**
	 * Spec 098 §2 — the relay cannot serve this chain.
	 *
	 * Not the funding sheet: a relay that cannot reach a network cannot use gas
	 * sent to it there. Until 098 this case went on to the passkey and failed
	 * after the person had signed, without a word; now the send stops at
	 * Continue and says whose it is to fix.
	 *
	 * - **A network Vela ships**: the operator's. Report it.
	 * - **A network the person added**: theirs. The relay reads it through the
	 *   RPC they set for it, so that RPC must be a public `https` address; a node
	 *   on their own machine or network can only be served by a relay beside it.
	 */
	import type { RelayUnreachableModel } from '../model';
	import Button from '$lib/ui/Button.svelte';
	import ChainMark from './ChainMark.svelte';

	interface Props {
		panel: RelayUnreachableModel;
		onprimary?: () => void;
		/**
		 * "Report this" (issue 466): the in-app report, in this same sheet,
		 * seeded with the core's account of the stop and filed under its
		 * marker — as on the treasury sheet. Absent in the gallery.
		 */
		onreport?: () => void;
	}

	let { panel, onprimary, onreport }: Props = $props();
</script>

<div class="unreachable">
	<div class="identity">
		<ChainMark mark={panel.mark} />
		<span class="name">{panel.name}</span>
	</div>

	<p class="lead">{panel.lead}</p>

	{#if panel.hint}
		<p class="hint">{panel.hint}</p>
	{/if}

	{#if panel.report}
		<!-- The same hook as the treasury sheet's, and the phones'. -->
		<button type="button" class="report" data-testid="relay-report" onclick={onreport}
			>{panel.report.label}</button
		>
	{/if}

	<Button variant="primary" shape="rounded" onclick={onprimary}>{panel.primary}</Button>
</div>

<style>
	.unreachable {
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
		padding-block: var(--space-md) var(--space-xl);
	}

	.identity {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
	}

	/* The same name, lead and report row as the treasury sheet beside it. */
	.name {
		font-size: calc(var(--text-xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	.lead {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
	}

	.hint {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-subtle);
	}

	.report {
		display: flex;
		align-items: center;
		justify-content: center;
		min-height: var(--size-control-md);
		border: none;
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
		cursor: pointer;
	}
</style>
