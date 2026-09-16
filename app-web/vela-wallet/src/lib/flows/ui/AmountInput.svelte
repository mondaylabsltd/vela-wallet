<script lang="ts">
	/**
	 * SD2's amount (spec 021 component 8): the number, big and centred, with
	 * its fiat equivalent and the toggle that swaps which of the two you type.
	 *
	 * The figure is the largest type on the screen because it is the one thing
	 * the person came to decide. The fiat line stays subordinate even when the
	 * denominations swap — the amount being ENTERED leads, whichever it is.
	 */
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';

	interface Props {
		value: string;
		/** The OTHER denomination — money while tokens are typed, and back. */
		fiat: string;
		denomLabel: string;
		/**
		 * Present ⇒ the core offers the ⇄ swap (`denom_toggle_shown`), and
		 * `enabled` is whether pressing it would change anything
		 * (`denom_toggle_enabled` — entering fiat needs a rate). Absent, the
		 * line below the figure is text: issue 197 was this control drawn
		 * unconditionally, with nothing behind it in either direction.
		 */
		denomToggle?: { enabled: boolean };
		ondenom?: () => void;
		/**
		 * Present ⇒ the figure is TYPED here (spec 026). The gallery passes
		 * nothing and keeps the drawn picture, exactly as the balance hero's
		 * tap-to-hide does: an affordance appears only where something is
		 * listening for it.
		 */
		oninput?: (value: string) => void;
		placeholder?: string;
	}

	let { value, fiat, denomLabel, denomToggle, ondenom, oninput, placeholder }: Props = $props();
</script>

<div class="amount">
	{#if oninput}
		<input
			class="value entry"
			inputmode="decimal"
			autocomplete="off"
			aria-label={denomLabel}
			placeholder={placeholder ?? '0'}
			{value}
			oninput={(event) => oninput(event.currentTarget.value)}
		/>
	{:else}
		<p class="value">{value}</p>
	{/if}
	{#if denomToggle}
		<!-- Named by what it switches TO, which is the line it shows. The old
		     `aria-label` said the unit already being typed — a two-state
		     control announced by the state you are leaving. -->
		<button type="button" class="fiat" onclick={ondenom} disabled={!denomToggle.enabled}>
			<span>{fiat}</span>
			<Icon icon={UTILITY_ICONS['chevrons-up-down']} size="sm" />
		</button>
	{:else if fiat}
		<p class="fiat">{fiat}</p>
	{/if}
</div>

<style>
	.amount {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-sm);
		padding-block: var(--space-3xl);
	}

	.value {
		margin: 0;
		font-family: var(--font-numeric);
		font-size: calc(var(--text-hero) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		font-variant-numeric: tabular-nums;
		line-height: var(--leading-none);
		color: var(--color-fg-base);
	}

	.entry {
		width: 100%;
		border: none;
		background: none;
		text-align: center;
		padding: 0;
	}

	.entry:focus {
		outline: none;
	}

	.fiat {
		display: inline-flex;
		align-items: center;
		gap: var(--space-xs);
		margin: 0;
		padding: var(--space-xs) var(--space-sm);
		border: none;
		border-radius: var(--radius-sm);
		background: none;
		font-family: var(--font-numeric);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-variant-numeric: tabular-nums;
		color: var(--color-fg-muted);
		cursor: pointer;
	}

	.fiat:hover:not(:disabled) {
		color: var(--color-fg-base);
	}

	/* The core's refusal, made visible: no rate to enter this currency
	   against. The sentence that says WHY rides the form's one notice line
	   (`denom_toggle_reason`), the same slot the desktop puts it in. */
	.fiat:disabled {
		opacity: var(--opacity-disabled);
		cursor: default;
	}

	p.fiat {
		cursor: default;
	}
</style>
