<script lang="ts">
	/**
	 * The account card above every QR (spec 021 component 17): whose address
	 * this is, spelled out in full, with one copy button.
	 *
	 * The address wraps to exactly two lines and never truncates. R2 is the
	 * screen a person reads an address OFF, and an ellipsis in the middle of it
	 * would defeat the only job the screen has.
	 */
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import Identicon from '$lib/wallet/ui/Identicon.svelte';
	import type { AddressCardModel } from '../model';

	interface Props {
		account: AddressCardModel;
		copied?: boolean;
		oncopy?: () => void;
	}

	let { account, copied = false, oncopy }: Props = $props();
</script>

<div class="card">
	<Identicon svg={account.identiconSvg} size="header" address={account.lines.join('')} />
	<span class="text">
		<span class="name">{account.name}</span>
		<span class="address">
			{#each account.lines as line, i (i)}<span class="line">{line}</span>{/each}
		</span>
	</span>
	<button type="button" aria-label={account.copyLabel} class:copied onclick={oncopy}>
		<Icon icon={copied ? UTILITY_ICONS.check : UTILITY_ICONS.copy} size="md" />
	</button>
</div>

<style>
	.card {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		padding-block: var(--space-lg);
	}

	.text {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		flex: 1;
		min-width: 0;
	}

	.name {
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.address {
		display: flex;
		flex-direction: column;
	}

	.line {
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
	}

	button {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-control-sm);
		height: var(--size-control-sm);
		flex-shrink: 0;
		border: none;
		border-radius: var(--radius-full);
		background: none;
		color: var(--color-fg-muted);
		cursor: pointer;
	}

	button:hover {
		background: var(--color-bg-raised);
		color: var(--color-fg-base);
	}

	.copied {
		color: var(--color-success-base);
	}
</style>
