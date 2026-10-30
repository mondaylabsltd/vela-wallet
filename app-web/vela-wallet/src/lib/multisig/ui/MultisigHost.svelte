<script lang="ts">
	/**
	 * The multisig side of the wallet page: boots the store, follows the
	 * active multisig's proposals, hands the two links (`#ms=` opens a
	 * transaction to approve, `#msa=` adds a multisig) to the accounts
	 * machine's one link flow and draws where it stands (`link`), and draws
	 * whichever multisig sheet is open. The page stays the session's; this
	 * only adds.
	 *
	 * ms-21: a `#ms=` link on a device with none of the multisig's members
	 * adds nothing and switches nothing — its transaction is shown, read
	 * only (the proposals machine's `linked`), and following it here is the
	 * person's tap.
	 */
	import { replaceUrl } from '$lib/ui/history';
	import { onMount, untrack } from 'svelte';
	import type { MultisigLinkStand } from '../generated/MultisigLinkStand';
	import type { MultisigNotice } from '../generated/MultisigNotice';
	import type { SigningMessages } from '$lib/signing/messages';
	import type { MultisigMessages } from '../messages';
	import { chainName, spaceNetworksSync } from '$lib/services/networks';
	import { isMultisigLink, type AddLink } from '../core/link';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import Button from '$lib/ui/Button.svelte';
	import { indexName, missingTitle } from '../core/activity.svelte';
	import { proposals } from '../core/proposals.svelte';
	import { proposalsHost } from '../shell/proposals-host.svelte';
	import { useRecordWords } from '../core/records';
	import { multisig, type MultisigAccount } from '../core/store.svelte';
	import { multisigUi } from '../core/ui.svelte';
	import { machines } from '../shell/machines.svelte';
	import { externalSigning } from '../external/signing.svelte';
	import { riskConfirmation, SigningBlockedError, SigningRefusedError } from '../core/gate.svelte';
	import { fillTemplate } from '$lib/i18n/fill';
	import AuthorityNotice from './AuthorityNotice.svelte';
	import AddMultisigSheet from './AddMultisigSheet.svelte';
	import CreateMultisigSheet from './CreateMultisigSheet.svelte';
	import InviteSheet from './InviteSheet.svelte';
	import ProposalSheet from './ProposalSheet.svelte';
	import RiskConfirmSheet from './RiskConfirmSheet.svelte';
	import PageHandoffSheet from '$lib/signing/ui/PageHandoffSheet.svelte';
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
		/**
		 * r4b-9: this page opens the link kept while the person had no
		 * account yet (the wallet page; not Settings, whose own wizards it
		 * would cover).
		 */
		takeKept?: boolean;
	}

	let {
		m,
		signing,
		locale,
		wide,
		closeLabel,
		current,
		onchanged,
		takeKept = false
	}: Props = $props();

	let addPrefill = $state('');
	/** r4b-13: what the link's own lookup found, said at once in the add sheet. */
	let addFailure = $state<string | null>(null);
	/** The person closed "Opening…": the link's transaction opens over nothing they do now. */
	let leftLink = false;

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
		// §3.5: a signature on a high-risk network asks here, on top of any sheet.
		const detachRisk = riskConfirmation.attach();
		// ms-9, r4a-3: an executed transaction's Activity row is titled as its proposal.
		useRecordWords(m);
		// Executed rows the proposals machine wrote to the activity: balances and activity move.
		// (ms-19: what was signed and never reached the mailbox goes when it starts.)
		proposals.onrecorded = () => onchanged?.();
		// A record that has not landed is asked about again while any page is
		// up; and, once a session, what each shared multisig's record is now
		// (registry v4: its name may have changed; the service may have no
		// record of an older registry's). The accounts machine's `tick` and
		// `check_records`.
		const stopRecords = machines.followRecords();
		// A link's transaction, opened by the accounts machine (`Notice::OpenProposal`).
		machines.onproposal = openedProposal;
		// r4b-9: a link opened before this device had an account opens now — a
		// link in the address is the one answered, one kept on the way is kept.
		void answerLink(takeKept);
		const onhash = () => void answerLink(false);
		window.addEventListener('hashchange', onhash);
		return () => {
			if (machines.onproposal === openedProposal) machines.onproposal = null;
			detachRisk();
			// §3.6: a review nobody can see any more is withdrawn — nothing is signed.
			proposalsHost.withdrawDraft();
			stopRecords();
			window.removeEventListener('hashchange', onhash);
			proposals.follow(null, []);
		};
	});

	/*
	 * §3.5: every multisig on this device is checked again — once a session,
	 * and on each network it comes to be used on (or this device comes to
	 * have): one added before the counterfactual rule was never vetted, and a
	 * deployed one can gain a module since. What is found is kept on it and
	 * shown (Settings, the switcher, its transactions); nothing is silent.
	 */
	const recheckKey = $derived(
		multisig.accounts.map((a) => `${a.address}:${a.chainIds.join(',')}`).join(';')
	);
	$effect(() => {
		if (!recheckKey) return;
		multisig.send({ type: 'recheck' });
	});

	// Follow the active multisig's proposals on the networks it is used on.
	const followKey = $derived(
		multisig.active ? `${multisig.active.address}:${multisig.active.chainIds.join(',')}` : ''
	);
	$effect(() => {
		void followKey;
		// Only the key moves this: what the machine says back (its view) does not.
		untrack(() => {
			const account = multisig.active;
			proposals.follow(account, account?.chainIds ?? []);
			// Once a session: networks another member made it used on (deployed
			// there, or proposed there) become its networks here too.
			if (account) {
				multisig.send({
					type: 'discover',
					address: account.address,
					chain_ids: spaceNetworksSync().map((n) => n.chainId)
				});
			}
		});
	});

	function clearHash() {
		replaceUrl(location.pathname + location.search);
	}

	/**
	 * A link someone sent — a transaction to approve, a multisig to add, or
	 * one damaged on the way — or (`takeKept`) the one kept while this device
	 * had no account: the accounts machine's one flow reads it, finds the
	 * multisig, keeps it only where a member is here, and says where it
	 * stands (`link`, drawn below).
	 */
	async function answerLink(takeKept: boolean) {
		const hash = location.hash;
		const meant = isMultisigLink(hash);
		if (!meant && !takeKept) return;
		if (meant) clearHash();
		leftLink = false;
		await multisig.boot();
		multisig.send({ type: 'open_link', hash: meant ? hash : '', take_kept: takeKept });
	}

	/** Where the link stands, drawn once each (the machine's `link`). */
	let drawn: string | null = null;
	$effect(() => {
		const stand = machines.accounts?.view.link ?? null;
		const key = stand ? JSON.stringify(stand) : null;
		if (key === drawn) return;
		drawn = key;
		if (stand) untrack(() => drawLink(stand));
	});

	function drawLink(stand: MultisigLinkStand) {
		switch (stand.kind) {
			case 'broken':
				// ms-22, r4b-13: a damaged link is said at once, not ignored. A broken
				// invite shows what it held in the add sheet, with what is wrong with
				// it already said (r5reg-22: an address whose letters were changed).
				if (stand.link === 'add') {
					multisigUi.open({ kind: 'add', broken: stand.text, fault: stand.fault ?? 'damaged' });
				} else multisigUi.open({ kind: 'notice', text: m.proposal.badLink });
				break;
			case 'invite':
				// r4D-4: its networks and its members' labels ride along; adding
				// is still the person's tap (r4b-8).
				multisigUi.open({ kind: 'add', link: stand.link });
				break;
			case 'missingNetwork':
				// ms-22: said, with Add network; the multisig is not added for it.
				// The machine opens the link again once the network is here.
				multisigUi.open({ kind: 'missing', chainId: stand.link.chainId, link: stand.link });
				break;
			case 'opening':
				// Reading the multisig may take a while (ms-16): said, never a blank wait.
				multisigUi.open({ kind: 'notice', text: m.proposal.opening });
				return;
			case 'preview':
				// ms-21: already kept here (to follow), or not.
				watched = stand.kept;
				break;
			case 'failed': {
				// ms-22: the person adds it themselves; its transaction opens then.
				// r4b-13: what the lookup found is said at once, not looked up again.
				// §3.5: who can act for it (refused, high risk, not checkable yet) is
				// the add sheet's own lookup to say, with every reason — not a line.
				const reason = stand.reason.kind;
				addPrefill = stand.link.multisig;
				addFailure =
					reason === 'refused' ||
					reason === 'highRisk' ||
					reason === 'unverified' ||
					reason === 'added' ||
					reason === 'chooseNetwork'
						? null
						: m.add[reason];
				multisigUi.open({ kind: 'add' });
				break;
			}
			case 'member':
			case 'kept':
				break;
		}
		multisig.send({ type: 'close_link' });
	}

	/**
	 * The machine opened a link's transaction: on its multisig, shown here
	 * now (a member is here) — or, with none here, read only (ms-21), the
	 * proposals machine reading it (`open_linked`). Not over whatever the
	 * person turned to after closing "Opening…".
	 */
	function openedProposal(notice: Extract<MultisigNotice, { kind: 'open_proposal' }>) {
		if (leftLink) return;
		const chainId = notice.chain_id;
		const safeTxHash = notice.safe_tx_hash;
		if (notice.preview) {
			const account = notice.preview as MultisigAccount;
			proposalsHost.send({
				type: 'open_linked',
				chain_id: chainId,
				safe_tx_hash: safeTxHash,
				preview: $state.snapshot(account) as never
			});
			multisigUi.open({ kind: 'preview', chainId, safeTxHash, account });
			return;
		}
		multisigUi.open({ kind: 'proposal', chainId, safeTxHash });
		const fresh = multisig.find(notice.address);
		if (fresh) void proposals.refresh(fresh, fresh.chainIds);
	}

	/**
	 * r4D-5: what an invite says of the members	/**
	 * r4D-5: what an invite says of the members, kept with the multisig on
	 * this device — only for addresses that are its members (founding, or on
	 * the invite's networks now), never over this device's own names. Labels
	 * only.
	 */
	function keepInvited(address: string, link: AddLink | undefined) {
		if (!link?.members?.length) return;
		multisig.send({
			type: 'keep_invited',
			address,
			chain_ids: link.chainIds,
			members: link.members.map((m) => ({
				address: m.address ?? null,
				name: m.name ?? null,
				kind: m.kind ?? null
			}))
		});
	}

	/** The name the chain index gives a network this device lacks, for its sheet. */
	let missingName = $state<string | null>(null);
	$effect(() => {
		const open = multisigUi.sheet;
		missingName = null;
		if (open?.kind !== 'missing') return;
		let live = true;
		void indexName(open.chainId).then((name) => {
			if (live) missingName = name;
		});
		return () => {
			live = false;
		};
	});

	/**
	 * Settings → Add network, the chain filled in (the wizard reads it in the
	 * chain index). A link's transaction opens once the network is here — the
	 * accounts machine keeps it waiting, through a reload (ms-22).
	 */
	function addNetwork(chainId: number) {
		multisigUi.close();
		// eslint-disable-next-line svelte/no-navigation-without-resolve -- Settings, resolved, with its query
		void goto(`${resolve('/[locale]/settings', { locale })}?addNetwork=${chainId}`);
	}

	/** ms-21: the person chose to follow the linked multisig here. It is not shown. */
	function watch(account: MultisigAccount) {
		void multisig.keepFound(account.address);
		watched = true;
	}

	const sheet = $derived(multisigUi.sheet);
	const active = $derived(multisig.active);
	/** The multisig a dApp's request is for: the one it was granted, shown or not. */
	const dappAccount = $derived(
		sheet?.kind === 'dapp' ? (multisig.find(sheet.request.held.safe) ?? null) : null
	);

	/**
	 * The person closed a dApp request's sheet: the machine answers its site
	 * as things stand (declined before it was proposed, else "proposed"), and
	 * goes back to the multisig shown — it followed the one the site was
	 * granted for the request.
	 */
	function closeDapp(rid: string) {
		proposalsHost.send({ type: 'dapp_leave', id: rid });
		multisigUi.closeDapp(rid);
		const shown = multisig.active;
		if (proposals.followed?.toLowerCase() !== (shown?.address.toLowerCase() ?? null)) {
			proposals.follow(shown, shown?.chainIds ?? []);
		}
	}
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
	{@const invite = sheet.link}
	<!-- Another link while one is open: a fresh sheet, never the last one's
	     address and networks under the new link's. -->
	{#key sheet}
		<AddMultisigSheet
			{m}
			prefill={invite?.multisig ?? sheet.broken ?? addPrefill}
			link={invite}
			broken={sheet.broken !== undefined ? (sheet.fault ?? 'damaged') : false}
			prefillFailure={invite || sheet.broken !== undefined ? null : addFailure}
			{locale}
			{wide}
			{closeLabel}
			onclose={() => {
				addPrefill = '';
				addFailure = null;
				multisigUi.close();
			}}
			onadded={(address) => {
				addPrefill = '';
				addFailure = null;
				keepInvited(address, invite);
				// ms-22: added from a link's transaction: the machine opens that
				// transaction now (`openedProposal`), over this sheet.
				if (multisigUi.sheet?.kind === 'add') multisigUi.close();
			}}
		/>
	{/key}
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
		onexecuted={() => onchanged?.()}
	/>
{:else if sheet?.kind === 'preview'}
	<!-- ms-21: a link's transaction on a device with none of its members (the machine's `linked`). -->
	{@const linked = sheet.account}
	<ProposalSheet
		{m}
		{signing}
		account={linked}
		chainId={sheet.chainId}
		safeTxHash={sheet.safeTxHash}
		preview
		watch={{ added: watched, onwatch: () => watch(linked) }}
		{locale}
		{wide}
		{closeLabel}
		onclose={() => {
			proposalsHost.send({ type: 'close_linked' });
			multisigUi.close();
		}}
		onexecuted={() => onchanged?.()}
	/>
{:else if sheet?.kind === 'dapp' && dappAccount}
	<!-- A dApp's transaction: its review first, then the transaction it became.
	     Every way out is the request's to answer. -->
	{#key sheet.request.rid}
		<ProposalSheet
			{signing}
			{m}
			account={dappAccount}
			chainId={sheet.request.held.chain_id}
			safeTxHash={sheet.request.held.safe_tx_hash ?? ''}
			dapp={sheet.request}
			{locale}
			{wide}
			{closeLabel}
			onclose={() => closeDapp(sheet.request.rid)}
			onexecuted={() => onchanged?.()}
		/>
	{/key}
{:else if sheet?.kind === 'missing'}
	<!-- Closed: the link waiting for its network is let go (the machine's
	     record — Add network keeps it, through a reload, until the network is here). -->
	<Sheet
		title={m.proposal.title}
		{closeLabel}
		{wide}
		onclose={() => {
			multisig.send({ type: 'drop_link' });
			multisigUi.close();
		}}
	>
		<p class="missing-title" data-testid="missing-network">
			{missingTitle(m, sheet.chainId, missingName)}
		</p>
		<p class="notice">{m.proposal.missingNetworkBody}</p>
		<Button variant="primary" shape="rounded" onclick={() => addNetwork(sheet.chainId)}
			>{m.proposal.addNetwork}</Button
		>
	</Sheet>
{:else if sheet?.kind === 'authority'}
	<Sheet title={m.proposal.title} {closeLabel} {wide} onclose={() => multisigUi.close()}>
		{#if sheet.error instanceof SigningBlockedError}
			<!-- §3.6: read right before the passkey, it changes who controls the multisig. -->
			<p class="notice" role="alert">{m.review.blocked}</p>
		{:else if sheet.error instanceof SigningRefusedError}
			<AuthorityNotice
				{m}
				kind="refused"
				findings={sheet.error.findings}
				readOnlyOn={[sheet.error.chainId]}
			/>
		{:else}
			<p class="notice" role="alert">
				{fillTemplate(m.authority.signUnchecked, { network: chainName(sheet.error.chainId) })}
			</p>
		{/if}
	</Sheet>
{:else if sheet?.kind === 'notice'}
	<Sheet
		title={m.proposal.title}
		{closeLabel}
		{wide}
		onclose={() => {
			// Closed while it opens: the link's transaction opens over nothing now.
			if (sheet.text === m.proposal.opening) leftLink = true;
			multisigUi.close();
		}}
	>
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

<!-- §3.6: a proposal's review before the proposer signs it (send, rename, members): on top. -->
{#if proposalsHost.reviewing && proposalsHost.view?.draft}
	{@const first = proposalsHost.reviewing}
	{@const firstAccount = multisig.find(first.address)}
	{#if firstAccount}
		{#key first}
			<ProposalSheet
				{signing}
				{m}
				account={firstAccount}
				chainId={first.chainId}
				safeTxHash={proposalsHost.view.draft.item.safeTxHash}
				prebuilt={first}
				{locale}
				{wide}
				{closeLabel}
				onclose={() => proposalsHost.withdrawDraft()}
			/>
		{/key}
	{/if}
{/if}

<!-- A signature on a high-risk network, asked from any sheet or the send flow: on top of them. -->
{#if riskConfirmation.request}
	{#key riskConfirmation.request}
		<RiskConfirmSheet {m} request={riskConfirmation.request} {wide} {closeLabel} />
	{/key}
{/if}

<!--
	2026-10-29: a member approving on its trusted signing page — the hand-off card;
	and an executor's own operation on its page where no signing host is mounted.
-->
{#if m.handoff !== undefined}
	<PageHandoffSheet
		m={m.handoff}
		{locale}
		{wide}
		{closeLabel}
		kind="approval"
		standIn="signature"
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
	.missing-title {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.notice {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}
</style>
