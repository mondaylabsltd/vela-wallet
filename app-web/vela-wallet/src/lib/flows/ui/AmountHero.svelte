<script lang="ts">
	/**
	 * The big signed amount (spec 021 component 19): A2's and A3's transaction
	 * figure, T2's balance, SD3's confirmation total.
	 *
	 * Money in is green; money out is plain ink, not red. Red in this product
	 * means something went wrong, and a transfer you chose to make did not.
	 */
	interface Props {
		amount: string;
		fiat: string;
		positive?: boolean;
		/** Spec 093: an allowance with no limit — the one figure drawn in red. */
		danger?: boolean;
		/** Spec 093 / 083 F1: what came back, under what left. */
		received?: string;
		align?: 'start' | 'centre';
	}

	let {
		amount,
		fiat,
		positive = false,
		danger = false,
		received,
		align = 'start'
	}: Props = $props();
</script>

<div class="hero {align}">
	<p class="amount" class:positive class:danger>{amount}</p>
	{#if received !== undefined}
		<p class="received">{received}</p>
	{/if}
	<p class="fiat">{fiat}</p>
</div>

<style>
	.hero {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		padding-block: var(--space-lg) var(--space-xl);
	}

	.centre {
		align-items: center;
		text-align: center;
	}

	p {
		margin: 0;
	}

	.amount {
		font-family: var(--font-numeric);
		font-size: calc(var(--text-4xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		font-variant-numeric: tabular-nums;
		line-height: var(--leading-tight);
		color: var(--color-fg-base);
	}

	.positive {
		color: var(--color-success-base);
	}

	.danger {
		color: var(--color-error-base);
	}

	.received {
		font-family: var(--font-numeric);
		font-size: calc(var(--text-xl) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		font-variant-numeric: tabular-nums;
		color: var(--color-success-base);
	}

	.fiat {
		font-family: var(--font-numeric);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-variant-numeric: tabular-nums;
		color: var(--color-fg-subtle);
	}
</style>
