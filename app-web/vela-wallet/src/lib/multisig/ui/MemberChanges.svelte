<script lang="ts">
	/**
	 * Changing who the members are and how many must approve, in Settings:
	 * Add member, Replace and Remove on each member, and the approvals
	 * stepper. Each action is ONE proposal per network the multisig is
	 * deployed on (owner, 2026-10-29: a Safe's owners on one chain never
	 * change another's), built by the core against that network's members as
	 * its chain holds them; where the core refuses one (too many approvals
	 * for the members left, someone already a member, the multisig itself…)
	 * the reason is said for that network, in plain words, before anything is
	 * signed. A network it is not deployed on starts with the original
	 * members when first used: said, and nothing offered that would deploy.
	 *
	 * All of that is the core's members machine (`multisig_members`, the
	 * host's `machines.members`): each network's members, the member entry
	 * (an address, an ENS name, a suggestion; the member's kind), the change
	 * and its plan per network, and the changes waiting. This draws its view
	 * and sends it what the person does.
	 *
	 * Proposing is "Propose and approve", as a transfer is: one passkey per
	 * network (each network's transaction is its own), each plan row's
	 * `proposal` through `proposeChange`. The proposals then go the normal
	 * way — approved on other devices, executed — and the changes still
	 * waiting show here as one change with each network's progress
	 * ("Arc 1/2 · Base 0/2").
	 */
	import Button from '$lib/ui/Button.svelte';
	import { fillTemplate } from '$lib/i18n/fill';
	import { requireApprovals } from '../core/approvals-words';
	import { contactsBook } from '$lib/contacts/core/contacts-book.svelte';
	import Identicon from '$lib/wallet/ui/Identicon.svelte';
	import { identiconSvgForClient } from '$lib/wallet/identicon';
	import type { MultisigMessages } from '../messages';
	import { chainName, shortenAddress } from '../core/display';
	import { sameAddress, type Action, type MemberRefusal, type MemberVerdict } from '../core/kernel';
	import { proposeChange } from '../core/member-propose';
	import { names } from '../core/names.svelte';
	import { multisig, type MultisigAccount } from '../core/store.svelte';
	import { multisigUi } from '../core/ui.svelte';
	import type { MultisigChangeGroup } from '../generated/MultisigChangeGroup';
	import type { MultisigMemberCompose } from '../generated/MultisigMemberCompose';
	import type { MultisigMembersEvent } from '../generated/MultisigMembersEvent';
	import { machines } from '../shell/machines.svelte';
	import MemberKindChoice from './MemberKindChoice.svelte';

	interface Props {
		m: MultisigMessages;
		account: MultisigAccount;
		cancelLabel: string;
	}

	let { m, account, cancelLabel }: Props = $props();

	const mm = $derived(m.members);

	// The members machine is shown this multisig, and told what it reads (the
	// proposals waiting are the proposals machine's, fed by the one host).
	$effect(() => machines.showMembers(account, multisig.accounts));
	$effect(() => machines.membersContacts(contactsBook.view?.contacts ?? []));

	/** The machine's view, once it shows this multisig. */
	const view = $derived.by(() => {
		const v = machines.members?.view;
		return v && sameAddress(v.address, account.address) ? v : null;
	});
	const send = (event: MultisigMembersEvent) => machines.members?.dispatch(event);

	const compose = $derived(view?.compose ?? null);
	const entry = $derived(view?.entry ?? null);
	const plan = $derived(view?.plan ?? null);

	/** What the entry's note says, in the words the sheet always said it in. */
	const entryNote = $derived.by(() => {
		const note = entry?.note;
		if (!note) return null;
		switch (note.kind) {
			case 'invalid_address':
				return m.send.invalidAddress;
			case 'invalid_checksum':
				return m.send.invalidChecksum;
			case 'no_one':
				return mm.refuse.noOne;
			case 'name_not_found':
				return fillTemplate(mm.nameNotFound, { name: note.name });
			case 'name_unavailable':
				return fillTemplate(mm.nameUnavailable, { name: note.name });
		}
	});

	function open(next: MultisigMemberCompose | null) {
		send({ type: 'compose', compose: next });
		progress = [];
		failure = null;
	}

	const rule = (set: { owners: string[]; threshold: number }) =>
		fillTemplate(mm.ruleValue, { m: set.threshold, n: set.owners.length });

	function refusalText(r: MemberRefusal): string {
		const words = mm.refuse[r.kind];
		return fillTemplate(words, r as unknown as Record<string, string | number>);
	}

	const verdictOf = (v: unknown) => v as MemberVerdict;

	function title(action: Action): string {
		switch (action.kind) {
			case 'addOwner':
				return fillTemplate(mm.changeAdd, { name: names.label(action.owner) });
			case 'removeOwner':
				return fillTemplate(mm.changeRemove, { name: names.label(action.owner) });
			case 'swapOwner':
				return fillTemplate(mm.changeReplace, {
					old: names.label(action.oldOwner),
					new: names.label(action.newOwner)
				});
			case 'changeThreshold':
				return requireApprovals(m, action.threshold);
			default:
				return '';
		}
	}

	/** "Arc 1/2 · Base 0/2": each network's approvals so far, of what it needs there. */
	const progressLine = (g: MultisigChangeGroup) =>
		g.parts.map((p) => `${chainName(p.chain_id)} ${p.shown}/${p.threshold}`).join(' · ');

	// -- proposing, network by network ------------------------------------------------
	type Step = { chainId: number; state: 'waiting' | 'proposing' | 'proposed' | 'failed' };
	let progress = $state<Step[]>([]);
	let failure = $state<string | null>(null);
	const busy = $derived(progress.some((p) => p.state === 'proposing'));

	async function proposeAll() {
		if (!view?.plan || !view.can_propose || busy) return;
		// The new member's name and kind are kept with the multisig on this
		// device (r4D-5): the machine tells the accounts machine.
		send({ type: 'proposing' });
		// The networks as they stood when the person pressed: once one is
		// proposed, the plan reads it as a change waiting there.
		const ready = view.plan.networks.flatMap((n) =>
			n.proposal ? [{ chainId: n.chain_id, proposal: n.proposal }] : []
		);
		progress = ready.map(({ chainId }) => ({ chainId, state: 'waiting' }));
		failure = null;
		const made: { chainId: number; safeTxHash: string }[] = [];
		for (const { chainId, proposal } of ready) {
			progress = progress.map((p) => (p.chainId === chainId ? { ...p, state: 'proposing' } : p));
			const outcome = await proposeChange(account.address, chainId, proposal);
			if (outcome.ok) {
				made.push({ chainId, safeTxHash: outcome.safeTxHash });
				progress = progress.map((p) => (p.chainId === chainId ? { ...p, state: 'proposed' } : p));
				continue;
			}
			progress = progress.map((p) => (p.chainId === chainId ? { ...p, state: 'failed' } : p));
			// A passkey prompt dismissed: stop here, nothing said wrong.
			if (outcome.declined) break;
			failure = fillTemplate(mm.failedOn, {
				network: chainName(chainId),
				reason: outcome.refusal
					? refusalText(outcome.refusal)
					: outcome.mailboxDown
						? m.proposal.mailboxDown
						: (outcome.message ?? '')
			});
		}
		if (made.length > 0 && made.length === ready.length) {
			send({ type: 'compose', compose: null });
			progress = [];
			// One network: straight to its proposal, as a transfer's confirm does.
			if (made.length === 1) multisigUi.open({ kind: 'proposal', ...made[0] });
		}
	}
