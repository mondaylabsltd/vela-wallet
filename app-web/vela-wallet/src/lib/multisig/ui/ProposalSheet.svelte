<script lang="ts">
	/**
	 * One multisig transaction — what every member sees, and where each of
	 * them approves.
	 *
	 * What it does is the core's reading of the transaction itself
	 * (`reviewTx`), never text that came with it. Each approval shown was
	 * verified here, and counts only if its owner is an owner on that chain
	 * now. There is one primary action, the next thing that moves it forward:
	 * approve, approve and execute, or execute — each one passkey prompt.
	 */
	import Button from '$lib/ui/Button.svelte';
	import { fillTemplate } from '$lib/i18n/fill';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { identiconSvgForClient } from '$lib/wallet/identicon';
	import type { MultisigMessages } from '../messages';
	import {
		actionRecipient,
		actionTitle,
		chainName,
		explorerTxUrl,
		formatUnits,
		memberLabel,
		shortenAddress,
		tokenInfo
	} from '../core/display';
	import { sameAddress } from '../core/kernel';
	import { proposalLink } from '../core/link';
	import { localOwners, type LocalOwner } from '../core/owners';
	import { approve, execute, proposals, proposeCancel } from '../core/proposals.svelte';
	import type { MultisigAccount } from '../core/store.svelte';
	import Sheet from './Sheet.svelte';

	interface Props {
		m: MultisigMessages;
		account: MultisigAccount;
		chainId: number;
		safeTxHash: string;
		locale: string;
		wide: boolean;
		closeLabel: string;
		onclose: () => void;
	}

	let { m, account, chainId, safeTxHash, locale, wide, closeLabel, onclose }: Props = $props();

	const item = $derived(
		proposals.chain(chainId)?.items.find((i) => i.safeTxHash.toLowerCase() === safeTxHash.toLowerCase()) ?? null
	);
	const members = $derived(proposals.chain(chainId)?.members ?? null);

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
					const info = await tokenInfo(chainId, t.token).catch(() => ({ symbol: '', decimals: 18 }));
					return { label: `${formatUnits(t.amount, info.decimals)} ${info.symbol}`, to: t.to };
				})
			).then((l) => (lines = l));
		} else lines = [];
	});

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

	type Phase =
		| { kind: 'idle' }
		| { kind: 'busy'; what: 'approve' | 'execute' | 'reject' }
		| { kind: 'sent'; txHash: string | null }
		| { kind: 'failed'; reason: string };
	let phase = $state<Phase>({ kind: 'idle' });
	let rejecting = $state(false);
	let copied = $state(false);

	const link = $derived(
		typeof location === 'undefined'
			? ''
			: proposalLink(location.origin, locale, { chainId, multisig: account.address, safeTxHash })
	);

	function cancelled(error: unknown): boolean {
		return error instanceof DOMException && (error.name === 'NotAllowedError' || error.name === 'AbortError');
	}

	function reasonOf(error: unknown): string {
		return error instanceof Error ? error.message : String(error);
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
			const result = await execute(account, item, actor, members);
			phase = { kind: 'sent', txHash: null };
			const txHash = await result.waitForTxHash(180_000).catch(() => null);
			phase = { kind: 'sent', txHash };
			await proposals.refresh(account, account.chainIds);
		} catch (error) {
			phase = cancelled(error) ? { kind: 'idle' } : { kind: 'failed', reason: reasonOf(error) };
		}
	}

	async function onReject() {
		if (!item || !actor) return;
		phase = { kind: 'busy', what: 'reject' };
		try {
			await proposeCancel(account, item, actor);
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
			await navigator.share({ title: title || m.proposal.title, url: link });
		} catch {
			// dismissed, or no share sheet: copying is the fallback
		}
	}

	const canShare = typeof navigator !== 'undefined' && typeof navigator.share === 'function';
	const busy = $derived(phase.kind === 'busy');
	const recipient = $derived(item ? actionRecipient(item.review.action) : null);
</script>

