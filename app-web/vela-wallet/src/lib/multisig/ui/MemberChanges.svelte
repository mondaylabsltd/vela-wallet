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
	 * Proposing is "Propose and approve", as a transfer is: one passkey per
	 * network (each network's transaction is its own). The proposals then go
	 * the normal way — approved on other devices, executed — and the changes
	 * still waiting show here as one change with each network's progress
	 * ("Arc 1/2 · Base 0/2").
	 */
	import Button from '$lib/ui/Button.svelte';
	import { fillTemplate } from '$lib/i18n/fill';
	import { loadAccounts } from '$lib/onboarding/core/storage';
	import { contactsBook } from '$lib/contacts/core/contacts-book.svelte';
	import Identicon from '$lib/wallet/ui/Identicon.svelte';
	import { identiconSvgForClient } from '$lib/wallet/identicon';
	import type { MultisigMessages } from '../messages';
	import { chainName, shortenAddress } from '../core/display';
	import {
		sameAddress,
		type Action,
		type MemberChangeRequest,
		type MemberRefusal
	} from '../core/kernel';
	import {
		deployedMembers,
		memberChangeGroups,
		MemberChangeRefusedError,
		planMemberChange,
		progressLine
	} from '../core/member-changes';
	import { looksLikeName, resolveTypedName } from '../core/member-entry';
	import { memberProblem } from '../core/members';
	import { kindChoice, type MemberKind } from '../core/member-kind';
	import { keptMember, names } from '../core/names.svelte';
	import { MailboxUnreadableError, proposals, proposeMemberChange } from '../core/proposals.svelte';
	import { signers } from '../core/signers.svelte';
	import { multisig, type MultisigAccount } from '../core/store.svelte';
	import { multisigUi } from '../core/ui.svelte';
	import MemberKindChoice from './MemberKindChoice.svelte';

	interface Props {
		m: MultisigMessages;
		account: MultisigAccount;
		cancelLabel: string;
	}

	let { m, account, cancelLabel }: Props = $props();

	const mm = $derived(m.members);
	const chains = $derived(proposals.chains);
	const deployed = $derived(chains.filter((c) => c.members.deployed));
	/** The members the rows offer Replace and Remove on: every deployed network's. */
	const rows = $derived(deployedMembers(chains));
	/** The rule the stepper starts from: the first deployed network's. */
	const base = $derived(deployed[0]?.members ?? null);
	const groups = $derived(
		memberChangeGroups(
			proposals.items,
			chains.map((c) => c.chainId)
		)
	);

	type Compose =
		| { kind: 'add' }
		| { kind: 'remove'; owner: string }
		| { kind: 'replace'; owner: string }
		| { kind: 'threshold' };
	let compose = $state<Compose | null>(null);
	/** The approvals the person picked; `null`: untouched (each network keeps its own). */
	let picked = $state<number | null>(null);

	// -- who joins (Add, Replace) -------------------------------------------------
	let typed = $state('');
	let entry = $state<{ address: string; name: string | null } | null>(null);
	let entryNote = $state<string | null>(null);
	let resolving = $state(false);
	let memberName = $state('');
	let kindPick = $state<MemberKind | null>(null);

	const accounts = loadAccounts();
	/** This device's accounts and the contacts, matching what is typed. */
	const suggestions = $derived.by(() => {
		const q = typed.trim().toLowerCase();
		if (q === '' || entry) return [];
		const book = (contactsBook.view?.contacts ?? []).map((c) => ({
			address: c.address,
			name: c.name ?? c.resolved_name ?? null
		}));
		const mine = accounts.map((a) => ({ address: a.address, name: a.name || null }));
		return [...mine, ...book]
			.filter((s, i, all) => all.findIndex((x) => sameAddress(x.address, s.address)) === i)
			.filter((s) => s.name?.toLowerCase().includes(q) || s.address.toLowerCase().startsWith(q))
			.slice(0, 5);
	});
	/**
	 * r7: the member's kind — Vela's finding where it has one (an account
	 * here or the index: a Vela account; code on one of the multisig's
	 * networks and no index record: a smart contract account), else the
	 * person's word (here, or kept on this device), else nothing: asked,
	 * "Other wallet" only hinted when no Vela account is found — never
	 * guessed (it used to read "Other wallet" whenever nothing said).
	 */
	const kindShown = $derived(
		entry
			? kindChoice(
					signers.evidence(account.chainIds, entry.address),
					kindPick ?? keptMember(entry.address).kind
				)
			: null
	);

	let lookup = 0;
	async function readEntry() {
		const value = typed.trim();
		entry = null;
		entryNote = null;
		if (value === '') return;
		if (looksLikeName(value)) {
			const ask = ++lookup;
			resolving = true;
			const answer = await resolveTypedName(value);
			if (ask !== lookup) return;
			resolving = false;
			if (answer.state === 'resolved') {
				entry = { address: answer.address, name: answer.name };
				if (memberName === '') memberName = answer.name;
			} else {
				entryNote = fillTemplate(
					answer.state === 'not_found' ? mm.nameNotFound : mm.nameUnavailable,
					{ name: value }
				);
			}
			return;
		}
		const problem = memberProblem(value, []);
		if (problem === null) entry = { address: value, name: null };
		// A name being searched among the suggestions is no wrong address.
		else if (problem === 'invalidAddress' && suggestions.length > 0) return;
		else
			entryNote =
				problem === 'checksum'
					? m.send.invalidChecksum
					: problem === 'notMember'
						? mm.refuse.noOne
						: m.send.invalidAddress;
	}

	function pick(s: { address: string; name: string | null }) {
		typed = s.address;
		entry = { address: s.address, name: null };
		entryNote = null;
		if (memberName === '' && s.name) memberName = s.name;
		kindPick = null;
	}

	function open(next: Compose) {
		compose = next;
		picked = null;
		typed = '';
		entry = null;
		entryNote = null;
		memberName = '';
		kindPick = null;
		progress = [];
		failure = null;
	}

	// -- the change, and what it is on each network ---------------------------------
	const change = $derived.by((): MemberChangeRequest | null => {
		const c = compose;
		if (!c) return null;
		const threshold = picked ?? undefined;
		switch (c.kind) {
			case 'add':
				return entry ? { kind: 'add', owner: entry.address, threshold } : null;
			case 'remove':
				return { kind: 'remove', owner: c.owner, threshold };
			case 'replace':
				return entry ? { kind: 'replace', oldOwner: c.owner, newOwner: entry.address } : null;
			case 'threshold':
				return picked === null ? null : { kind: 'threshold', threshold: picked };
		}
	});
	const plan = $derived(change ? planMemberChange(account.address, chains, change) : null);

	/** The approvals stepper's bounds and value for the change being composed. */
	const stepper = $derived.by(() => {
		if (!base || !compose || compose.kind === 'replace') return null;
		const n =
			base.owners.length + (compose.kind === 'add' ? 1 : compose.kind === 'remove' ? -1 : 0);
		// Untouched, it reads the rule as it stands — "3 of 2" when removing
		// would leave too few, which the core's reason under it explains.
		return { n, value: picked ?? base.threshold };
	});

	function step(by: number) {
		if (!stepper) return;
		picked = Math.max(1, Math.min(stepper.n, stepper.value + by));
	}

	const rule = (set: { owners: string[]; threshold: number }) =>
		fillTemplate(mm.ruleValue, { m: set.threshold, n: set.owners.length });

	function refusalText(r: MemberRefusal): string {
		const words = mm.refuse[r.kind];
		return fillTemplate(words, r as unknown as Record<string, string | number>);
	}

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
				return fillTemplate(m.action.changeThreshold, { threshold: action.threshold });
			default:
				return '';
		}
	}

	// -- proposing, network by network ------------------------------------------------
	type Step = { chainId: number; state: 'waiting' | 'proposing' | 'proposed' | 'failed' };
	let progress = $state<Step[]>([]);
	let failure = $state<string | null>(null);
	const busy = $derived(progress.some((p) => p.state === 'proposing'));

	async function proposeAll() {
		const c = change;
		if (!c || !plan || plan.ready.length === 0 || busy) return;
		if ((c.kind === 'add' || c.kind === 'replace') && entry) {
			const who = c.kind === 'add' ? c.owner : c.newOwner;
			// Kept with the multisig on this device, as its creator's names are
			// (r4D-5): one store of members' labels, read by `names.label`.
			multisig.nameMembers(
				account.address,
				{ [who]: { name: memberName, ...(kindShown?.kind ? { kind: kindShown.kind } : {}) } },
				{ typed: true }
			);
		}
		// The networks as they stood when the person pressed: once one is
		// proposed, the plan reads it as a change waiting there.
		const ready = [...plan.ready];
		progress = ready.map((chainId) => ({ chainId, state: 'waiting' }));
		failure = null;
		const made: { chainId: number; safeTxHash: string }[] = [];
		for (const chainId of ready) {
			progress = progress.map((p) => (p.chainId === chainId ? { ...p, state: 'proposing' } : p));
			try {
				const { safeTxHash } = await proposeMemberChange(account.address, chainId, c);
				made.push({ chainId, safeTxHash });
				progress = progress.map((p) => (p.chainId === chainId ? { ...p, state: 'proposed' } : p));
			} catch (error) {
				progress = progress.map((p) => (p.chainId === chainId ? { ...p, state: 'failed' } : p));
				const name = error instanceof DOMException ? error.name : '';
				// A passkey prompt dismissed: stop here, nothing said wrong.
				if (name === 'NotAllowedError' || name === 'AbortError') break;
				failure = fillTemplate(mm.failedOn, {
					network: chainName(chainId),
					reason:
						error instanceof MemberChangeRefusedError
							? refusalText(error.refusal)
							: error instanceof MailboxUnreadableError
								? m.proposal.mailboxDown
								: error instanceof Error
									? error.message
									: String(error)
				});
			}
		}
		if (made.length > 0 && made.length === ready.length) {
			compose = null;
			progress = [];
			// One network: straight to its proposal, as a transfer's confirm does.
			if (made.length === 1) multisigUi.open({ kind: 'proposal', ...made[0] });
		}
	}
