<script lang="ts">
	/**
	 * The multisigs on this device, as rows of the account switcher's own list
	 * — a multisig is an account; the one thing its row adds is its rule
	 * ("2/3"). `<li>`s, drawn inside `AccountsSheetBody`'s list and styled as
	 * its rows are.
	 */
	import Button from '$lib/ui/Button.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import Identicon from '$lib/wallet/ui/Identicon.svelte';
	import { identiconSvgForClient } from '$lib/wallet/identicon';
	import { shortenAddress } from '$lib/wallet/identity';
	import { fillTemplate } from '$lib/i18n/fill';
	import type { MultisigMessages } from '../messages';
	import { multisigName } from '../core/display';
	import { multisig, type MultisigAccount } from '../core/store.svelte';

	interface Props {
		m: MultisigMessages;
		removeLabel: string;
		cancelLabel: string;
		onselect: (account: MultisigAccount) => void;
	}

	let { m, removeLabel, cancelLabel, onselect }: Props = $props();

	let removing = $state<string | null>(null);
	const active = $derived(multisig.active);
</script>

{#each multisig.accounts as account (account.address)}
	{@const selected = active?.address === account.address}
	<li class="account">
		<Identicon
			svg={identiconSvgForClient(account.address)}
			size="row"
			label={account.address}
			address={account.address}
		/>
		<button
			type="button"
			class="row"
			aria-current={selected ? 'true' : undefined}
			onclick={() => onselect(account)}
		>
			<span class="text">
				<span class="name" class:active={selected}>{multisigName(account, m)}</span>
				<span class="address">{shortenAddress(account.address)}</span>
			</span>
			<span
				class="rule"
				title={fillTemplate(m.panel.title, { m: account.threshold, n: account.owners.length })}
				><Icon icon={UTILITY_ICONS['users-round']} size="sm" />{account.threshold}/{account.owners
					.length}</span
			>
			{#if selected}
				<span class="check"><Icon icon={UTILITY_ICONS.check} size="md" /></span>
			{/if}
		</button>
		<button
			type="button"
			class="remove"
			aria-label={removeLabel}
			onclick={() => (removing = account.address)}
		>
			<Icon icon={UTILITY_ICONS['x']} size="md" />
		</button>
	</li>
	{#if removing === account.address}
		<li class="confirm">
			<p>{m.switcher.removeBody}</p>
			<div class="confirm-actions">
				<Button
					variant="danger"
					shape="rounded"
					onclick={() => {
						removing = null;
						multisig.remove(account.address);
					}}>{removeLabel}</Button
				>
				<Button variant="secondary" shape="rounded" onclick={() => (removing = null)}
					>{cancelLabel}</Button
				>
			</div>
		</li>
	{/if}
{/each}

<style>
	.account {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		border-bottom: var(--border-hairline) solid var(--color-border-base);
	}

	.row {
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

	.name,
	.address {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
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

	.rule {
		display: inline-flex;
		align-items: center;
		gap: var(--space-xs);
		flex: none;
		padding: 0 var(--space-sm);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-full);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-muted);
	}

	.check {
		display: flex;
		color: var(--color-accent-base);
	}

	.remove {
		flex: none;
		padding: var(--space-xs);
		border: 0;
		background: none;
		color: var(--color-fg-subtle);
		cursor: pointer;
	}

	.remove:hover {
		color: var(--color-fg-base);
	}

	.confirm {
		list-style: none;
		padding: var(--space-md) 0 var(--space-lg);
	}

	.confirm p {
		margin: 0 0 var(--space-md);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.confirm-actions {
		display: flex;
		gap: var(--space-sm);
	}
</style>
