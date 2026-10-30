<script lang="ts">
	/**
	 * The multisig's own lines at the top of its home:
	 *
	 * - r4D-7: what waits here for this device — "1 needs your approval ·
	 *   1 ready to execute ›", the first of them a tap away;
	 * - r4D-8: once created on this device, its first steps — invite the
	 *   members, add funds, and what its first transfer out costs (it also
	 *   sets the multisig up on that network) — until the person dismisses
	 *   them, or it is set up there.
	 *
	 * While it is on screen, the multisigs not shown are read now and then
	 * (`presence.follow`), for the switcher's counts and its button's dot.
	 */
	import { onMount } from 'svelte';
	import Button from '$lib/ui/Button.svelte';
	import { fillTemplate } from '$lib/i18n/fill';
	import { chainName } from '$lib/services/networks';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import type { MultisigMessages } from '../messages';
	import { presence, waitingWords } from '../core/presence.svelte';
	import { proposals } from '../core/proposals.svelte';
	import { multisig } from '../core/store.svelte';
	import { multisigUi } from '../core/ui.svelte';
	import { balance } from '$lib/wallet/core/balance.svelte';

	interface Props {
		m: MultisigMessages;
		/** Add funds: the receive card, for the multisig shown. */
		onfund?: () => void;
	}

	let { m, onfund }: Props = $props();

	onMount(() => presence.follow());

	const active = $derived(multisig.active);
	const waiting = $derived(active ? presence.of(active.address) : null);
	const words = $derived(waiting ? waitingWords(waiting, m) : '');
	const network = $derived(active?.chainIds[0] ?? null);
	/** Set up on its first network already (read from the chain): the first steps are behind it. */
	const setUp = $derived(
		!!active &&
			proposals.followed?.toLowerCase() === active.address.toLowerCase() &&
			proposals.chains.some((c) => c.members.deployed)
	);
	const firstRun = $derived(!!active?.firstRun && !setUp);
	/**
	 * S4 (r7): a step done is ticked — funds are in once the multisig holds
	 * something (not spam) on its first network, as the wallet's balance read
	 * it for the address shown.
	 */
	const funded = $derived(
		!!active &&
			network !== null &&
			balance.view.address.toLowerCase() === active.address.toLowerCase() &&
			balance.view.tokens.some(
				(t) => t.chain_id === network && !t.spam && /[1-9]/.test(t.balance)
			)
	);
	/** r5reg-18: a multisig of one has no members to invite. */
	const solo = $derived(!!active && active.owners.length === 1);
	// r5reg-17: once it is set up (deployed — funded and used), its first
	// steps are behind it for good: not back on the next page before the
	// chain is read again.
	$effect(() => {
		if (setUp && active?.firstRun) multisig.setFirstRun(active.address, false);
	});
</script>

{#if active && words && waiting?.first}
	{@const first = waiting.first}
	<button
		type="button"
		class="strip"
		data-testid="multisig-waiting"
		onclick={() => multisigUi.open({ kind: 'proposal', ...first })}
	>
		<span>{words}</span>
		<Icon icon={UTILITY_ICONS['chevron-right']} size="sm" />
	</button>
{/if}

{#if active && firstRun}
	<section class="first" aria-labelledby="multisig-first-run" data-testid="multisig-first-run">
		<h2 id="multisig-first-run">{m.create.firstRunTitle}</h2>
		<ol>
			{#if !solo}
				<li>
					<button type="button" class="step" onclick={() => multisigUi.open({ kind: 'invite' })}>
						<span class="icon"><Icon icon={UTILITY_ICONS['qr-code']} size="md" /></span>
						<span class="text">
							<span class="title">{m.create.firstRunInvite}</span>
							<span class="note">{m.create.firstRunInviteNote}</span>
						</span>
						<Icon icon={UTILITY_ICONS['chevron-right']} size="sm" />
					</button>
				</li>
			{/if}
			<li>
				<button
					type="button"
					class="step"
					class:done={funded}
					data-testid="first-run-fund"
					onclick={() => onfund?.()}
					disabled={!onfund}
				>
					<span class="icon"
						><Icon
							icon={funded ? UTILITY_ICONS.check : UTILITY_ICONS['arrow-down-left']}
							size="md"
						/></span
					>
					<span class="text">
						<span class="title">{m.create.firstRunFund}</span>
						{#if network !== null}<span class="note"
								>{fillTemplate(m.create.firstRunFundNote, { network: chainName(network) })}</span
							>{/if}
					</span>
					<Icon icon={UTILITY_ICONS['chevron-right']} size="sm" />
				</button>
			</li>
			{#if network !== null}
				<li class="cost">
					<span class="icon"><Icon icon={UTILITY_ICONS.info} size="md" /></span>
					<span class="note"
						>{fillTemplate(m.create.firstRunCost, { network: chainName(network) })}</span
					>
				</li>
			{/if}
		</ol>
		<Button
			variant="secondary"
			shape="rounded"
			onclick={() => multisig.setFirstRun(active.address, false)}>{m.create.firstRunDismiss}</Button
		>
	</section>
{/if}

<style>
	.strip {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-md);
		width: 100%;
		min-height: var(--size-control-md);
		margin-block-end: var(--space-md);
		padding: var(--space-sm) var(--space-lg);
		border: var(--border-hairline) solid var(--color-border-strong);
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
		text-align: start;
		cursor: pointer;
	}

	.first {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
		margin-block-end: var(--space-md);
		padding: var(--space-lg);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
	}

	h2 {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	ol {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	.step,
	.cost {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		width: 100%;
	}

	.step {
		min-height: var(--size-control-md);
		padding: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		color: var(--color-fg-base);
		text-align: start;
		cursor: pointer;
	}

	.step:disabled {
		cursor: default;
		opacity: var(--opacity-disabled);
	}

	/* A step done: ticked, its title quieter — neutral ink, never the accent. */
	.step.done .title {
		color: var(--color-fg-muted);
	}

	.icon {
		display: flex;
		align-items: center;
		justify-content: center;
		flex: none;
		width: var(--size-control-sm);
		height: var(--size-control-sm);
		border-radius: var(--radius-full);
		background: var(--color-bg-sunken);
		color: var(--color-fg-base);
	}

	.text {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		flex: 1;
		min-width: 0;
	}

	.title {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
	}

	.note {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
	}
</style>
