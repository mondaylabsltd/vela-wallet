<script lang="ts">
	/**
	 * SD2f's fee-token row (spec 021 component 14): a coin that could pay this
	 * transfer's fee, what you hold of it, and what the fee would come to.
	 *
	 * The estimate is per row and not per screen because that is the whole
	 * decision: the same transfer costs a different number in each coin, and
	 * showing one figure with a token switcher would hide the comparison.
	 */
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import TokenIcon from '$lib/wallet/ui/TokenIcon.svelte';
	import type { FeeTokenPickModel } from '../model';

	interface Props {
		row: FeeTokenPickModel['rows'][number];
		estimateLabel: string;
		onselect?: () => void;
	}

	let { row, estimateLabel, onselect }: Props = $props();
</script>

<!--
	A coin that cannot cover the fee is drawn for context and answers to
	nothing — the desktop's rule (`panels.rs`), and the core's: `select_fee_asset`
	refuses such a row. Web drew it exactly like the others and dropped the tap
	on the floor, so paying gas in a coin the account did not hold looked like a
	choice that had been made (issue 211).
-->
<button
	type="button"
	class="row"
	class:on={row.selected}
	class:spent={row.insufficient}
	aria-pressed={row.selected}
	disabled={row.insufficient}
	onclick={onselect}
>
	<TokenIcon
		ticker={row.mark.ticker}
		badgeColor={row.mark.badgeColor}
		logoUrls={row.mark.logoUrls}
		badgeLogoUrl={row.mark.badgeLogoUrl}
		badgeHidden={row.mark.badgeHidden}
	/>
	<span class="text">
		<span class="symbol">{row.symbol}</span>
		<span class="balance"
			>{row.insufficient && row.insufficientNote !== undefined
				? row.insufficientNote
				: row.balanceLabel}</span
		>
	</span>
	<span class="numbers">
		<span class="fee">{row.fee}</span>
		<span class="caption">{estimateLabel}</span>
	</span>
	<span class="tick" aria-hidden="true">
		{#if row.selected}<Icon icon={UTILITY_ICONS.check} size="md" />{/if}
	</span>
</button>

<style>
	.row {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		width: 100%;
		padding: var(--space-lg);
		border: none;
		border-radius: var(--radius-lg);
		background: none;
		font-family: var(--font-ui);
		text-align: start;
		cursor: pointer;
	}

	.on {
		background: var(--color-bg-raised);
	}

	.spent {
		opacity: 0.45;
		cursor: default;
	}

	.text {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		flex: 1;
		min-width: 0;
	}

	.symbol {
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.balance {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.numbers {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		gap: var(--space-xs);
		flex-shrink: 0;
	}

	.fee {
		font-family: var(--font-numeric);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-variant-numeric: tabular-nums;
		color: var(--color-fg-base);
	}

	.caption {
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	/* Always laid out, so choosing a row does not shift the ones under it. */
	.tick {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--icon-lg);
		flex-shrink: 0;
		color: var(--color-accent-base);
	}
</style>
