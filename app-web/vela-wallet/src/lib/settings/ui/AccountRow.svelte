<script lang="ts">
	/**
	 * ST1's identity block: identicon, name, truncated address, and a trailing
	 * text action rather than a bare chevron — "切换账户 ›" says what the tap
	 * does, which a chevron alone does not.
	 *
	 * The artwork is its own control beside the row's: it opens the identicon
	 * viewer on this account (founder call, 2026-09-05).
	 */
	import type { AccountRowModel } from '../model';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import Identicon from '$lib/wallet/ui/Identicon.svelte';

	interface Props {
		account: AccountRowModel;
		onselect?: () => void;
	}

	let { account, onselect }: Props = $props();
</script>

<div class="account">
	<Identicon
		svg={account.identiconSvg}
		size="row"
		label={account.name}
		address={account.addressFull}
	/>
	<button type="button" class="main" onclick={onselect}>
		<span class="text">
			<span class="name">{account.name}</span>
			<span class="address">{account.addressDisplay}</span>
		</span>
		<span class="action">{account.action}</span>
		<span class="chevron"><Icon icon={UTILITY_ICONS['chevron-right']} size="sm" /></span>
	</button>
</div>

<style>
	.account {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		width: 100%;
		border-bottom: var(--border-hairline) solid var(--color-border-base);
	}

	.main {
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
		font-size: calc(var(--text-xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.address {
		font-family: var(--font-mono);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.action {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		white-space: nowrap;
	}

	.chevron {
		display: flex;
		color: var(--color-fg-subtle);
	}
</style>
