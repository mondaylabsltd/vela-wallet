<script lang="ts">
	/**
	 * A contact's address as a code (spec 028 US5, the detail's 二维码): the
	 * receive card's geometry with THEIR address in it, so another wallet can
	 * scan it and pay them. The centre carries their identicon — the same
	 * anti-forgery mark the share card wears — and the address is printed in
	 * full beneath, because a code nobody can read is a code nobody can check.
	 */
	import QRCard from '$lib/flows/ui/QRCard.svelte';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import Identicon from '$lib/wallet/ui/Identicon.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import type { QrCode } from '$lib/wallet/qr';

	interface Props {
		name: string;
		address: string;
		identiconSvg: string;
		code: QrCode;
		copyLabel: string;
		copiedLabel: string;
		copied?: boolean;
		oncopy?: () => void;
	}

	let {
		name,
		address,
		identiconSvg,
		code,
		copyLabel,
		copiedLabel,
		copied = false,
		oncopy
	}: Props = $props();
</script>

<div class="qr">
	<div class="card">
		<QRCard label={name} {code}>
			{#snippet centre()}
				<Identicon svg={identiconSvg} size="row" label={name} />
			{/snippet}
		</QRCard>
	</div>
	<p class="name">{name}</p>
	<p class="address">{address}</p>
	<button type="button" class="copy" onclick={oncopy}>
		<Icon icon={UTILITY_ICONS[copied ? 'check' : 'copy']} size="sm" />
		<span>{copied ? copiedLabel : copyLabel}</span>
	</button>
</div>

<style>
	.qr {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-md);
		padding-block-end: var(--space-xl);
	}

	.card {
		display: flex;
		justify-content: center;
		width: 100%;
	}

	.name {
		margin: var(--space-md) 0 0;
		font-size: calc(var(--text-xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	.address {
		margin: 0;
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		word-break: break-all;
		text-align: center;
	}

	.copy {
		display: inline-flex;
		align-items: center;
		gap: var(--space-sm);
		height: var(--size-control-sm);
		padding-inline: var(--space-xl);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-full);
		background: var(--color-bg-raised);
		color: var(--color-fg-base);
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		cursor: pointer;
	}
</style>
