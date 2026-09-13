<script lang="ts">
	/**
	 * ST2 / DST1 — the account switcher. Each row is an identicon, a name, a
	 * truncated address and that account's balance; the active one is checked
	 * and named in accent. Two buttons close it: create, or sign in to one you
	 * already have.
	 *
	 * The artwork sits BESIDE the row's button, not inside it: it opens the
	 * identicon viewer on that account's address (founder call, 2026-09-05),
	 * and a button inside a button is invalid HTML.
	 */
	import type { AccountsSheetModel } from '../model';
	import Button from '$lib/ui/Button.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import Identicon from '$lib/wallet/ui/Identicon.svelte';

	interface Props {
		sheet: AccountsSheetModel;
		/** Desktop lays the two buttons side by side; the phone stacks them. */
		layout?: 'stacked' | 'inline';
		onselect?: (index: number) => void;
		oncreate?: () => void;
		onsignin?: () => void;
	}

	let { sheet, layout = 'stacked', onselect, oncreate, onsignin }: Props = $props();
</script>

<p class="summary">{sheet.summary}</p>

<ul class="accounts">
	{#each sheet.rows as row, i (row.addressDisplay)}
		<li class="account">
			<Identicon svg={row.identiconSvg} size="row" label={row.name} address={row.addressFull} />
			<button
				type="button"
				aria-current={row.selected ? 'true' : undefined}
				onclick={() => onselect?.(i)}
			>
				<span class="text">
					<span class="name" class:active={row.selected}>{row.name}</span>
					<span class="address">{row.addressDisplay}</span>
				</span>
				<span class="amount">{row.amount}</span>
				{#if row.selected}
					<span class="check"><Icon icon={UTILITY_ICONS.check} size="md" /></span>
				{/if}
			</button>
		</li>
	{/each}
</ul>

<div class="actions {layout}">
	<Button variant="primary" shape="rounded" onclick={oncreate}>{sheet.primary}</Button>
	<Button variant="secondary" shape="rounded" onclick={onsignin}>{sheet.secondary}</Button>
</div>

<style>
	.summary {
		margin: 0 0 var(--space-lg);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.accounts {
		list-style: none;
		margin: 0 0 var(--space-3xl);
		padding: 0;
	}

	.account {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		border-bottom: var(--border-hairline) solid var(--color-border-base);
	}

	button {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		flex: 1;
		min-width: 0;
		padding-block: var(--space-lg);
		padding-inline: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		text-align: start;
		cursor: pointer;
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

	.name.active {
		color: var(--color-accent-base);
	}

	.address {
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.amount {
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		color: var(--color-fg-base);
		font-variant-numeric: tabular-nums;
	}

	.check {
		display: flex;
		color: var(--color-accent-base);
	}

	.actions {
		display: flex;
		gap: var(--space-lg);
	}

	.actions.stacked {
		flex-direction: column;
	}

	/* Button is `width: 100%` by default — right for a stacked phone sheet,
	   wrong for a desktop panel where the two sit side by side and should be
	   as wide as their labels. */
	.actions.inline > :global(*) {
		flex: 0 0 auto;
		width: auto;
		padding-inline: var(--space-4xl);
	}
</style>
