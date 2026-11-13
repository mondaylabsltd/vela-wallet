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
	 * - r13-1: a multisig used only in the other space says where it lives —
	 *   "Used on Arc Testnet · Testnet space" — with the switch there (the
	 *   core's `lives_elsewhere`, as the native homes say it).
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
	import { listOf } from '$lib/i18n/list';
	import { networkSpace } from '$lib/network-space/network-space.svelte';
	import type { NetworkSpaceMessages } from '$lib/network-space/messages';
	import type { Elsewhere } from '../core/elsewhere';

	interface Props {
		m: MultisigMessages;
		/** Add funds: the receive card, for the multisig shown. */
		onfund?: () => void;
		/** r13-1: the shown multisig lives in the other space (the core's word). */
		elsewhere?: Elsewhere | null;
		/** r13-1: the switch there (the `set_network_space` intent). */
		onswitch?: (space: 'mainnet' | 'testnet') => void;
		locale?: string;
	}

	let { m, onfund, elsewhere = null, onswitch, locale = 'en' }: Props = $props();

	/** The space words by the core's key (`networkSpace.headingTestnet`, …). */
	function spaceWord(key: string): string {
		const words = networkSpace.words;
		const leaf = key.startsWith('networkSpace.') ? key.slice('networkSpace.'.length) : key;
		return (words?.[leaf as keyof NetworkSpaceMessages] as string | undefined) ?? '';
	}
	const elsewhereLine = $derived(
		elsewhere
			? `${fillTemplate(m.add.usedOn, {
					networks: listOf(elsewhere.chain_ids.map(chainName), locale)
				})} · ${spaceWord(elsewhere.heading_key)}`
			: ''
	);

	onMount(() => presence.follow());

	const active = $derived(multisig.active);
	/** The proposals machine's line for the multisig it follows (the one shown): what waits, what it opens. */
	const banner = $derived(
		active && proposals.followed?.toLowerCase() === active.address.toLowerCase()
			? presence.banner
			: null
	);
	const words = $derived(banner ? waitingWords(banner.parts, m) : '');
	const network = $derived(active?.chainIds[0] ?? null);
	/** Set up on a network already (read from the chain): the first steps are behind it — the machine's `set_up`. */
	const setUp = $derived(
		!!active && proposals.followed?.toLowerCase() === active.address.toLowerCase() && presence.setUp
	);
	const firstRun = $derived(!!active?.firstRun && !setUp);
	/**
	 * S4 (r7): a step done is ticked — funds are in once the multisig holds
	 * something (not spam) on its first network, as the wallet's balance read
	 * it for the address shown.
	 */
	/** Its invite was handed on (shown, copied or shared): the accounts machine's word. */
	const invited = $derived(!!active && (multisig.row(active.address)?.invite_shared ?? false));
	const funded = $derived(
		!!active &&
			network !== null &&
			balance.view.address?.toLowerCase() === active.address.toLowerCase() &&
			balance.view.tokens.some((t) => t.chain_id === network && !t.spam && /[1-9]/.test(t.balance))
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

{#if active && elsewhere}
	<!-- r13-1: where it lives, and the way there — never a confident zero here. -->
	<div class="elsewhere" data-testid="multisig-other-space" role="status">
		<p class="elsewhere-line">{elsewhereLine}</p>
		<Button
			variant="secondary"
			shape="rounded"
			testid="multisig-other-space-switch"
			onclick={() => elsewhere && onswitch?.(elsewhere.space)}
			>{spaceWord(elsewhere.switch_key)}</Button
		>
	</div>
{/if}

{#if active && words && banner}
	{@const first = banner.first}
	<button
		type="button"
		class="strip"
		data-testid="multisig-waiting"
		onclick={() =>
			multisigUi.open({
				kind: 'proposal',
				chainId: first.chain_id,
				safeTxHash: first.safe_tx_hash
			})}
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
					<button
						type="button"
						class="step"
						class:done={invited}
						data-testid="first-run-invite"
						onclick={() => multisigUi.open({ kind: 'invite' })}
					>
						<span class="icon"
							><Icon
								icon={invited ? UTILITY_ICONS.check : UTILITY_ICONS['qr-code']}
								size="md"
							/></span
						>
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
	.elsewhere {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--space-sm);
		margin: 0 0 var(--space-md);
		padding: var(--space-md) var(--space-lg);
		border: var(--border-hairline) solid var(--color-border-strong);
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
	}
	.elsewhere-line {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-base);
	}
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