</script>

{#snippet who()}
	<label class="field">
		<span>{compose?.kind === 'replace' ? mm.newAccount : mm.entryLabel}</span>
		<input
			class="mono"
			placeholder={mm.entryPlaceholder}
			autocomplete="off"
			spellcheck="false"
			bind:value={typed}
			oninput={() => {
				entry = null;
				entryNote = null;
				kindPick = null;
			}}
			onblur={() => void readEntry()}
			onkeydown={(e) => e.key === 'Enter' && (e.preventDefault(), void readEntry())}
			data-testid="member-entry"
		/>
	</label>
	{#if resolving}
		<p class="note" role="status">{fillTemplate(mm.resolving, { name: typed.trim() })}</p>
	{:else if entryNote}
		<p class="error" role="alert">{entryNote}</p>
	{/if}
	{#if suggestions.length > 0}
		<p class="label">{mm.suggestions}</p>
		<ul class="suggestions">
			{#each suggestions as s (s.address)}
				<li>
					<!-- Picked on press, before the field's blur reads what is typed. -->
					<button type="button" onmousedown={(e) => e.preventDefault()} onclick={() => pick(s)}>
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
	{#if entry}
		<!-- The address the entry resolved to, in full: the member is the address. -->
		<div class="resolved" data-testid="member-resolved">
			<Identicon svg={identiconSvgForClient(entry.address)} size="compact" />
			<span class="text">
				{#if entry.name}<span class="name">{entry.name}</span>{/if}
				<span class="address full">{entry.address}</span>
			</span>
		</div>
		<label class="field">
			<span>{mm.memberName}</span>
			<input
				placeholder={mm.memberNamePlaceholder}
				autocomplete="off"
				bind:value={memberName}
				data-testid="member-name"
			/>
		</label>
		<p class="hint">{mm.nameNote}</p>
		{#if kindShown}
			<MemberKindChoice {m} choice={kindShown} onpick={(k) => (kindPick = k)} />
		{/if}
	{/if}
{/snippet}

{#snippet thresholdStepper(label: string)}
	{#if stepper}
		<div class="stepper-row">
			<span class="label">{label}</span>
			<div class="stepper" role="group" aria-label={label}>
				<button
					type="button"
					aria-label={m.create.fewer}
					disabled={stepper.value <= 1 || busy}
					onclick={() => step(-1)}>−</button
				>
				<output data-testid="threshold-value"
					>{fillTemplate(mm.ruleValue, { m: stepper.value, n: stepper.n })}</output
				>
				<button
					type="button"
					aria-label={m.create.more}
					disabled={stepper.value >= stepper.n || busy}
					onclick={() => step(1)}>+</button
				>
			</div>
		</div>
	{/if}
{/snippet}

<div class="changes" data-testid="member-changes">
	<h3>{mm.title}</h3>
	{#if deployed.length === 0}
		<!-- Nothing deployed: a change would deploy it. Said, nothing offered. -->
		<p class="note" data-testid="members-none-deployed">{mm.noneDeployed}</p>
	{:else}
		<ul class="rows">
			{#each rows as owner (owner)}
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
			{#if base}
				{@const s = { n: base.owners.length, value: picked ?? base.threshold }}
				<div class="stepper-row">
					<span class="label">{mm.thresholdTitle}</span>
					<div class="stepper" role="group" aria-label={mm.thresholdTitle}>
						<button
							type="button"
							aria-label={m.create.fewer}
							disabled={s.value <= 1 || busy}
							onclick={() => {
								compose = { kind: 'threshold' };
								picked = Math.max(1, s.value - 1);
							}}>−</button
						>
						<output data-testid="rule-value"
							>{fillTemplate(mm.ruleValue, { m: s.value, n: s.n })}</output
						>
						<button
							type="button"
							aria-label={m.create.more}
							disabled={s.value >= s.n || busy}
							onclick={() => {
								compose = { kind: 'threshold' };
								picked = Math.min(s.n, s.value + 1);
							}}>+</button
						>
					</div>
				</div>
				<p class="note" data-testid="recovery">
					{base.threshold < base.owners.length
						? fillTemplate(mm.recovery, { m: base.threshold, n: base.owners.length })
						: mm.recoveryAll}
				</p>
			{/if}
		{/if}

		{#if compose}
			<div class="compose" data-testid="member-compose">
				{#if compose.kind === 'add'}
					<h4>{mm.add}</h4>
					{@render who()}
					{@render thresholdStepper(mm.thresholdAfter)}
				{:else if compose.kind === 'replace'}
					<h4>{fillTemplate(mm.replaceLabel, { name: names.label(compose.owner) })}</h4>
					<p class="hint">{fillTemplate(mm.replaceIntro, { name: names.label(compose.owner) })}</p>
					{@render who()}
				{:else if compose.kind === 'remove'}
					<h4>{fillTemplate(mm.removeLabel, { name: names.label(compose.owner) })}</h4>
					<p class="hint">{fillTemplate(mm.removeIntro, { name: names.label(compose.owner) })}</p>
					{@render thresholdStepper(mm.thresholdAfter)}
				{/if}

				{#if plan && compose.kind === 'replace' && entry}
					<!-- Round 5: the preview names who replaces whom, before anything is proposed. -->
					<p class="replacing" data-testid="member-replace-preview">
						{fillTemplate(mm.changeReplace, {
							old: names.label(compose.owner),
							new: memberName.trim() || entry.name || names.label(entry.address)
						})}
					</p>
				{/if}
				{#if plan}
					<p class="label">{mm.perNetwork}</p>
					<ul class="plan" data-testid="member-plan">
						{#each plan.networks as n (n.chainId)}
							<li data-testid="plan-{n.chainId}" class:refused={n.verdict.kind === 'refused'}>
								{#if n.verdict.kind === 'ready'}
									{fillTemplate(mm.onNetwork, {
										network: chainName(n.chainId),
										before: rule(n.verdict.before),
										after: rule(n.verdict.after)
									})}
								{:else}
									{chainName(n.chainId)}: {refusalText(n.verdict.refusal)}
								{/if}
								{#each progress.filter((p) => p.chainId === n.chainId) as p (p.chainId)}
									{#if p.state === 'proposing'}
										<span class="state" role="status"
											>{fillTemplate(mm.proposing, { network: chainName(n.chainId) })}</span
										>
									{:else if p.state === 'proposed'}
										<span class="state"
											>{fillTemplate(mm.proposed, { network: chainName(n.chainId) })}</span
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
						disabled={!plan || plan.ready.length === 0}
						loading={busy}
						busyLabel={m.send.propose}
						onclick={() => void proposeAll()}>{m.send.propose}</Button
					>
					<Button
						variant="secondary"
						shape="rounded"
						disabled={busy}
						onclick={() => {
							compose = null;
							picked = null;
							progress = [];
						}}>{cancelLabel}</Button
					>
				</div>
			</div>
		{/if}
	{/if}

	{#if groups.length > 0}
		<div class="waiting" data-testid="member-groups">
			<p class="label">{mm.waitingTitle}</p>
			<ul>
				{#each groups as g (g.key)}
					<li data-testid="member-group" data-progress={progressLine(g, chainName)}>
						<span class="name">{title(g.action)}</span>
						<span class="parts">
							{#each g.parts as p, i (p.chainId)}
								{#if i > 0}<span aria-hidden="true">·</span>{/if}
								<button
									type="button"
									class="part"
									onclick={() =>
										multisigUi.open({
											kind: 'proposal',
											chainId: p.chainId,
											safeTxHash: p.safeTxHash
										})}
									>{chainName(p.chainId)}
									{Math.min(p.approved, p.threshold)}/{p.threshold}</button
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
