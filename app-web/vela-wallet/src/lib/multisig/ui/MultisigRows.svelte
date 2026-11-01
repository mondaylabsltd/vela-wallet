<script lang="ts">
	/**
	 * The multisigs on this device, as rows of the account switcher's own list
	 * — a multisig is an account, and its row is drawn as a Vela account's:
	 * name, address, total — and its rule chip ("Needs 2 of 4"), the core's
	 * `AccountRow.rule` (r9-14, r11-5), marked last known where no chain said
	 * it this session. `<li>`s, drawn inside `AccountsSheetBody`'s list and
	 * styled as its rows are.
	 */
	import { onMount } from 'svelte';
	import Button from '$lib/ui/Button.svelte';
	import { presence, waitingWords } from '../core/presence.svelte';
	import { fillTemplate } from '$lib/i18n/fill';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import Identicon from '$lib/wallet/ui/Identicon.svelte';
	import SkeletonRow from '$lib/wallet/ui/SkeletonRow.svelte';
	import { identiconSvgForClient } from '$lib/wallet/identicon';
	import type { MultisigMessages } from '../messages';
	import { multisigName, shortenAddress } from '../core/display';
	import { localOwners } from '../core/owners';
	import { membersAnywhere } from '../core/proposals.svelte';
	import { ruleChip } from '../core/shown';
	import { multisig, type MultisigAccount } from '../core/store.svelte';

	interface Props {
		m: MultisigMessages;
		removeLabel: string;
		/** r4D-12: the ✕'s name, saying which multisig (`{{name}}`). */
		removeNamed?: string;
		cancelLabel: string;
		/** Its total as the Vela rows draw theirs: the mask, the figure, or nothing yet. */
		amount?: (address: string) => string;
		/** ms-24: no figure yet, and it is being read — drawn as loading, never "$0.00". */
		loading?: (address: string) => boolean;
		onselect: (account: MultisigAccount) => void;
	}

	let { m, removeLabel, removeNamed, cancelLabel, amount, loading, onselect }: Props = $props();

	let removing = $state<string | null>(null);
	const active = $derived(multisig.active);

	/** "1 needs your approval · 1 ready to execute", or nothing (none, or not read yet). */
	function waitingLine(address: string): string {
		return waitingWords(presence.parts(address), m);
	}

	// The rows are on screen: the multisigs not shown are read now, and again a while later.
	onMount(() => {
		void presence.refreshOthers();
	});
</script>

{#each multisig.accounts as account (account.address)}
	{@const selected = active?.address === account.address}
	{@const row = multisig.row(account.address)}
	{@const rule = ruleChip(account, m)}
	<li class="account">
		<!-- A multisig's mark, as the header draws it: a small disc on the
		     artwork's lower edge. -->
		<span class="avatar">
			<Identicon
				svg={identiconSvgForClient(account.address)}
				size="row"
				label={account.address}
				address={account.address}
			/>
			<span class="members" aria-hidden="true"
				><Icon icon={UTILITY_ICONS['users-round']} size="xs" /></span
			>
		</span>
		<button
			type="button"
			class="row"
			aria-current={selected ? 'true' : undefined}
			onclick={() => onselect(account)}
		>
			<span class="text">
				<span class="name" class:active={selected}>{multisigName(account, m)}</span>
				<span class="address"
					>{shortenAddress(account.address)}<span
						class="rule"
						data-testid="row-rule"
						title={rule.title}
					>
						· {rule.label}{rule.lastKnown ? ` · ${rule.lastKnown}` : ''}</span
					>{#if row ? row.refused_on.length > 0 : account.findings?.some((f) => f.from === 'genesis')}<span
							class="flag risk"
							data-testid="flag-refused"
						>
							· {m.authority.refusedFlag}</span
						>{:else if row ? row.high_risk_on.length > 0 : account.findings?.length}<span
							class="flag risk"
							data-testid="flag-risk"
						>
							· {m.authority.riskFlag}</span
						>{:else if row?.shared === false}<span class="flag">
							· {m.switcher.notShared}</span
						>{:else if localOwners(membersAnywhere(account)).length === 0}<span class="flag">
							· {m.switcher.readOnly}</span
						>{/if}</span
				>
				{#if waitingLine(account.address)}
					<!-- r4D-7: what waits here for this device, on the row itself. -->
					<span class="waiting" data-testid="row-waiting">{waitingLine(account.address)}</span>
				{/if}
			</span>
			{#if loading?.(account.address)}
				<span class="amount"><SkeletonRow kind="figure" /></span>
			{:else if amount}
				<span class="amount">{amount(account.address)}</span>
			{/if}
			{#if selected}
				<span class="check"><Icon icon={UTILITY_ICONS.check} size="md" /></span>
			{/if}
		</button>
		<button
			type="button"
			class="remove"
			aria-label={removeNamed
				? fillTemplate(removeNamed, { name: multisigName(account, m) })
				: removeLabel}
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
		color: var(--color-fg-base);
	}

	.address {
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.waiting {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	/* What this row's account cannot do yet, said where it is picked. */
	.flag,
	.rule {
		font-family: var(--font-ui);
		color: var(--color-fg-muted);
	}

	.flag.risk {
		color: var(--color-error-base);
	}

	.avatar {
		position: relative;
		display: inline-flex;
		flex: none;
	}

	/* A multisig's mark: a small soft disc tucked on the artwork's lower
	   edge, set off by a ring of the surface it sits on rather than an
	   outline — it says "members" without covering the art. */
	.members {
		position: absolute;
		inset-inline-end: calc(var(--space-sm) * -1);
		inset-block-end: calc(var(--space-sm) * -1);
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--icon-sm);
		height: var(--icon-sm);
		border-radius: var(--radius-full);
		background: var(--color-bg-sunken);
		box-shadow: 0 0 0 var(--border-emphasis) var(--color-bg-raised);
		color: var(--color-fg-muted);
	}

	.members :global(svg) {
		width: calc(var(--icon-xs) * 0.75);
		height: calc(var(--icon-xs) * 0.75);
		stroke-width: var(--icon-stroke-bold);
	}

	/* The total, drawn as a Vela row draws its own. */
	.amount {
		flex: none;
		white-space: nowrap;
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		color: var(--color-fg-base);
		font-variant-numeric: tabular-nums;
	}

	.check {
		display: flex;
		color: var(--color-fg-base);
	}

	/* r4D-12: never drawn smaller than the target floor (--size-targetMin). */
	.remove {
		flex: none;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-width: var(--size-targetMin);
		min-height: var(--size-targetMin);
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