<Sheet title={m.proposal.title} {closeLabel} {wide} {busy} {onclose}>
	{#if !item}
		<p class="quiet" aria-busy="true">{proposals.loading ? m.proposal.opening : m.proposal.notFound}</p>
	{:else}
		<header class="what">
			<p class="title">{title || '…'}</p>
			{#if recipient}
				<p class="to">
					<!-- eslint-disable-next-line svelte/no-at-html-tags -- the core's own SVG, from the address -->
					<span class="avatar" aria-hidden="true">{@html identiconSvgForClient(recipient)}</span>
					<span class="to-label">{m.action.to}</span>
					<span class="to-who">{memberLabel(recipient)}</span>
					<span class="to-address">{shortenAddress(recipient)}</span>
				</p>
			{/if}
			{#if lines.length > 0}
				<ul class="lines">
					{#each lines as line, i (i)}
						<li><span>{line.label}</span><span class="to-address">{shortenAddress(line.to)}</span></li>
					{/each}
				</ul>
			{/if}
			<dl class="facts">
				<div><dt>{m.action.network}</dt><dd>{chainName(chainId)}</dd></div>
				<div><dt>{m.action.number}</dt><dd>{item.nonce.toString()}</dd></div>
				<div><dt>safeTxHash</dt><dd class="mono">{item.safeTxHash.slice(0, 10)}…{item.safeTxHash.slice(-8)}</dd></div>
			</dl>
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
				<span class="count">{fillTemplate(m.status.progress, { approved: item.approved, threshold: item.threshold })}</span>
			</h3>
			<div class="bar" aria-hidden="true">
				<span class="fill" style:width="{Math.min(100, (item.approved / Math.max(1, item.threshold)) * 100)}%"></span>
			</div>
			<ul>
				{#each members?.owners ?? [] as owner (owner)}
					{@const row = item.approvals.find((a) => a.counts && sameAddress(a.owner, owner))}
					{@const mine = localOwners([owner]).length > 0}
					<li class:done={!!row}>
						<!-- eslint-disable-next-line svelte/no-at-html-tags -- the core's own SVG, from the address -->
						<span class="avatar" aria-hidden="true">{@html identiconSvgForClient(owner)}</span>
						<span class="who">{memberLabel(owner)}{#if mine}<span class="here"> · {m.panel.thisDevice}</span>{/if}</span>
						<span class="state">
							{#if row}
								<Icon icon={UTILITY_ICONS.check} size="sm" />{m.proposal.approved}
							{:else}
								{m.proposal.waiting}
							{/if}
						</span>
					</li>
				{/each}
			</ul>
		</section>

		{#if item.status === 'done'}
			<p class="quiet">{m.proposal.doneNote}</p>
		{:else if item.status === 'queued'}
			<p class="quiet">{fillTemplate(m.proposal.queuedNote, { nonce: (proposals.chain(chainId)?.members.nonce ?? 0n).toString() })}</p>
		{/if}
		{#if item.cancelledBy}
			<p class="quiet">{m.proposal.cancelPending}</p>
		{/if}

		{#if phase.kind === 'sent'}
			<div class="sent" role="status">
				<Icon icon={UTILITY_ICONS.check} size="md" />
				<span>{phase.txHash ? m.proposal.executed : m.proposal.executing}</span>
				{#if phase.txHash && explorerTxUrl(chainId, phase.txHash)}
					<a href={explorerTxUrl(chainId, phase.txHash)} target="_blank" rel="noopener noreferrer">{m.proposal.explorer}</a>
				{/if}
			</div>
		{:else if item.status !== 'done' && item.review.action !== null}
			{#if candidates.length > 1}
				<fieldset class="actor">
					<legend>{m.proposal.as}</legend>
					{#each candidates as owner (owner.address)}
						<label class:on={actor?.address === owner.address}>
							<input type="radio" name="actor" value={owner.address} checked={actor?.address === owner.address} onchange={() => (chosen = owner.address)} />
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
					<Button variant="primary" shape="rounded" loading={busy && phase.kind === 'busy' && phase.what === 'execute'} disabled={busy} onclick={onExecute}>
						{actorApproved || item.approved >= item.threshold ? m.proposal.execute : m.proposal.approveAndExecute}
					</Button>
					<p class="fine">{m.proposal.feeNote}</p>
				{:else if approvers.length > 0 && actor}
					<Button variant="primary" shape="rounded" loading={busy && phase.kind === 'busy' && phase.what === 'approve'} disabled={busy} onclick={onApprove}>
						{m.proposal.approve}
					</Button>
				{:else if localOwners(members?.owners ?? []).length === 0}
					<p class="quiet">{m.proposal.notMember}</p>
				{/if}
			</div>
		{/if}

		{#if item.status !== 'done' && item.approved < item.threshold}
			<section class="invite">
				<h3>{m.proposal.invite}</h3>
				<p class="fine">{m.proposal.inviteHint}</p>
				<div class="link-row">
					<code class="link">{link}</code>
				</div>
				<div class="link-actions">
					<Button variant="secondary" shape="rounded" onclick={copyLink}>
						<span class="label"><Icon icon={copied ? UTILITY_ICONS.check : UTILITY_ICONS.copy} size="sm" />{copied ? m.proposal.copied : m.proposal.copyLink}</span>
					</Button>
					{#if canShare}
						<Button variant="secondary" shape="rounded" onclick={shareLink}>{m.proposal.share}</Button>
					{/if}
				</div>
			</section>
		{/if}

		{#if item.status !== 'done' && item.review.action?.kind !== 'cancel' && candidates.length > 0 && phase.kind !== 'sent'}
			<div class="reject">
				{#if rejecting}
					<p class="fine">{fillTemplate(m.proposal.rejectHint, { threshold: item.threshold })}</p>
					<Button variant="danger" shape="rounded" loading={busy && phase.kind === 'busy' && phase.what === 'reject'} disabled={busy} onclick={onReject}>{m.proposal.reject}</Button>
				{:else}
					<button type="button" class="text-button" onclick={() => (rejecting = true)}>{m.proposal.reject}</button>
				{/if}
			</div>
		{/if}

		<p class="fine subtle">{m.proposal.noExpiry}</p>
	{/if}
</Sheet>

<style>
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
