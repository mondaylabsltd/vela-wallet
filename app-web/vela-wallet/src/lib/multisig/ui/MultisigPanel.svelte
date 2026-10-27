<script lang="ts">
	/**
	 * The multisig's own strip on the home, between the header and the
	 * balance: its rule and members, and every transaction waiting — each with
	 * its progress and whose turn it is. A waiting transaction is the one thing
	 * a multisig asks of the person that a Vela account never does, so it is
	 * the first thing on the screen, not a tab away.
	 */
	import { fillTemplate } from '$lib/i18n/fill';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { identiconSvgForClient } from '$lib/wallet/identicon';
	import type { MultisigMessages } from '../messages';
	import { names } from '../core/names.svelte';
	import { actionTitle, chainName } from '../core/display';
	import { localOwners } from '../core/owners';
	import { proposals, type ProposalItem } from '../core/proposals.svelte';
	import type { MultisigAccount } from '../core/store.svelte';
	import { multisigUi } from '../core/ui.svelte';

	interface Props {
		m: MultisigMessages;
		account: MultisigAccount;
	}

	let { m, account }: Props = $props();

	/** The members now: the first chain that answered, else the genesis's. */
	const members = $derived.by(() => {
		const c = proposals.chains.find((x) => x.members.deployed) ?? proposals.chains[0];
		return c
			? { owners: c.members.owners, threshold: c.members.threshold }
			: { owners: account.owners, threshold: account.threshold };
	});
	const local = $derived(localOwners(members.owners));
	const open = $derived(proposals.open);

	let titles = $state<Record<string, string>>({});
	$effect(() => {
		for (const item of open) {
			if (titles[item.safeTxHash] !== undefined) continue;
			void actionTitle(m, item.chainId, item.review.action, item.nonce).then((title) => {
				titles = { ...titles, [item.safeTxHash]: title };
			});
		}
	});

	function statusOf(item: ProposalItem): { label: string; tone: 'turn' | 'success' | 'muted' } {
		if (item.status === 'queued') return { label: m.status.queued, tone: 'muted' };
		if (item.status === 'ready') return { label: m.status.ready, tone: 'success' };
		// This device can give the approval still missing (and, giving the last, execute).
		if (item.canApprove.length > 0) return { label: m.status.yourTurn, tone: 'turn' };
		return { label: m.status.waiting, tone: 'muted' };
	}
</script>

<section
	class="panel"
	aria-label={fillTemplate(m.panel.title, { m: members.threshold, n: members.owners.length })}
