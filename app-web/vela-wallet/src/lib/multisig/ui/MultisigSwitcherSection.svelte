<script lang="ts">
	/**
	 * The switcher's multisig section: every multisig this device has added,
	 * the active one checked, and the two ways to get another — create one, or
	 * add one by its address. A multisig row reads as an account row (same
	 * identicon, same type), with its rule — "2/3" — where a Vela account has
	 * its name, since a multisig has no name of its own.
	 */
	import Button from '$lib/ui/Button.svelte';
	import { fillTemplate } from '$lib/i18n/fill';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import Identicon from '$lib/wallet/ui/Identicon.svelte';
	import { identiconSvgForClient } from '$lib/wallet/identicon';
	import { shortenAddress } from '$lib/wallet/identity';
	import type { MultisigMessages } from '../messages';
	import { localOwners } from '../core/owners';
	import { multisig, type MultisigAccount } from '../core/store.svelte';

	interface Props {
		m: MultisigMessages;
		removeLabel: string;
		cancelLabel: string;
		onselect: (account: MultisigAccount) => void;
		oncreate: () => void;
		onadd: () => void;
	}

	let { m, removeLabel, cancelLabel, onselect, oncreate, onadd }: Props = $props();

	let removing = $state<string | null>(null);
	const active = $derived(multisig.active);
</script>

<section class="multisig" aria-labelledby="multisig-section">
	<h3 id="multisig-section">{m.switcher.section}</h3>
	{#if multisig.accounts.length > 0}
		<ul>
			{#each multisig.accounts as account (account.address)}
				{@const selected = active?.address === account.address}
				{@const readOnly = localOwners(account.owners).length === 0}
				<li class="row">
					<Identicon
						svg={identiconSvgForClient(account.address)}
						size="row"
						label={account.address}
						address={account.address}
					/>
					<button
						type="button"
						aria-current={selected ? 'true' : undefined}
						onclick={() => onselect(account)}
					>
						<span class="text">
							<span class="name" class:active={selected}>
								{fillTemplate(m.switcher.rowName, {
									m: account.threshold,
									n: account.owners.length
								})}
							</span>
							<span class="address">{shortenAddress(account.address)}</span>
						</span>
						{#if readOnly}
							<span class="tag">{m.switcher.readOnly}</span>
						{/if}
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
		</ul>
	{/if}
	<div class="actions">
		<Button variant="secondary" shape="rounded" onclick={oncreate}>
			<span class="label"
				><Icon icon={UTILITY_ICONS['users-round']} size="sm" />{m.switcher.create}</span
			>
		</Button>
		<Button variant="secondary" shape="rounded" onclick={onadd}>
			<span class="label"><Icon icon={UTILITY_ICONS.plus} size="sm" />{m.switcher.add}</span>
		</Button>
	</div>
</section>

<style>
	.multisig {
		margin-block-start: var(--space-3xl);
	}

	h3 {
		margin: 0 0 var(--space-sm);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		letter-spacing: var(--letterSpacing-sectionLabel);
		text-transform: uppercase;
		color: var(--color-fg-subtle);
	}

	ul {
		list-style: none;
		margin: 0 0 var(--space-lg);
		padding: 0;
	}

	.row {
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

	.remove {
		flex: none;
		padding: var(--space-xs);
		color: var(--color-fg-subtle);
	}

	.remove:hover {
		color: var(--color-fg-base);
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

	.tag {
		flex: none;
		padding: var(--space-xs) var(--space-sm);
		border-radius: var(--radius-full);
		background: var(--color-bg-sunken);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.check {
		display: flex;
		color: var(--color-accent-base);
	}

	.confirm {
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

	.actions {
		display: flex;
		gap: var(--space-md);
	}

	.actions > :global(*) {
		flex: 1;
	}

	.label {
		display: inline-flex;
		align-items: center;
		gap: var(--space-sm);
	}
</style>
