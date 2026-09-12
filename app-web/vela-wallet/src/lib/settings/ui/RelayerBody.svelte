<script lang="ts">
	/**
	 * SR4 — fund this chain's bundler treasury.
	 *
	 * The QR and the copyable address are the payload; the amber note is the
	 * part that must not be missed, because this gas is non-refundable and goes
	 * to the bundler operator rather than to Vela or to the person's own
	 * transaction. It sits between the address and the CTA for that reason.
	 */
	import type { RelayerModel } from '../model';
	import Button from '$lib/ui/Button.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import QRPlaceholder from '$lib/wallet/ui/QRPlaceholder.svelte';
	import QRCard from '$lib/flows/ui/QRCard.svelte';
	import Callout from './Callout.svelte';
	import ChainMark from './ChainMark.svelte';

	interface Props {
		panel: RelayerModel;
		onprimary?: () => void;
		oncopy?: () => void;
	}

	let { panel, onprimary, oncopy }: Props = $props();
</script>

<div class="relayer">
	<p class="lead">{panel.lead}</p>

	<div class="identity">
		<ChainMark mark={panel.mark} />
		<span class="text">
			<span class="name">{panel.name}</span>
			<span class="hint">{panel.amountHint}</span>
		</span>
	</div>

	{#if panel.code !== undefined}
		<!-- A live sheet encodes the treasury's real address (spec 028 Phase 8). -->
		<div class="qr"><QRCard label={panel.qrCaption} code={panel.code} /></div>
	{:else}
		<div class="qr"><QRPlaceholder caption={panel.qrCaption} /></div>
	{/if}

	<button type="button" class="address" onclick={oncopy}>
		<span>{panel.addressDisplay}</span>
		<Icon icon={UTILITY_ICONS.copy} size="sm" />
	</button>

	<Callout callout={panel.callout} />

	<Button variant="primary" shape="rounded" onclick={onprimary}>{panel.primary}</Button>
</div>

<style>
	.relayer {
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
		padding-block: var(--space-md) var(--space-xl);
	}

	.lead {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
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
	}

	.name {
		font-size: calc(var(--text-xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	.hint {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.qr {
		display: flex;
		justify-content: center;
	}

	.address {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-md);
		min-height: var(--size-control-md);
		border: none;
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
		font-family: var(--font-mono);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
		cursor: pointer;
	}
</style>
