<script lang="ts">
	/**
	 * The multisig side of the wallet page: boots the store, follows the
	 * active multisig's proposals, answers the two links (`#ms=` opens a
	 * transaction to approve, `#msa=` adds a multisig) and draws whichever
	 * multisig sheet is open. The page stays the session's; this only adds.
	 */
	import { onMount } from 'svelte';
	import type { BalanceToken } from '$lib/core/generated/BalanceToken';
	import type { MultisigMessages } from '../messages';
	import { addMultisig, retryUnrecorded } from '../core/accounts';
	import { parseAddLink, parseProposalLink } from '../core/link';
	import { proposals } from '../core/proposals.svelte';
	import { recordExecution } from '../core/records';
	import { multisig } from '../core/store.svelte';
	import { multisigUi } from '../core/ui.svelte';
	import AddMultisigSheet from './AddMultisigSheet.svelte';
	import CreateMultisigSheet from './CreateMultisigSheet.svelte';
	import InviteSheet from './InviteSheet.svelte';
	import ProposalSheet from './ProposalSheet.svelte';
	import ProposeSendSheet from './ProposeSendSheet.svelte';

	interface Props {
		m: MultisigMessages;
		locale: string;
		wide: boolean;
		closeLabel: string;
		/** The Vela account the session shows (a member from the start of a creation). */
		current: string | null;
		/** The active multisig's holdings (the balance core follows the address shown). */
		tokens: BalanceToken[];
		/** Something moved on chain from here: balances and activity are stale. */
		onchanged?: () => void;
	}

	let { m, locale, wide, closeLabel, current, tokens, onchanged }: Props = $props();

	let addPrefill = $state('');

	onMount(() => {
		multisig.boot();
		proposals.onrecorded = () => onchanged?.();
		void retryUnrecorded();
		void answerLink();
		const onhash = () => void answerLink();
		window.addEventListener('hashchange', onhash);
		return () => {
			window.removeEventListener('hashchange', onhash);
			proposals.follow(null, []);
		};
	});

	// Follow the active multisig's proposals on the networks it is used on.
	const followKey = $derived(
		multisig.active ? `${multisig.active.address}:${multisig.active.chainIds.join(',')}` : ''
	);
	$effect(() => {
		void followKey;
		const account = multisig.active;
		proposals.follow(account, account?.chainIds ?? []);
	});

	function clearHash() {
		history.replaceState(history.state, '', location.pathname + location.search);
	}

	/** A link someone sent: a transaction to approve, or a multisig to add. */
	async function answerLink() {
		const proposal = parseProposalLink(location.hash);
		const adding = parseAddLink(location.hash);
		if (!proposal && !adding) return;
		clearHash();
		if (adding) {
			addPrefill = adding;
			multisigUi.open({ kind: 'add' });
			return;
		}
		if (!proposal) return;
		let account = multisig.find(proposal.multisig);
		if (!account) {
			const outcome = await addMultisig(proposal.multisig, [proposal.chainId]);
			if (!outcome.ok) {
				addPrefill = proposal.multisig;
				multisigUi.open({ kind: 'add' });
				return;
			}
			account = outcome.account;
		}
		multisig.activate(account.address);
		multisig.useChain(account.address, proposal.chainId);
		multisigUi.open({ kind: 'proposal', chainId: proposal.chainId, safeTxHash: proposal.safeTxHash });
		const fresh = multisig.find(account.address);
		if (fresh) void proposals.refresh(fresh, fresh.chainIds);
	}

	const sheet = $derived(multisigUi.sheet);
	const active = $derived(multisig.active);
</script>

{#if sheet?.kind === 'create'}
	<CreateMultisigSheet
		{m}
		{locale}
		{current}
		{wide}
		{closeLabel}
		onclose={() => multisigUi.close()}
		oncreated={() => multisigUi.close()}
	/>
{:else if sheet?.kind === 'add'}
	<AddMultisigSheet
		{m}
		prefill={addPrefill}
		{wide}
		{closeLabel}
		onclose={() => {
			addPrefill = '';
			multisigUi.close();
		}}
		onadded={() => {
			addPrefill = '';
			multisigUi.close();
		}}
	/>
{:else if sheet?.kind === 'propose' && active}
	<ProposeSendSheet
		{m}
		account={active}
		{tokens}
		assetId={sheet.assetId}
		{wide}
		{closeLabel}
		onclose={() => multisigUi.close()}
		onproposed={(chainId, safeTxHash) => multisigUi.open({ kind: 'proposal', chainId, safeTxHash })}
	/>
{:else if sheet?.kind === 'proposal' && active}
	<ProposalSheet
		{m}
		account={active}
		chainId={sheet.chainId}
		safeTxHash={sheet.safeTxHash}
		{locale}
		{wide}
		{closeLabel}
		onclose={() => multisigUi.close()}
		onexecuted={({ item, txHash, userOpHash }) => {
			void recordExecution(active.address, item, txHash, userOpHash).finally(() => onchanged?.());
		}}
	/>
{:else if sheet?.kind === 'invite' && active}
	<InviteSheet {m} account={active} {locale} {wide} {closeLabel} onclose={() => multisigUi.close()} />
{/if}
