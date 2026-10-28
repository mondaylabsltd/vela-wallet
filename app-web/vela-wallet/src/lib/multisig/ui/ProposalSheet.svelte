<script lang="ts">
	/**
	 * One multisig transaction — what every member sees, and where each of
	 * them approves.
	 *
	 * What it does is the core's reading of the transaction itself
	 * (`reviewTx`), never text that came with it. A call to another contract
	 * (a dApp's) is read further, the way the signing sheet reads the same
	 * call sent by a site — the same machines, the same blocks (cs-12) — so
	 * every member approves what they can read. Each approval shown was
	 * verified here, and counts only if its owner is an owner on that chain
	 * now. There is one primary action, the next thing that moves it forward:
	 * approve, approve and execute, or execute — each one passkey prompt.
	 *
	 * A dApp's transaction (`draft`) is drawn here before it exists: the same
	 * reading, at the number it would take, with "Propose and approve" and
	 * Decline — nothing is signed until the person has seen what the site
	 * asks. Once proposed it is the transaction like any other.
	 */
	import { onMount, untrack } from 'svelte';
	import Button from '$lib/ui/Button.svelte';
	import { hostLabel } from '$lib/dapp/host';
	import { fillTemplate } from '$lib/i18n/fill';
	import { nativeSymbol } from '$lib/services/networks';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { identiconSvgForClient } from '$lib/wallet/identicon';
	import type { SigningMessages } from '$lib/signing/messages';
	import type { MultisigMessages } from '../messages';
	import { CallReading, callTitle } from '../core/call-review.svelte';
	import { names } from '../core/names.svelte';
	import {
		actionRecipient,
		actionTitle,
		chainName,
		explorerTxUrl,
		formatUnits,
		localName,
		shortenAddress,
		tokenInfo
	} from '../core/display';
	import { ExecutorCannotPayError, relayRefused } from '../core/execute';
	import { sameAddress } from '../core/kernel';
	import { proposalLink, shareOrigin } from '../core/link';
	import { localOwners, type LocalOwner } from '../core/owners';
	import {
		approve,
		approveExternally,
		draftCall,
		execute,
		MailboxUnreadableError,
		proposals,
		proposeCancel,
		type ChainState,
		type ProposalItem
	} from '../core/proposals.svelte';
	import type { MultisigAccount } from '../core/store.svelte';
	import type { DappProposal } from '../core/ui.svelte';
	import CallReview from './CallReview.svelte';
	import Sheet from './Sheet.svelte';

	interface Props {
		m: MultisigMessages;
		/** The signing sheet's words, for a call's review (cs-12). */
		signing: SigningMessages;
		account: MultisigAccount;
		chainId: number;
		safeTxHash: string;
		locale: string;
		wide: boolean;
		closeLabel: string;
		onclose: () => void;
		/** It was executed from this device: the transaction is on chain. */
		onexecuted?: (done: { item: ProposalItem; txHash: string; userOpHash: string }) => void;
		/** A dApp's transaction: reviewed here before it is proposed (`DappRequestHost`). */
		draft?: DappProposal | null;
	}

	let {
		m,
		signing,
		account,
		chainId,
		safeTxHash,
		locale,
		wide,
		closeLabel,
		onclose,
		onexecuted,
		draft = null
	}: Props = $props();

	/** A dApp's transaction not proposed yet: the review, before any passkey. */
	const drafting = $derived(!!draft && !draft.safeTxHash);
	let drafted = $state<{ chain: ChainState; item: ProposalItem } | null>(null);
	let draftFailure = $state<string | null>(null);

	async function loadDraft() {
		if (!draft) return;
		draftFailure = null;
		try {
			drafted = await draftCall(draft.address, chainId, draft.call);
		} catch (error) {
			draftFailure = reasonOf(error);
		}
	}

	onMount(() => {
		if (draft && !draft.safeTxHash) void loadDraft();
	});

	const item = $derived(
		drafting
			? (drafted?.item ?? null)
			: (proposals
					.chain(chainId)
					?.items.find((i) => i.safeTxHash.toLowerCase() === safeTxHash.toLowerCase()) ?? null)
	);
	const members = $derived(
		drafting ? (drafted?.chain.members ?? null) : (proposals.chain(chainId)?.members ?? null)
	);
	const site = $derived(draft ? hostLabel(draft.origin) : '');

	let title = $state('');
	let lines = $state<{ label: string; to: string }[]>([]);
	$effect(() => {
		const current = item;
		if (!current) return;
		void actionTitle(m, chainId, current.review.action, current.nonce).then((t) => (title = t));
		const action = current.review.action;
		if (action?.kind === 'batch') {
			void Promise.all(
				action.transfers.map(async (t) => {
					const info = await tokenInfo(chainId, t.token).catch(() => ({
						symbol: '',
						decimals: 18
					}));
					return { label: `${formatUnits(t.amount, info.decimals)} ${info.symbol}`, to: t.to };
				})
			).then((l) => (lines = l));
		} else lines = [];
	});

	/*
	 * A call to another contract, read by the signing sheet's machines. Keyed
	 * by the call itself: the list is re-read every few seconds, and the same
	 * call must not be read again (and flash "Loading…") each time.
	 */
	const callKey = $derived.by(() => {
		const action = item?.review.action;
		return action?.kind === 'call'
			? `${chainId}:${account.address}:${action.to}:${action.value}:${action.data}`
			: '';
	});
	let reading = $state<CallReading | null>(null);
	$effect(() => {
		if (!callKey) return;
		const { action, chain, address } = untrack(() => ({
			action: item?.review.action,
			chain: chainId,
			address: account.address
		}));
		if (action?.kind !== 'call') return;
		const next = new CallReading(chain, address, action);
		reading = next;
		void next.start();
		return () => {
			next.dispose();
			if (reading === next) reading = null;
		};
	});
	/** The core's verb for a call it could read ("Approve"); else the multisig's line. */
	const shownTitle = $derived((reading && callTitle(reading.clear, signing)) || title);

	/** Whose hand does the work: the person picks among this device's members. */
	let chosen = $state<string | null>(null);
	const executors = $derived(item?.canExecute ?? []);
	const approvers = $derived(item?.canApprove ?? []);
	const candidates = $derived(executors.length > 0 ? executors : approvers);
	const actor = $derived<LocalOwner | null>(
		candidates.find((o) => sameAddress(o.address, chosen)) ?? candidates[0] ?? null
	);
	const actorApproved = $derived(
		!!item && !!actor && item.approvals.some((a) => a.counts && sameAddress(a.owner, actor.address))
	);

	/**
	 * Who can withdraw it: any member on this device, whether or not it has
	 * approved — the proposer above all. Rejecting proposes a cancellation at
	 * its number, which that member approves at once.
	 */
	const rejecters = $derived(localOwners(members?.owners ?? []));
	const rejecter = $derived<LocalOwner | null>(
		rejecters.find((o) => sameAddress(o.address, actor?.address)) ?? rejecters[0] ?? null
	);
	/** "Approve and execute" also asks for the fee: a member who cannot pay it still approves. */
	const canApproveOnly = $derived(
		!!actor && approvers.some((o) => sameAddress(o.address, actor.address))
	);

	type Phase =
		| { kind: 'idle' }
		| { kind: 'busy'; what: 'approve' | 'execute' | 'reject' | 'propose' }
		| { kind: 'sent'; txHash: string | null; by: string }
		| { kind: 'failed'; reason: string };
	let phase = $state<Phase>({ kind: 'idle' });
	let rejecting = $state(false);
	let copied = $state(false);

	// The link another member opens: the wallet's public address, never this
	// extension's own (`chrome-extension://…` opens nowhere else).
	const link = $derived(
		typeof location === 'undefined'
			? ''
			: proposalLink(shareOrigin(location.origin), locale, {
					chainId,
					multisig: account.address,
					safeTxHash
				})
	);

	function cancelled(error: unknown): boolean {
		return (
			error instanceof DOMException &&
			(error.name === 'NotAllowedError' || error.name === 'AbortError')
		);
	}

	function reasonOf(error: unknown): string {
		if (error instanceof ExecutorCannotPayError && actor) {
			const symbol = nativeSymbol(chainId);
			return fillTemplate(m.proposal.cannotPay, {
				name: names.label(actor.address),
				symbol,
				fee: `${formatUnits(error.need, 18)} ${symbol}`,
				balance: `${formatUnits(error.have, 18)} ${symbol}`
			});
		}
		if (error instanceof MailboxUnreadableError) return m.proposal.mailboxDown;
		return error instanceof Error ? error.message : String(error);
	}

	/** The person confirmed the dApp's transaction: propose it, approved by this device's member. */
	async function onPropose() {
		if (!draft || !item) return;
		phase = { kind: 'busy', what: 'propose' };
		try {
			await draft.propose();
			phase = { kind: 'idle' };
		} catch (error) {
			phase = cancelled(error) ? { kind: 'idle' } : { kind: 'failed', reason: reasonOf(error) };
		}
	}

	async function onApprove() {
		if (!item || !actor) return;
		phase = { kind: 'busy', what: 'approve' };
		try {
			await approve(account, item, actor);
			await proposals.refresh(account, account.chainIds);
			phase = { kind: 'idle' };
		} catch (error) {
			phase = cancelled(error) ? { kind: 'idle' } : { kind: 'failed', reason: reasonOf(error) };
		}
	}

	async function onExecute() {
		if (!item || !actor || !members) return;
		phase = { kind: 'busy', what: 'execute' };
		try {
			const by = actor.address;
			const result = await execute(account, item, actor, members);
			phase = { kind: 'sent', txHash: null, by };
			// A reply that was no hash (a 500, a refusal after a lost reply): the
			// relay is asked about it at once, and an op it refused or never took
			// is said now — not after three minutes of "Executing…".
			const watch = new AbortController();
			const refused = result.maybeSent
				? relayRefused(result.userOpHash, chainId, watch.signal)
				: new Promise<boolean>(() => {});
			const outcome = await Promise.race([
				result
					.waitForTxHash(180_000, watch.signal)
					.then((hash) => ({ hash }))
					.catch(() => ({ hash: null })),
				refused.then((yes) => (yes ? ('refused' as const) : new Promise<never>(() => {})))
			]);
			watch.abort();
			if (outcome === 'refused') {
				phase = { kind: 'failed', reason: m.proposal.notSent };
				return;
			}
			const txHash = outcome.hash;
			await proposals.refresh(account, account.chainIds);
			if (!txHash) {
				// Not on chain after the wait: say so, and let them try again —
				// unless the chain shows it executed meanwhile.
				phase =
					item?.status === 'done'
						? { kind: 'sent', txHash: null, by }
						: { kind: 'failed', reason: m.proposal.notLanded };
				return;
			}
			phase = { kind: 'sent', txHash, by };
			onexecuted?.({ item, txHash, userOpHash: result.userOpHash });
		} catch (error) {
			phase = cancelled(error) ? { kind: 'idle' } : { kind: 'failed', reason: reasonOf(error) };
		}
	}

	/** Owners still to approve whose wallets are elsewhere: an outside wallet can sign for any. */
	const outside = $derived(
		!item || item.status === 'done' || item.status === 'queued' || item.review.action === null
			? []
			: (members?.owners ?? []).filter(
					(o) =>
						localOwners([o]).length === 0 &&
						!item.approvals.some((a) => a.counts && sameAddress(a.owner, o))
				)
	);

	async function onExternal() {
		if (!item) return;
		try {
			await approveExternally(account, chainId, item.tx, outside);
			await proposals.refresh(account, account.chainIds);
		} catch (error) {
			if (!cancelled(error)) phase = { kind: 'failed', reason: reasonOf(error) };
		}
	}

	async function onReject() {
		if (!item || !rejecter) return;
		phase = { kind: 'busy', what: 'reject' };
		try {
			await proposeCancel(account, item, rejecter);
			await proposals.refresh(account, account.chainIds);
			rejecting = false;
			phase = { kind: 'idle' };
		} catch (error) {
			phase = cancelled(error) ? { kind: 'idle' } : { kind: 'failed', reason: reasonOf(error) };
		}
	}

	async function copyLink() {
		try {
			await navigator.clipboard.writeText(link);
			copied = true;
			setTimeout(() => (copied = false), 1600);
		} catch {
			// nothing to copy into: the link stays on screen
		}
	}

	async function shareLink() {
		try {
			await navigator.share({ title: shownTitle || m.proposal.title, url: link });
		} catch {
			// dismissed, or no share sheet: copying is the fallback
		}
	}

	const canShare = typeof navigator !== 'undefined' && typeof navigator.share === 'function';
	const busy = $derived(phase.kind === 'busy');
	const recipient = $derived(item ? actionRecipient(item.review.action) : null);
	/**
	 * An owner executing approves as `msg.sender`: once its execution is on
	 * chain, it counts. Anyone else adds nothing — and nobody does while it is
	 * only on its way (the relay may yet refuse it).
	 */
	const shownApproved = $derived.by(() => {
		if (!item) return 0;
		const p = phase;
		if (p.kind !== 'sent' || !p.txHash) return item.approved;
		const owner = (members?.owners ?? []).some((o) => sameAddress(o, p.by));
		const approved = item.approvals.some((a) => a.counts && sameAddress(a.owner, p.by));
		return owner && !approved ? item.approved + 1 : item.approved;
	});