>
	<div class="rule">
		<span class="glyph"><Icon icon={UTILITY_ICONS['users-round']} size="md" /></span>
		<span class="rule-text"
			>{fillTemplate(m.panel.title, { m: members.threshold, n: members.owners.length })}</span
		>
		<button type="button" class="link" onclick={() => multisigUi.open({ kind: 'invite' })}>
			{m.panel.invite}
		</button>
	</div>

	<ul class="members" aria-label={m.panel.members}>
		{#each members.owners as owner (owner)}
			{@const mine = local.some((o) => o.address.toLowerCase() === owner.toLowerCase())}
			<li class:mine>
				<!-- eslint-disable-next-line svelte/no-at-html-tags -- the core's own SVG, from the address -->
				<span class="avatar" aria-hidden="true">{@html identiconSvgForClient(owner)}</span>
				<span class="who">{names.label(owner)}</span>
				{#if mine}<span class="here">{m.panel.thisDevice}</span>{/if}
			</li>
		{/each}
	</ul>

	{#if local.length === 0}
		<p class="note">{m.panel.readOnly}</p>
	{/if}
	{#if !account.recorded && !account.shared}
		<p class="note">{m.panel.notShared}</p>
	{/if}

	{#if open.length > 0}
		<h3>{m.panel.pending}</h3>
		<ul class="pending">
			{#each open as item (item.safeTxHash)}
				{@const status = statusOf(item)}
				<li>
					<button
						type="button"
						onclick={() =>
							multisigUi.open({
								kind: 'proposal',
								chainId: item.chainId,
								safeTxHash: item.safeTxHash
							})}
					>
						<span class="what">
							<span class="title">{titles[item.safeTxHash] ?? '…'}</span>
							<span class="meta">{chainName(item.chainId)} · #{item.nonce.toString()}</span>
						</span>
						<span
							class="progress"
							aria-label={fillTemplate(m.status.progress, {
								approved: item.approved,
								threshold: item.threshold
							})}
						>
							<span class="bar"
								><span
									class="fill"
									style:width="{Math.min(
										100,
										(item.approved / Math.max(1, item.threshold)) * 100
									)}%"
								></span></span
							>
							<span class="count">{item.approved}/{item.threshold}</span>
						</span>
						<span class="chip {status.tone}">{status.label}</span>
						<span class="chev"><Icon icon={UTILITY_ICONS['chevron-right']} size="sm" /></span>
					</button>
				</li>
			{/each}
		</ul>
	{/if}
	{#if proposals.stale}
		<p class="note subtle">{m.panel.stale}</p>
	{/if}
</section>

<style>
	.panel {
		padding: var(--space-lg);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-xl);
		background: var(--color-bg-raised);
	}

	.rule {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
	}

	.glyph {
		display: flex;
		color: var(--color-fg-muted);
	}

	.rule-text {
		flex: 1;
		min-width: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.link {
		flex: none;
		padding: var(--space-xs) 0;
		border: 0;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		text-decoration: underline;
		text-underline-offset: 0.2em;
		cursor: pointer;
	}

	.members {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-sm);
		list-style: none;
		margin: var(--space-md) 0 0;
		padding: 0;
	}

	.members li {
		display: inline-flex;
		align-items: center;
		gap: var(--space-xs);
		padding: var(--space-xs) var(--space-sm) var(--space-xs) var(--space-xs);
		border-radius: var(--radius-full);
		background: var(--color-bg-sunken);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	.avatar {
		display: inline-flex;
		width: var(--icon-lg);
		height: var(--icon-lg);
		border-radius: var(--radius-full);
		overflow: hidden;
	}

	.avatar :global(svg) {
		width: 100%;
		height: 100%;
	}

	.here {
		color: var(--color-fg-subtle);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
	}

	.note {
		margin: var(--space-md) 0 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.note.subtle {
		color: var(--color-fg-subtle);
	}

	h3 {
		margin: var(--space-lg) 0 var(--space-xs);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		letter-spacing: var(--letterSpacing-sectionLabel);
		text-transform: uppercase;
		color: var(--color-fg-subtle);
	}

	.pending {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	.pending li + li {
		border-top: var(--border-hairline) solid var(--color-border-base);
	}

	.pending button {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		width: 100%;
		padding: var(--space-md) 0;
		border: 0;
		background: none;
		font-family: var(--font-ui);
		text-align: start;
		cursor: pointer;
	}

	.what {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		flex: 1;
		min-width: 0;
	}

	.title {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-medium);
		color: var(--color-fg-base);
	}

	.meta {
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.progress {
		display: flex;
		align-items: center;
		gap: var(--space-xs);
		flex: none;
	}

	.bar {
		display: block;
		width: var(--space-4xl);
		height: var(--space-xs);
		border-radius: var(--radius-full);
		background: var(--color-bg-sunken);
		overflow: hidden;
	}

	.fill {
		display: block;
		height: 100%;
		background: var(--color-success-base);
	}

	.count {
		font-family: var(--font-numeric);
		font-variant-numeric: tabular-nums;
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.chip {
		flex: none;
		padding: var(--space-xs) var(--space-sm);
		border-radius: var(--radius-full);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		font-weight: var(--weight-medium);
	}

	/* Not the accent: that is the value-moving action's alone. */
	.chip.turn {
		background: var(--color-info-soft);
		color: var(--color-info-base);
	}

	.chip.success {
		background: var(--color-success-soft);
		color: var(--color-success-base);
	}

	.chip.muted {
		background: var(--color-bg-sunken);
		color: var(--color-fg-muted);
	}

	.chev {
		display: flex;
		color: var(--color-fg-subtle);
	}
</style>
