<script lang="ts">
	/**
	 * The total line above the fee (spec 021 component 25): SD2b's
	 * "Total · 3 recipients — 120 USDT · ≈$120.00", SD2d's equivalent.
	 *
	 * Deliberately not a `FactRow`: that row is a fact ABOUT the transaction
	 * inside a card, and this is a running sum of what the form above it
	 * currently says, sitting between the form and the fee.
	 *
	 * The sum in the display currency sits UNDER the figure, as it does under
	 * every other amount in the app, rather than after it on one line — where
	 * a long total pushed it off a phone. `over` is the core saying the sum is
	 * more than the account holds: the figure takes the refusal colour while
	 * the rows are still being typed, not after Continue has been refused.
	 */
	interface Props {
		label: string;
		value: string;
		detail?: string;
		over?: boolean;
		/** "2.25 ETH left" — under the label, where the eye starts the line. */
		remaining?: string;
	}

	let { label, value, detail, over = false, remaining }: Props = $props();
</script>

<p class="summary" class:over>
	<span class="lead">
		<span class="label">{label}</span>
		{#if remaining !== undefined}<span class="detail">{remaining}</span>{/if}
	</span>
	<span class="figures">
		<span class="value">{value}</span>
		{#if detail !== undefined}<span class="detail">{detail}</span>{/if}
	</span>
</p>

<style>
	.summary {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-xs) var(--space-lg);
		margin: 0;
		padding-block: var(--space-md);
	}

	.label {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.lead {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	.figures {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		gap: var(--space-xs);
		margin-inline-start: auto;
		text-align: end;
	}

	.value {
		font-family: var(--font-numeric);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		font-variant-numeric: tabular-nums;
		color: var(--color-fg-base);
	}

	.over .value {
		color: var(--color-error-base);
	}

	.detail {
		font-family: var(--font-numeric);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		font-variant-numeric: tabular-nums;
		color: var(--color-fg-muted);
	}
</style>