</script>

<Sheet title={m.proposal.title} {closeLabel} {wide} {busy} {onclose}>
	{#if drafting && !item}
		<!-- The dApp's transaction is being read: said, never a blank window. -->
		{#if draftFailure}
			<p class="warn" role="alert">{fillTemplate(m.proposal.failed, { reason: draftFailure })}</p>
			<div class="two">
				<Button variant="secondary" shape="rounded" onclick={() => draft?.decline()}
					>{m.external.decline}</Button
				>
				<Button variant="secondary" shape="rounded" onclick={() => void loadDraft()}
					>{m.external.retry}</Button
				>
			</div>
		{:else}
			<p class="quiet" aria-busy="true">{m.proposal.opening}</p>
		{/if}
	{:else if !item}
		<p class="quiet" aria-busy="true">
			{proposals.loading ? m.proposal.opening : m.proposal.notFound}
		</p>
	{:else}
		{#if draft}
			<p class="site">{fillTemplate(m.proposal.siteAsks, { host: site })}</p>
		{/if}
		<header class="what">
			<p class="title">{shownTitle || '…'}</p>
			{#if recipient}
				{@const named = reading?.clear.result?.contract_name ?? localName(recipient)}
				<p class="to">
					<!-- eslint-disable-next-line svelte/no-at-html-tags -- the core's own SVG, from the address -->
					<span class="avatar" aria-hidden="true">{@html identiconSvgForClient(recipient)}</span>
					<!-- A call names the contract it calls, in the signing sheet's word. -->
					<span class="to-label">{reading ? signing.labelInteracting : m.action.to}</span>
					{#if named}<span class="to-who">{named}</span>{/if}
					<span class="to-address">{shortenAddress(recipient)}</span>
				</p>
			{/if}
			{#if reading}
				<CallReview {reading} m={signing} />
			{/if}
			{#if lines.length > 0}
				<ul class="lines">
					{#each lines as line, i (i)}
						<li>
							<span>{line.label}</span><span class="to-address">{shortenAddress(line.to)}</span>
						</li>
					{/each}
				</ul>
			{/if}
			<dl class="facts">
				<div>
					<dt>{m.action.network}</dt>
					<dd>{chainName(chainId)}</dd>
				</div>
				<div>
					<dt>{m.action.number}</dt>
					<dd>{item.nonce.toString()}</dd>
				</div>
				<div>
					<dt>{m.proposal.hash}</dt>
					<dd class="mono">{item.safeTxHash.slice(0, 10)}…{item.safeTxHash.slice(-8)}</dd>
				</div>
			</dl>
			{#if item.review.action?.kind === 'call' && item.review.action.data !== '0x'}
				<!-- What the call carries, as it will be signed: every member can read it. -->
				<details class="data">
					<summary>{m.proposal.data}</summary>
					<code>{item.review.action.data}</code>
				</details>
			{/if}
		</header>

		{#if item.review.action === null}
			<p class="warn" role="alert">
				<Icon icon={UTILITY_ICONS['triangle-alert']} size="sm" />
				{fillTemplate(m.proposal.refused, { reason: item.review.refusal ?? '' })}
			</p>
		{/if}

		<section class="approvals" aria-label={m.proposal.approvals}>
			<h3>
				{m.proposal.approvals}
				<span class="count"
					>{fillTemplate(m.status.progress, {
						approved: shownApproved,
						threshold: item.threshold
					})}</span
				>
			</h3>
			<div class="bar" aria-hidden="true">
				<span
					class="fill"
					style:width="{Math.min(100, (shownApproved / Math.max(1, item.threshold)) * 100)}%"
				></span>
			</div>
			<ul>
				{#each members?.owners ?? [] as owner (owner)}
					{@const row = item.approvals.find((a) => a.counts && sameAddress(a.owner, owner))}
					{@const executedBy =
						phase.kind === 'sent' && !!phase.txHash && sameAddress(phase.by, owner)}
					{@const mine = localOwners([owner]).length > 0}
					<li class:done={!!row || executedBy}>
						<!-- eslint-disable-next-line svelte/no-at-html-tags -- the core's own SVG, from the address -->
						<span class="avatar" aria-hidden="true">{@html identiconSvgForClient(owner)}</span>
						<span class="who"
							>{names.label(owner)}{#if mine}<span class="here">
									· {m.panel.thisDevice}</span
								>{/if}</span
						>
						<span class="state">
							{#if executedBy}
								<Icon icon={UTILITY_ICONS.check} size="sm" />{m.proposal.executed}
							{:else if row}
								<Icon icon={UTILITY_ICONS.check} size="sm" />{m.proposal.approved}
							{:else if !drafting && outside.some( (o) => sameAddress(o, owner) ) && phase.kind === 'idle'}
								<button type="button" class="external" onclick={onExternal}
									>{m.external.signWithWallet}</button
								>
							{:else}
								{m.proposal.waiting}
							{/if}
						</span>
					</li>
				{/each}
			</ul>
		</section>

		{#if item.status === 'done' && phase.kind !== 'sent'}
			<p class="quiet">{m.proposal.doneNote}</p>
		{:else if item.status === 'queued'}
			<p class="quiet">
				{fillTemplate(m.proposal.queuedNote, {
					nonce: (proposals.chain(chainId)?.members.nonce ?? 0n).toString()
				})}
			</p>
		{/if}
		{#if item.cancelledBy}
			<p class="quiet">{m.proposal.cancelPending}</p>
		{/if}

		{#if drafting}
			<!-- The review of what the site asks: nothing is signed before this. -->
			{#if phase.kind === 'failed'}
				<p class="warn" role="alert">{fillTemplate(m.proposal.failed, { reason: phase.reason })}</p>
			{/if}
			<div class="two">
				<Button variant="secondary" shape="rounded" disabled={busy} onclick={() => draft?.decline()}
					>{m.external.decline}</Button
				>
				{#if item.review.action !== null}
					<Button
						variant="value"
						shape="rounded"
						loading={busy && phase.kind === 'busy' && phase.what === 'propose'}
						disabled={busy}
						onclick={onPropose}>{m.send.propose}</Button
					>
				{/if}
			</div>
		{:else if phase.kind === 'sent'}
			<div class="sent" class:pending={!phase.txHash} role="status" aria-busy={!phase.txHash}>
				<!-- The check is for what is on chain; on its way, it is only said. -->
				{#if phase.txHash}<Icon icon={UTILITY_ICONS.check} size="md" />{/if}
				<span>{phase.txHash ? m.proposal.executed : m.proposal.executing}</span>
				{#if phase.txHash && explorerTxUrl(chainId, phase.txHash)}
					<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- an external block explorer -->
					<a href={explorerTxUrl(chainId, phase.txHash)} target="_blank" rel="noopener noreferrer"
						>{m.proposal.explorer}</a
					>
				{/if}
			</div>
		{:else if item.status !== 'done' && item.review.action !== null}
			{#if candidates.length > 1}
				<fieldset class="actor">
					<legend>{m.proposal.as}</legend>
					{#each candidates as owner (owner.address)}
						<label class:on={actor?.address === owner.address}>
							<input
								type="radio"
								name="actor"
								value={owner.address}
								checked={actor?.address === owner.address}
								onchange={() => (chosen = owner.address)}
							/>
							{owner.name}
						</label>
					{/each}
				</fieldset>
			{/if}

			{#if phase.kind === 'failed'}
				<p class="warn" role="alert">{fillTemplate(m.proposal.failed, { reason: phase.reason })}</p>
			{/if}

			<div class="primary">
				{#if executors.length > 0 && actor}
					{@const approveFirst = !actorApproved && item.approved < item.threshold}
					<div class:two={approveFirst && canApproveOnly}>
						{#if approveFirst && canApproveOnly}
							<!-- Executing pays the fee; approving alone does not. -->
							<Button
								variant="secondary"
								shape="rounded"
								loading={busy && phase.kind === 'busy' && phase.what === 'approve'}
								disabled={busy}
								onclick={onApprove}
							>
								{m.proposal.approve}
							</Button>
						{/if}
						<Button
							variant="value"
							shape="rounded"
							loading={busy && phase.kind === 'busy' && phase.what === 'execute'}
							disabled={busy}
							onclick={onExecute}
						>
							{approveFirst ? m.proposal.approveAndExecute : m.proposal.execute}
						</Button>
					</div>
					<p class="fine">{m.proposal.feeNote}</p>
				{:else if approvers.length > 0 && actor}
					<Button
						variant="value"
						shape="rounded"
						loading={busy && phase.kind === 'busy' && phase.what === 'approve'}
						disabled={busy}
						onclick={onApprove}
					>
						{m.proposal.approve}
					</Button>
				{:else if localOwners(members?.owners ?? []).length === 0}
					<p class="quiet">{m.proposal.notMember}</p>
				{/if}
			</div>
		{/if}

		{#if draft && !drafting && item.status !== 'done'}
			<p class="fine">{fillTemplate(m.proposal.siteWaits, { host: site })}</p>
		{/if}

		<!-- Only while approvals are missing: not once it is being executed. -->
		{#if !drafting && item.status !== 'done' && phase.kind !== 'sent' && shownApproved < item.threshold}
			<section class="invite">
				<h3>{m.proposal.invite}</h3>
				<p class="fine">{m.proposal.inviteHint}</p>
				<div class="link-row">
					<code class="link">{link}</code>
				</div>
				<div class="link-actions">
					<Button variant="secondary" shape="rounded" onclick={copyLink}>
						<span class="label"
							><Icon icon={copied ? UTILITY_ICONS.check : UTILITY_ICONS.copy} size="sm" />{copied
								? m.proposal.copied
								: m.proposal.copyLink}</span
						>
					</Button>
					{#if canShare}
						<Button variant="secondary" shape="rounded" onclick={shareLink}
							>{m.proposal.share}</Button
						>
					{/if}
				</div>
			</section>
		{/if}

		<!-- Any member here may withdraw it, the proposer included, approved or not. -->
		{#if !drafting && item.status !== 'done' && item.review.action?.kind !== 'cancel' && !item.cancelledBy && rejecter && phase.kind !== 'sent'}
			<div class="reject">
				{#if rejecting}
					<p class="fine">{fillTemplate(m.proposal.rejectHint, { threshold: item.threshold })}</p>
					<Button
						variant="danger"
						shape="rounded"
						loading={busy && phase.kind === 'busy' && phase.what === 'reject'}
						disabled={busy}
						onclick={onReject}>{m.proposal.reject}</Button
					>
				{:else}
					<button type="button" class="text-button" data-tap onclick={() => (rejecting = true)}
						>{m.proposal.reject}</button
					>
				{/if}
			</div>
		{/if}

		{#if !drafting}<p class="fine subtle">{m.proposal.noExpiry}</p>{/if}
	{/if}
</Sheet>

<style>
	.external {
		padding: 0;
		border: 0;
		background: none;
		font: inherit;
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
		text-decoration: underline;
		cursor: pointer;
	}

	.what {
		padding-block-end: var(--space-lg);
		border-bottom: var(--border-hairline) solid var(--color-border-base);
	}

	.title {
		margin: 0 0 var(--space-md);
		font-family: var(--font-display);
		font-size: calc(var(--text-2xl) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
		font-variant-numeric: tabular-nums;
	}

	.to {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		margin: 0 0 var(--space-md);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
	}

	.to-label {
		color: var(--color-fg-subtle);
	}

	.to-who {
		color: var(--color-fg-base);
		font-weight: var(--weight-medium);
	}

	.to-address,
	.mono {
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.lines {
		list-style: none;
		margin: 0 0 var(--space-md);
		padding: 0;
	}

	.lines li {
		display: flex;
		justify-content: space-between;
		padding-block: var(--space-xs);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	.avatar {
		display: inline-flex;
		flex: none;
		width: var(--icon-xl);
		height: var(--icon-xl);
		border-radius: var(--radius-full);
		overflow: hidden;
	}

	.avatar :global(svg) {
		width: 100%;
		height: 100%;
	}

	.facts {
		display: grid;
		gap: var(--space-xs);
		margin: 0;
	}

	.facts div {
		display: flex;
		justify-content: space-between;
		gap: var(--space-md);
	}

	dt {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	dd {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	h3 {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		margin: var(--space-xl) 0 var(--space-sm);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.count {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-regular);
		color: var(--color-fg-muted);
	}

	.bar {
		height: var(--space-xs);
		border-radius: var(--radius-full);
		background: var(--color-bg-sunken);
		overflow: hidden;
		margin-block-end: var(--space-md);
	}

	.fill {
		display: block;
		height: 100%;
		background: var(--color-success-base);
		transition: width var(--motion-duration-normal);
	}

	.approvals ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	.approvals li {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		padding-block: var(--space-sm);
	}

	.who {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	.here {
		color: var(--color-fg-subtle);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
	}

	.state {
		display: inline-flex;
		align-items: center;
		gap: var(--space-xs);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	li.done .state {
		color: var(--color-success-base);
	}

	.quiet {
		margin: var(--space-lg) 0 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.warn {
		display: flex;
		gap: var(--space-sm);
		align-items: flex-start;
		margin: var(--space-lg) 0 0;
		padding: var(--space-md);
		border-radius: var(--radius-lg);
		background: var(--color-warning-soft);
		border: var(--border-hairline) solid var(--color-warning-border);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	.actor {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-sm);
		margin: var(--space-xl) 0 0;
		padding: 0;
		border: 0;
	}

	.actor legend {
		margin-block-end: var(--space-sm);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.actor label {
		display: inline-flex;
		align-items: center;
		gap: var(--space-xs);
		padding: var(--space-sm) var(--space-md);
		border-radius: var(--radius-full);
		border: var(--border-hairline) solid var(--color-border-base);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-base);
		cursor: pointer;
	}

	.actor label.on {
		border-color: var(--color-border-strong);
		background: var(--color-bg-sunken);
	}

	.actor input {
		position: absolute;
		opacity: 0;
		pointer-events: none;
	}

	.primary {
		margin-block-start: var(--space-xl);
	}

	.fine {
		margin: var(--space-sm) 0 0;
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.fine.subtle {
		margin-block-start: var(--space-xl);
		color: var(--color-fg-subtle);
	}

	.sent {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		margin-block-start: var(--space-xl);
		padding: var(--space-md);
		border-radius: var(--radius-lg);
		background: var(--color-success-soft);
		color: var(--color-success-base);
		font-weight: var(--weight-medium);
	}

	/* On its way, not on chain: no success colour for what may yet be refused. */
	.sent.pending {
		background: var(--color-bg-sunken);
		color: var(--color-fg-base);
	}

	.two {
		display: flex;
		gap: var(--space-sm);
		margin-block-start: var(--space-xl);
	}

	.primary .two {
		margin-block-start: 0;
	}

	.two > :global(*) {
		flex: 1;
	}

	.site {
		margin: 0 0 var(--space-md);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.data {
		margin-block-start: var(--space-md);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.data summary {
		cursor: pointer;
	}

	.data code {
		display: block;
		margin-block-start: var(--space-sm);
		padding: var(--space-md);
		max-height: calc(var(--space-3xl) * 4);
		overflow: auto;
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
		font-family: var(--font-mono);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		overflow-wrap: anywhere;
	}

	.sent a {
		margin-inline-start: auto;
		color: var(--color-fg-base);
		font-weight: var(--weight-regular);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
	}

	.link-row {
		margin-block-start: var(--space-md);
		padding: var(--space-md);
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
		overflow-wrap: anywhere;
	}

	.link {
		font-family: var(--font-mono);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.link-actions {
		display: flex;
		gap: var(--space-sm);
		margin-block-start: var(--space-md);
	}

	.link-actions > :global(*) {
		flex: 1;
	}

	.label {
		display: inline-flex;
		align-items: center;
		gap: var(--space-sm);
	}

	.reject {
		margin-block-start: var(--space-xl);
		text-align: center;
	}

	.text-button {
		padding: var(--space-sm);
		border: 0;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-error-base);
		cursor: pointer;
	}
</style>
