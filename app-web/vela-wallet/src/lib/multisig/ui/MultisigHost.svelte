<script lang="ts">
	/**
	 * The multisig side of the wallet page: boots the store, follows the
	 * active multisig's proposals, answers the two links (`#ms=` opens a
	 * transaction to approve, `#msa=` adds a multisig) and draws whichever
	 * multisig sheet is open. The page stays the session's; this only adds.
	 *
	 * ms-21: a `#ms=` link on a device with none of the multisig's members
	 * adds nothing and switches nothing — its transaction is shown, read
	 * only, and following it here is the person's tap.
	 */
	import { replaceUrl } from '$lib/ui/history';
	import { onMount } from 'svelte';
	import type { SigningMessages } from '$lib/signing/messages';
	import type { MultisigMessages } from '../messages';
	import { checkRecords, followUnrecorded, keepMultisig } from '../core/accounts';
	import { openProposalLink } from '../core/linked.svelte';
	import { brokenLink, parseAddLink, parseProposalLink, type ProposalLink } from '../core/link';
	import { chainMeta } from '$lib/services/chains';
	import { flushOutbox, proposals } from '../core/proposals.svelte';
	import { recordExecution, useCancelTitle } from '../core/records';
	import { multisig, type MultisigAccount } from '../core/store.svelte';
	import { multisigUi } from '../core/ui.svelte';
	import { externalSigning } from '../external/signing.svelte';
	import AddMultisigSheet from './AddMultisigSheet.svelte';
	import CreateMultisigSheet from './CreateMultisigSheet.svelte';
	import InviteSheet from './InviteSheet.svelte';
	import ProposalSheet from './ProposalSheet.svelte';
	import Sheet from './Sheet.svelte';

	interface Props {
		m: MultisigMessages;
		/** The signing sheet's words: a proposal's call reads as that sheet reads it (cs-12). */
		signing: SigningMessages;
		locale: string;
		wide: boolean;
		closeLabel: string;
		/** The Vela account the session shows (a member from the start of a creation). */
		current: string | null;
		/** Something moved on chain from here: balances and activity are stale. */
		onchanged?: () => void;
	}

	let { m, signing, locale, wide, closeLabel, current, onchanged }: Props = $props();

	let addPrefill = $state('');
	/**
	 * ms-22: a link's transaction, waiting for its multisig to be added (the
	 * service could not find it at once): opened once it is.
	 */
	let pendingLink: ProposalLink | null = null;

	/**
	 * The outside wallet's sheet, fetched the first time an approval is asked
	 * of one (perf-3). It carries PeerConnect, the room transport: 79 KB of
	 * gzip, its wasm inlined as base64 and compiled on the main thread the
	 * moment it is imported — on every page this host is on, for a sheet most
	 * people never open. A sheet that cannot be fetched (offline) ends the
	 * request with the error rather than leaving the caller waiting.
	 */
	let ExternalSignSheet = $state<typeof import('./ExternalSignSheet.svelte').default | null>(null);
	$effect(() => {
		const request = externalSigning.request;
		if (request === null || ExternalSignSheet !== null) return;
		import('./ExternalSignSheet.svelte').then(
			(module) => (ExternalSignSheet = module.default),
			(error: unknown) => request.reject(error)
		);
	});

	/** ms-21: the linked multisig was added here, on the person's tap. */
	let watched = $state(false);

	onMount(() => {
		multisig.boot();
		// ms-9: an executed cancellation's Activity row is titled as its proposal.
		useCancelTitle(m.action.cancel);
		proposals.onrecorded = () => onchanged?.();
		// ms-19: what was signed and never reached the mailbox goes now.
		void flushOutbox().then((sent) => {
			const shown = multisig.active;
			if (shown && sent.some((safe) => safe.toLowerCase() === shown.address.toLowerCase())) {
				void proposals.refresh(shown, shown.chainIds);
			}
		});
		// A record that has not landed is asked about again while any page is up.
		const stopRecords = followUnrecorded();
		// What each shared multisig's record is now (registry v4: its name may
		// have changed; the service may have no record of an older registry's).
		void checkRecords();
		void answerLink();
		const onhash = () => void answerLink();
		window.addEventListener('hashchange', onhash);
		return () => {
			stopRecords();
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
		replaceUrl(location.pathname + location.search);
	}

	/** A link someone sent: a transaction to approve, or a multisig to add. */
	async function answerLink() {
		const broken = brokenLink(location.hash);
		if (broken) {
			// ms-22: a damaged link is said, not ignored. A broken add link
			// shows what it held in the add sheet, where the address rule says
			// what is wrong with it.
			const raw = decodeURIComponent(location.hash.replace(/^#(ms|msa)=/, ''));
			clearHash();
			if (broken === 'add') {
				addPrefill = raw;
				multisigUi.open({ kind: 'add' });
			} else multisigUi.open({ kind: 'notice', text: m.proposal.badLink });
			return;
		}
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
		// ms-22: a network the wallet does not have is said in the sheet; the
		// multisig is not added for it, nor the network taken into its list.
		if (!chainMeta(proposal.chainId)) {
			multisigUi.open({
				kind: 'notice',
				text: m.proposal.unknownNetwork.replace('{{chainId}}', String(proposal.chainId))
			});
			return;
		}
		// Reading the multisig may take a while (ms-16): said, never a blank wait.
		multisigUi.open({ kind: 'notice', text: m.proposal.opening });
		const opening = await openProposalLink(proposal);
		// Closed meanwhile: the person left it; nothing opens over what they do now.
		const sheetNow = multisigUi.sheet;
		if (sheetNow?.kind !== 'notice' || sheetNow.text !== m.proposal.opening) return;
		if (opening.kind === 'failed') {
			// ms-22: the person adds it themselves; its transaction opens then.
			addPrefill = proposal.multisig;
			pendingLink = proposal;
			multisigUi.open({ kind: 'add' });
			return;
		}
		if (opening.kind === 'member') {
			// A member is here: kept (if new) and shown, as a member's link always was.
			const account = opening.added ? opening.account : keepMultisig(opening.account);
			openLinked(account.address, proposal);
			return;
		}
		// ms-21: none is — the transaction alone. Nothing added, nothing switched.
		watched = opening.added;
		multisigUi.open({
			kind: 'preview',
			chainId: proposal.chainId,
			safeTxHash: proposal.safeTxHash,
			source: opening.source
		});
	}

	/** ms-21: the person chose to follow the linked multisig here. It is not shown. */
	function watch(account: MultisigAccount) {
		keepMultisig(account);
		watched = true;
	}

	/** The link's transaction, on its multisig (now on this device). */
	function openLinked(address: string, proposal: ProposalLink) {
		pendingLink = null;
		const account = multisig.find(address);
		if (!account) return;
		multisig.activate(account.address);
		multisig.useChain(account.address, proposal.chainId);
		multisigUi.open({
			kind: 'proposal',
			chainId: proposal.chainId,
			safeTxHash: proposal.safeTxHash
		});
		const fresh = multisig.find(account.address);
		if (fresh) void proposals.refresh(fresh, fresh.chainIds);
	}

	const sheet = $derived(multisigUi.sheet);
	const active = $derived(multisig.active);
	/** The multisig a dApp's request is for: the one it was granted, shown or not. */
	const dappAccount = $derived(
		sheet?.kind === 'dapp' ? (multisig.find(sheet.request.address) ?? null) : null
	);
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
			pendingLink = null;
			multisigUi.close();
		}}
		onadded={(address) => {
			addPrefill = '';
			// ms-22: added from a link's transaction: that transaction opens now.
			const linked = pendingLink;
			if (linked && linked.multisig.toLowerCase() === address.toLowerCase()) {
				openLinked(address, linked);
			} else multisigUi.close();
		}}
	/>
{:else if sheet?.kind === 'proposal' && active}
	<ProposalSheet
		{m}
		{signing}
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
{:else if sheet?.kind === 'preview'}
	<!-- ms-21: a link's transaction on a device with none of its members. -->
	{@const linked = sheet.source.account}
	<ProposalSheet
		{m}
		{signing}
		account={linked}
		chainId={sheet.chainId}
		safeTxHash={sheet.safeTxHash}
		source={sheet.source}
		watch={{ added: watched, onwatch: () => watch(linked) }}
		{locale}
		{wide}
		{closeLabel}
		onclose={() => multisigUi.close()}
		onexecuted={({ item, txHash, userOpHash }) => {
			void recordExecution(linked.address, item, txHash, userOpHash).finally(() => onchanged?.());
		}}
	/>
{:else if sheet?.kind === 'dapp' && dappAccount}
	<!-- A dApp's transaction: its review first, then the transaction it became.
	     Every way out is the request's to answer. -->
	{#key sheet.request.rid}
		<ProposalSheet
			{signing}
			{m}
			account={dappAccount}
			chainId={sheet.request.chainId}
			safeTxHash={sheet.request.safeTxHash ?? ''}
			draft={sheet.request}
			{locale}
			{wide}
			{closeLabel}
			onclose={() => sheet.request.close()}
			onexecuted={({ item, txHash, userOpHash }) => {
				void recordExecution(dappAccount.address, item, txHash, userOpHash).finally(() =>
					onchanged?.()
				);
			}}
		/>
	{/key}
{:else if sheet?.kind === 'notice'}
	<Sheet title={m.proposal.title} {closeLabel} {wide} onclose={() => multisigUi.close()}>
		<p class="notice" role="alert">{sheet.text}</p>
	</Sheet>
{:else if sheet?.kind === 'invite' && active}
	<InviteSheet
		{m}
		account={active}
		{locale}
		{wide}
		{closeLabel}
		onclose={() => multisigUi.close()}
	/>
{/if}

<!-- An outside owner's wallet, asked from any sheet or from the send flow: on top of them. -->
{#if externalSigning.request && ExternalSignSheet}
	{#key externalSigning.request}
		<ExternalSignSheet
			{m}
			{signing}
			{locale}
			request={externalSigning.request}
			{wide}
			{closeLabel}
		/>
	{/key}
{/if}

<style>
	.notice {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}
</style>