</script>

{#snippet who()}
	{#if entry}
		<label class="field">
			<span>{compose?.kind === 'replace' ? mm.newAccount : mm.entryLabel}</span>
			<input
				class="mono"
				placeholder={mm.entryPlaceholder}
				autocomplete="off"
				spellcheck="false"
				value={entry.typed}
				oninput={(e) => send({ type: 'entry', text: e.currentTarget.value })}
				onblur={() => send({ type: 'read_entry' })}
				onkeydown={(e) => e.key === 'Enter' && (e.preventDefault(), send({ type: 'read_entry' }))}
				data-testid="member-entry"
			/>
		</label>
		{#if entry.resolving}
			<p class="note" role="status">{fillTemplate(mm.resolving, { name: entry.typed.trim() })}</p>
		{:else if entryNote}
			<p class="error" role="alert">{entryNote}</p>
		{/if}
		{#if entry.suggestions.length > 0}
			<p class="label">{mm.suggestions}</p>
			<ul class="suggestions">
				{#each entry.suggestions as s (s.address)}
					<li>
						<!-- Picked on press, before the field's blur reads what is typed. -->
						<button
							type="button"
							onmousedown={(e) => e.preventDefault()}
							onclick={() => send({ type: 'pick', address: s.address })}
						>
							<Identicon svg={identiconSvgForClient(s.address)} size="compact" />
							<span class="text">
								<span class="name">{s.name ?? shortenAddress(s.address)}</span>
								<span class="address">{shortenAddress(s.address)}</span>
							</span>
						</button>
					</li>
				{/each}
			</ul>
		{/if}
		{#if entry.entry}
			<!-- The address the entry resolved to, in full: the member is the address. -->
			<div class="resolved" data-testid="member-resolved">
				<Identicon svg={identiconSvgForClient(entry.entry.address)} size="compact" />
				<span class="text">
					{#if entry.entry.name}<span class="name">{entry.entry.name}</span>{/if}
					<span class="address full">{entry.entry.address}</span>
				</span>
			</div>
			<label class="field">
				<span>{mm.memberName}</span>
				<input
					placeholder={mm.memberNamePlaceholder}
					autocomplete="off"
					value={entry.name}
					oninput={(e) => send({ type: 'entry_name', name: e.currentTarget.value })}
					data-testid="member-name"
				/>
			</label>
			<p class="hint">{mm.nameNote}</p>
			{#if entry.choice}
				<MemberKindChoice
					{m}
					choice={entry.choice}
					onpick={(kind) => send({ type: 'entry_kind', kind })}
				/>
			{/if}
		{/if}
	{/if}
{/snippet}

{#snippet stepperRow(label: string, testid: string)}
	{@const stepper = view?.stepper}
	{#if stepper}
		<div class="stepper-row">
			<span class="label">{label}</span>
			<div class="stepper" role="group" aria-label={label}>
				<button
					type="button"
					aria-label={m.create.fewer}
					disabled={!stepper.can_fewer || busy}
					onclick={() => send({ type: 'step', by: -1 })}>−</button
				>
				<output data-testid={testid}
					>{fillTemplate(mm.ruleValue, { m: stepper.value, n: stepper.n })}</output
				>
				<button
					type="button"
					aria-label={m.create.more}
					disabled={!stepper.can_more || busy}
					onclick={() => send({ type: 'step', by: 1 })}>+</button
				>
			</div>
		</div>
	{/if}
{/snippet}

<div class="changes" data-testid="member-changes">
	<h3>{mm.title}</h3>
	{#if view?.none_deployed}
		<!-- Nothing deployed: a change would deploy it. Said, nothing offered. -->
		<p class="note" data-testid="members-none-deployed">{mm.noneDeployed}</p>
	{:else if view && view.rows.length > 0}
		<ul class="rows">
			{#each view.rows as row (row.address)}
				{@const owner = row.address}
				<li>
					<Identicon svg={identiconSvgForClient(owner)} size="compact" address={owner} />
					<span class="text">
						<span class="name">{names.label(owner)}</span>
						{#if names.label(owner) !== shortenAddress(owner)}
							<span class="address">{shortenAddress(owner)}</span>
						{/if}
					</span>
					<span class="row-buttons">
						<button
							type="button"
							class="small"
							aria-label={fillTemplate(mm.replaceLabel, { name: names.label(owner) })}
							disabled={busy}
							onclick={() => open({ kind: 'replace', owner })}>{mm.replace}</button
						>
						<button
							type="button"
							class="small"
							aria-label={fillTemplate(mm.removeLabel, { name: names.label(owner) })}
							disabled={busy}
							onclick={() => open({ kind: 'remove', owner })}>{mm.remove}</button
						>
					</span>
				</li>
			{/each}
		</ul>
		<p class="hint">{mm.lostPasskey}</p>
		{#if compose?.kind !== 'add'}
			<div class="row-actions">
				<Button
					variant="secondary"
					shape="rounded"
					disabled={busy}
					onclick={() => open({ kind: 'add' })}>{mm.add}</Button
				>
			</div>
		{/if}

		{#if compose === null || compose.kind === 'threshold'}
			{@render stepperRow(mm.thresholdTitle, 'rule-value')}
			{#if view.recovery}
				<p class="note" data-testid="recovery">
					{view.recovery.kind === 'replaceable'
						? fillTemplate(mm.recovery, { m: view.recovery.m, n: view.recovery.n })
						: mm.recoveryAll}
				</p>
			{/if}
		{/if}

		{#if compose}
			<div class="compose" data-testid="member-compose">
				{#if compose.kind === 'add'}
					<h4>{mm.add}</h4>
					{@render who()}
					{@render stepperRow(mm.thresholdAfter, 'threshold-value')}
				{:else if compose.kind === 'replace'}
					<h4>{fillTemplate(mm.replaceLabel, { name: names.label(compose.owner) })}</h4>
					<p class="hint">{fillTemplate(mm.replaceIntro, { name: names.label(compose.owner) })}</p>
					{@render who()}
				{:else if compose.kind === 'remove'}
					<h4>{fillTemplate(mm.removeLabel, { name: names.label(compose.owner) })}</h4>
					<p class="hint">{fillTemplate(mm.removeIntro, { name: names.label(compose.owner) })}</p>
					{@render stepperRow(mm.thresholdAfter, 'threshold-value')}
				{/if}

				{#if compose.kind === 'replace' && view.replace_preview}
					{@const preview = view.replace_preview}
					<!-- Round 5: the preview names who replaces whom, before anything is proposed. -->
					<p class="replacing" data-testid="member-replace-preview">
						{fillTemplate(mm.changeReplace, {
							old: names.label(compose.owner),
							new: preview.new_name ?? names.label(preview.new_address)
						})}
					</p>
				{/if}
				{#if plan}
					<p class="label">{mm.perNetwork}</p>
					<ul class="plan" data-testid="member-plan">
						{#each plan.networks as n (n.chain_id)}
							{@const verdict = verdictOf(n.verdict)}
							<li data-testid="plan-{n.chain_id}" class:refused={verdict.kind === 'refused'}>
								{#if verdict.kind === 'ready'}
									{fillTemplate(mm.onNetwork, {
										network: chainName(n.chain_id),
										before: rule(verdict.before),
										after: rule(verdict.after)
									})}
								{:else}
									{chainName(n.chain_id)}: {refusalText(verdict.refusal)}
								{/if}
								{#each progress.filter((p) => p.chainId === n.chain_id) as p (p.chainId)}
									{#if p.state === 'proposing'}
										<span class="state" role="status"
											>{fillTemplate(mm.proposing, { network: chainName(n.chain_id) })}</span
										>
									{:else if p.state === 'proposed'}
										<span class="state"
											>{fillTemplate(mm.proposed, { network: chainName(n.chain_id) })}</span
										>
									{/if}
								{/each}
							</li>
						{/each}
						{#each plan.undeployed as chainId (chainId)}
							<li class="undeployed" data-testid="plan-undeployed-{chainId}">
								{fillTemplate(m.settings.undeployedOn, { network: chainName(chainId) })}
							</li>
						{/each}
					</ul>
				{/if}
				{#if failure}<p class="error" role="alert">{failure}</p>{/if}
				<div class="row-actions">
					<Button
						variant="value"
						shape="rounded"
						disabled={!view.can_propose}
						loading={busy}
						busyLabel={m.send.propose}
						onclick={() => void proposeAll()}>{m.send.propose}</Button
					>
					<Button variant="secondary" shape="rounded" disabled={busy} onclick={() => open(null)}
						>{cancelLabel}</Button
					>
				</div>
			</div>
		{/if}
	{/if}

	{#if view && view.groups.length > 0}
		<div class="waiting" data-testid="member-groups">
			<p class="label">{mm.waitingTitle}</p>
			<ul>
				{#each view.groups as g (g.key)}
					<li data-testid="member-group" data-progress={progressLine(g)}>
						<span class="name">{title(g.action as Action)}</span>
						<span class="parts">
							{#each g.parts as p, i (p.chain_id)}
								{#if i > 0}<span aria-hidden="true">·</span>{/if}
								<button
									type="button"
									class="part"
									onclick={() =>
										multisigUi.open({
											kind: 'proposal',
											chainId: p.chain_id,
											safeTxHash: p.safe_tx_hash
										})}
									>{chainName(p.chain_id)}
									{p.shown}/{p.threshold}</button
								>
							{/each}
						</span>
					</li>
				{/each}
			</ul>
		</div>
	{/if}
</div>

<style>
	.changes {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	h3 {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	h4 {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	.rows li {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		padding-block: var(--space-sm);
		border-bottom: var(--border-hairline) solid var(--color-border-base);
	}

	.text {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		flex: 1;
		min-width: 0;
	}

	.name {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
		overflow-wrap: anywhere;
	}

	.address {
		font-family: var(--font-mono);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.address.full {
		overflow-wrap: anywhere;
	}

	.row-buttons {
		display: flex;
		gap: var(--space-xs);
	}

	.small {
		min-height: var(--size-control-sm);
		padding: 0 var(--space-md);
		border: var(--border-hairline) solid var(--color-border-strong);
		border-radius: var(--radius-full);
		background: var(--color-bg-base);
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-base);
		cursor: pointer;
	}

	.small:disabled {
		opacity: var(--opacity-disabled);
		cursor: default;
	}

	.row-actions {
		display: flex;
		gap: var(--space-sm);
	}

	.row-actions > :global(*) {
		width: auto;
		padding-inline: var(--space-xl);
	}

	.stepper-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-md);
	}

	.stepper {
		display: flex;
		align-items: center;
		gap: var(--space-md);
	}

	.stepper button {
		width: var(--size-control-md);
		height: var(--size-control-md);
		border-radius: var(--radius-full);
		border: var(--border-hairline) solid var(--color-border-strong);
		background: var(--color-bg-base);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		color: var(--color-fg-base);
		cursor: pointer;
	}

	.stepper button:disabled {
		opacity: var(--opacity-disabled);
		cursor: default;
	}

	output {
		min-width: calc(var(--space-5xl) * 1.5);
		text-align: center;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.compose {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
		padding: var(--space-md);
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.field input {
		min-height: var(--size-control-lg);
		padding: 0 var(--space-md);
		border: var(--border-hairline) solid var(--color-border-strong);
		border-radius: var(--radius-lg);
		background: var(--color-bg-base);
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	.field input.mono {
		font-family: var(--font-mono);
	}

	.suggestions button,
	.resolved {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		width: 100%;
		padding: var(--space-sm);
		border: 0;
		border-radius: var(--radius-md);
		background: var(--color-bg-base);
		font: inherit;
		text-align: start;
		cursor: pointer;
	}

	.resolved {
		cursor: default;
	}

	.plan li {
		padding-block: var(--space-xs);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	.plan li.refused,
	.plan li.undeployed {
		color: var(--color-fg-muted);
	}

	.state {
		display: block;
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.waiting li {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		padding-block: var(--space-sm);
		border-bottom: var(--border-hairline) solid var(--color-border-base);
	}

	.parts {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-xs);
		color: var(--color-fg-muted);
	}

	.part {
		min-height: var(--size-control-sm);
		padding: 0 var(--space-xs);
		border: 0;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-base);
		text-decoration: underline;
		cursor: pointer;
	}

	.label {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-muted);
	}

	.hint,
	.note {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.replacing {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
		overflow-wrap: anywhere;
	}

	.error {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-error-base);
	}
</style>
