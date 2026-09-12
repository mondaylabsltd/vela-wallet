<script lang="ts">
	/**
	 * The network-fee row (spec 021 component 26), on every send form.
	 *
	 * A row and not a card: the fee is a fact about the transfer, and the only
	 * thing there is to DO with it is change which token pays it — which is
	 * what the chevron opens. The SPEC sheet is explicit that the tier picker
	 * does not live here: the fee is shown, not chosen.
	 */
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import TokenIcon from '$lib/wallet/ui/TokenIcon.svelte';
	import type { FeeRowModel } from '../model';

	interface Props {
		fee: FeeRowModel;
		onopen?: () => void;
	}

	let { fee, onopen }: Props = $props();
</script>

<button type="button" class="row" aria-label={fee.openLabel} onclick={onopen}>
	<span class="label">{fee.label}</span>
	<TokenIcon
		ticker={fee.mark.ticker}
		badgeColor={fee.mark.badgeColor}
		logoUrls={fee.mark.logoUrls}
		badgeLogoUrl={fee.mark.badgeLogoUrl}
		badgeHidden={fee.mark.badgeHidden}
		size="inline"
	/>
	<span class="value">{fee.value}</span>
	<Icon icon={UTILITY_ICONS['chevron-right']} size="sm" />
</button>

<style>
	.row {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		width: 100%;
		padding: var(--space-lg);
		border: none;
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
		font-family: var(--font-ui);
		color: var(--color-fg-muted);
		text-align: start;
		cursor: pointer;
	}

	.label {
		flex: 1;
		min-width: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.value {
		font-family: var(--font-numeric);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-variant-numeric: tabular-nums;
		color: var(--color-fg-base);
	}
</style>
