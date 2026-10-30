<script lang="ts">
	/**
	 * A multisig's own part of Settings → Account, where a Vela account shows
	 * its keys: how many must approve and who its members are, inviting them,
	 * its name in the registry (renamed on chain by a proposal of its own),
	 * its setup saved to Ethereum (a Vela account's "save to Ethereum"), and
	 * taking it off this device. Everything else in Settings is the same for
	 * both.
	 *
	 * The name a member types here stays a proposal until the members approve
	 * and execute it: the device's label (`label`) is never touched, and what
	 * the header shows is the record's name as the chain holds it.
	 *
	 * Where it can be renamed, sharing it again and its Ethereum backup are
	 * the core's accounts machine's (`plan_rename`, `share_again`,
	 * `check_backup`, `save_to_ethereum`): this draws its views.
	 */
	import { onMount } from 'svelte';
	import Button from '$lib/ui/Button.svelte';
	import { fillTemplate } from '$lib/i18n/fill';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import Identicon from '$lib/wallet/ui/Identicon.svelte';
	import { identiconSvgForClient } from '$lib/wallet/identicon';
	import type { MultisigMessages } from '../messages';
	import { chainName, ruleTitle, shortenAddress } from '../core/display';
	import { memberDrift, sameAddress, validName } from '../core/kernel';
	import { names } from '../core/names.svelte';
	import { localOwners } from '../core/owners';
	import {
		MailboxUnreadableError,
		membersAnywhere,
		proposals,
		proposeRename
	} from '../core/proposals.svelte';
	import {
		SigningBlockedError,
		SigningRefusedError,
		SigningUncheckedError
	} from '../core/gate.svelte';
	import { isShared, multisig, type MultisigAccount } from '../core/store.svelte';
	import { failureText } from '../shell/failure';
	import { localAccounts } from '../shell/local-accounts';
	import { multisigUi } from '../core/ui.svelte';
	import AuthorityNotice from './AuthorityNotice.svelte';
	import CustomServiceNote from './CustomServiceNote.svelte';
	import MemberChanges from './MemberChanges.svelte';

	interface Props {
		m: MultisigMessages;
		account: MultisigAccount;
		/** A Vela account on this device that pays for the Ethereum record. */
		payer: string | null;
		cancelLabel: string;
	}

	let { m, account, payer, cancelLabel }: Props = $props();

	/**
	 * Owners are each network's (owner, 2026-10-29): the original members (its
	 * genesis) are where it starts, and each network it is deployed on has its
	 * own members now, read through this wallet's RPC (`proposals`), shown
	 * beside the original ones with what changed. A network it is not
	 * deployed on starts with the original members.
	 */
	const original = $derived({ owners: account.owners, threshold: account.threshold });
	const deployedOn = $derived(
		proposals.chains
			.filter((c) => c.members.deployed)
			.map((c) => ({
				chainId: c.chainId,
				members: c.members,
				drift: memberDrift(original, c.members)
			}))
	);
	const undeployedOn = $derived(proposals.chains.filter((c) => !c.members.deployed));
	/**
	 * This device's members anywhere: on each network it is deployed on, and
	 * the original ones wherever it is not (or nothing is read yet).
	 */
	const local = $derived(localOwners(membersAnywhere(account)));
	/** §3.5: what the chain says, split: a genesis refused on a network, and a risk where it is deployed. */
	const findings = $derived((multisig.find(account.address) ?? account).findings ?? []);
	const refusedFindings = $derived(findings.filter((f) => f.from === 'genesis'));
	const refusedOn = $derived([...new Set(refusedFindings.map((f) => f.chainId))]);
	const riskFindings = $derived(findings.filter((f) => f.from === 'chain'));

	/** Who proves an owner asks to share it: an original member (the service checks the genesis). */
	const founders = $derived(localOwners(original.owners));

	let removing = $state(false);
	const key = $derived(account.address.toLowerCase());
	/** The machine's row: whether it is shared, whether its record is on its way (r4a-12). */
	const row = $derived(multisig.row(account.address));
	const shared = $derived(row?.shared ?? isShared(account));
	const recordWaiting = $derived(row?.record_waiting ?? false);

	/**
	 * Where its record stands on Ethereum (`check_backup`): a record sent from
	 * here reads as backed up; one whose send failed or was closed, as not.
	 */
	const backup = $derived.by(() => {
		const state = multisig.view?.backup[key];
		if (state === undefined) return 'checking';
		if (state === 'sent') return 'backedUp';
		if (state === 'failed') return 'notBackedUp';
		return state;
	});

	function check() {
		multisig.send({ type: 'check_backup', address: account.address });
	}

	/**
	 * Where its record can be renamed (`plan_rename`): a registry chain
	 * holding it that this wallet has a network for — the service's home
	 * chain (the index, Gnosis) first, where it writes records. `rename` is
	 * called BY the Safe on the chain whose registry holds the record it
	 * changes, so the transaction runs there whatever networks the multisig is
	 * used on. `null`: none (or not recorded); `undefined`: not known yet.
	 */
	const plan = $derived(multisig.view?.rename[key]);
	const renameChain = $derived(plan === undefined ? undefined : plan.chain_id);
	/**
	 * Whether it is deployed on `renameChain` (this wallet's RPC). A rename
	 * never deploys it (owner, 2026-10-29): deployed there, renaming is
	 * offered; not, Settings says only the multisig itself can rename it
	 * there, and offers nothing. `undefined`: not known yet.
	 */
	const renameDeployed = $derived(plan?.deployed ?? undefined);
	/**
	 * r4a-12: why there is no `renameChain` — its record is not saved yet
	 * (nothing to rename), the service could not be asked, or it is held only
	 * on chains this wallet lacks. Each said as itself.
	 */
	const renameBlocked = $derived(plan?.blocked ?? null);

	function findRenameChain() {
		multisig.send({ type: 'plan_rename', address: account.address });
	}

	onMount(() => {
		check();
		findRenameChain();
	});

	let renaming = $state(false);
	let newName = $state('');
	let renameBusy = $state(false);
	let renameFailure = $state<string | null>(null);
	const recordedName = $derived((multisig.find(account.address) ?? account).genesis.name ?? null);
	const cleanNewName = $derived(newName.trim());
	const newNameOk = $derived.by(() => {
		try {
			return cleanNewName !== '' && validName(cleanNewName);
		} catch {
			return false;
		}
	});

	async function rename() {
		if (!newNameOk || renameChain == null || !renameDeployed || cleanNewName === recordedName)
			return;
		const chainId = renameChain;
		renameBusy = true;
		renameFailure = null;
		try {
			const { safeTxHash } = await proposeRename(account.address, chainId, cleanNewName);
			renaming = false;
			multisigUi.open({ kind: 'proposal', chainId, safeTxHash });
		} catch (error) {
			// Declined (the passkey, or a high-risk network's confirmation): nothing to say.
			if (error instanceof DOMException && ['NotAllowedError', 'AbortError'].includes(error.name))
				return;
			renameFailure =
				error instanceof MailboxUnreadableError
					? m.proposal.mailboxDown
					: error instanceof SigningRefusedError
						? fillTemplate(m.authority.refusedOn, { network: chainName(error.chainId) })
						: error instanceof SigningUncheckedError
							? fillTemplate(m.authority.signUnchecked, { network: chainName(error.chainId) })
							: error instanceof SigningBlockedError
								? m.review.blocked
								: fillTemplate(m.proposal.failed, {
										reason: error instanceof Error ? error.message : String(error)
									});
		} finally {
			renameBusy = false;
		}
	}

	let sharing = $state(false);
	let shareFailure = $state<string | null>(null);

	async function share() {
		const owner = founders[0];
		if (!owner || sharing) return;
		sharing = true;
		shareFailure = null;
		// The owner's passkey is this device's: the machine is told who is here.
		multisig.send({ type: 'local_accounts', accounts: localAccounts() });
		const phase = await multisig.request(
			{ type: 'share_again', address: account.address, owner: owner.address },
			(view) => (view.share_again[key] !== 'working' ? view.share_again[key] : undefined)
		);
		sharing = false;
		if (phase === 'done') return findRenameChain();
		if (phase !== 'failed') return;
		const passkey = multisig.view?.share_again_failure[key];
		shareFailure =
			passkey === undefined
				? m.add.unavailable
				: fillTemplate(m.proposal.failed, { reason: failureText(passkey) });
	}

	function save() {
		if (backup === 'couldNotCheck') return check();
		if (backup !== 'notBackedUp' || !payer) return;
		multisig.send({ type: 'save_to_ethereum', address: account.address, from: payer });
	}

	const backupLine = $derived(
		{
			checking: '…',
			backedUp: m.settings.backedUp,
			notBackedUp: m.settings.notBackedUp,
			unavailable: m.settings.unavailable,
			couldNotCheck: m.settings.couldNotCheck
		}[backup]
	);
</script>

{#snippet memberList(owners: string[], added: string[], removed: string[])}
	<ul class="members">
		{#each owners as owner (owner)}
			{@const mine = local.some((o) => sameAddress(o.address, owner))}
			{@const isNew = added.some((a) => sameAddress(a, owner))}
			<li>
				<!-- The member's artwork opens the viewer on its address (founder
				     call, 2026-09-05): the row itself is no control. -->
				<Identicon svg={identiconSvgForClient(owner)} size="compact" address={owner} />
				<span class="text">
					<span class="name">{names.label(owner)}</span>
					<!-- A member with no name reads its address once, not twice (ms-26). -->
					{#if names.label(owner) !== shortenAddress(owner)}
						<span class="address">{shortenAddress(owner)}</span>
					{/if}
				</span>
				{#if isNew}<span class="tag" data-testid="member-added">{m.settings.added}</span>{/if}
				{#if mine}<span class="here">{m.panel.thisDevice}</span>{/if}
			</li>
		{/each}
		{#each removed as owner (owner)}
			<li class="gone">
				<Identicon svg={identiconSvgForClient(owner)} size="compact" address={owner} />
				<span class="text">
					<span class="name">{names.label(owner)}</span>
					{#if names.label(owner) !== shortenAddress(owner)}
						<span class="address">{shortenAddress(owner)}</span>
					{/if}
				</span>
				<span class="tag" data-testid="member-removed">{m.settings.removed}</span>
			</li>
		{/each}
	</ul>
{/snippet}

<section class="multisig" aria-labelledby="multisig-members">
	<header>
		<h2 id="multisig-members">{m.panel.members}</h2>
	</header>
	<CustomServiceNote {m} />
	{#if refusedFindings.length > 0}
		<!-- §3.5: the re-check refused its genesis on these networks — read-only
		     there, said for as long as the chain says so. -->
		<AuthorityNotice {m} kind="refused" findings={refusedFindings} readOnlyOn={refusedOn} />
	{/if}
	{#if riskFindings.length > 0}
		<!-- §3.5: more than its members can act for it (accepted when it was added,
		     or found since) — said here for as long as it is on this device. -->
		<AuthorityNotice {m} kind="risk" findings={riskFindings} />
	{/if}
	<div class="set" data-testid="members-original">
		<div class="set-head">
			<h3>{m.settings.originalMembers}</h3>
			<p class="rule">
				{ruleTitle(m, { m: original.threshold, n: original.owners.length })}
			</p>
		</div>
		<p class="note">{m.settings.originalNote}</p>
		{@render memberList(original.owners, [], [])}
	</div>
	{#each deployedOn as c (c.chainId)}
		<div class="set" data-testid="members-current">
			<div class="set-head">
				<h3>{fillTemplate(m.settings.currentOn, { network: chainName(c.chainId) })}</h3>
				<p class="rule">
					{ruleTitle(m, { m: c.members.threshold, n: c.members.owners.length })}
				</p>
			</div>
			{#if c.drift.same}
				<p class="note">{m.settings.sameAsOriginal}</p>
			{:else}
				{@render memberList(c.members.owners, c.drift.added, c.drift.removed)}
				{#if c.drift.thresholdWas !== c.drift.thresholdIs}
					<p class="note">
						{fillTemplate(m.settings.thresholdChanged, {
							was: c.drift.thresholdWas,
							is: c.drift.thresholdIs
						})}
					</p>
				{/if}
			{/if}
		</div>
	{/each}
	{#each undeployedOn as c (c.chainId)}
		<p class="note" data-testid="members-undeployed">
			{fillTemplate(m.settings.undeployedOn, { network: chainName(c.chainId) })}
		</p>
	{/each}
	{#if local.length === 0}
		<p class="note">{m.panel.readOnly}</p>
	{/if}
	<!-- Add, replace, remove, the approvals needed: one proposal per deployed network. -->
	<MemberChanges {m} {account} {cancelLabel} />
	{#if !shared}
		<!-- Its record has not landed: another member cannot add it yet. -->
		<p class="note" role="status">
			{recordWaiting ? m.create.savedLater : m.panel.notShared}
		</p>
		{#if founders.length > 0}
			<!-- An owner here shares it (again), one passkey. r4a-12: the "older
			     registry" guess is said only when no record of it is on its way
			     from this device — never of a creation still sharing. -->
			{#if !recordWaiting}<p class="note">{m.settings.shareAgainNote}</p>{/if}
			<div class="row-actions">
				<Button variant="secondary" shape="rounded" disabled={sharing} onclick={() => void share()}
					>{m.settings.shareAgain}</Button
				>
			</div>
			{#if shareFailure}<p class="error" role="alert">{shareFailure}</p>{/if}
		{/if}
	{/if}
	<div class="row-actions">
		<Button variant="secondary" shape="rounded" onclick={() => multisigUi.open({ kind: 'invite' })}>
			<span class="label"><Icon icon={UTILITY_ICONS['link-2']} size="sm" />{m.panel.invite}</span>
		</Button>
	</div>

	<hr />

	<div class="rename">
		<span class="text">
			<span class="name">{m.settings.renameTitle}</span>
			<span class="state" data-testid="registry-name">{recordedName ?? '…'}</span>
		</span>
		{#if renameChain === null}
			<p class="note" data-testid="rename-blocked">
				{renameBlocked === 'unrecorded'
					? m.settings.renameNotRecorded
					: renameBlocked === 'unreachable'
						? m.add.unavailable
						: m.settings.renameUnavailable}
			</p>
		{:else if renameChain !== undefined && renameDeployed === false}
			<!-- A rename never deploys it: only the deployed multisig renames its record. -->
			<p class="note" data-testid="rename-not-deployed">
				{fillTemplate(m.settings.renameNotDeployed, { network: chainName(renameChain) })}
			</p>
		{:else if renameChain !== undefined && renameDeployed === true}
			{#if renaming}
				<label class="field">
					<span>{m.create.name}</span>
					<input
						class="name-field"
						data-field
						placeholder={m.create.namePlaceholder}
						autocomplete="off"
						bind:value={newName}
					/>
				</label>
				{#if cleanNewName !== '' && !newNameOk}
					<p class="error">{m.create.nameInvalid}</p>
				{/if}
				<div class="row-actions">
					<Button
						variant="value"
						shape="rounded"
						disabled={!newNameOk || cleanNewName === recordedName || renameBusy}
						onclick={() => void rename()}>{m.send.propose}</Button
					>
					<Button variant="secondary" shape="rounded" onclick={() => (renaming = false)}
						>{cancelLabel}</Button
					>
				</div>
				{#if renameFailure}<p class="error" role="alert">{renameFailure}</p>{/if}
			{:else}
				<div class="row-actions">
					<Button
						variant="secondary"
						shape="rounded"
						onclick={() => {
							newName = recordedName ?? '';
							renameFailure = null;
							renaming = true;
						}}>{m.settings.renameAction}</Button
					>
				</div>
			{/if}
			<p class="note">{fillTemplate(m.settings.renameNote, { network: chainName(renameChain) })}</p>
		{/if}
	</div>

	<hr />

	<button
		type="button"
		class="backup"
		disabled={backup === 'checking' || backup === 'backedUp' || backup === 'unavailable'}
		onclick={() => void save()}
	>
		<span class="text">
			<span class="name">{m.settings.backupTitle}</span>
			<span class="state" class:done={backup === 'backedUp'}>
				{#if backup === 'backedUp'}<Icon icon={UTILITY_ICONS.check} size="sm" />{/if}{backupLine}
			</span>
		</span>
		{#if backup === 'notBackedUp' || backup === 'couldNotCheck'}
			<span class="chev"><Icon icon={UTILITY_ICONS['chevron-right']} size="sm" /></span>
		{/if}
	</button>
	<p class="note">{m.settings.backupNote}</p>

	<hr />

	{#if removing}
		<p class="note">{m.switcher.removeBody}</p>
		<div class="row-actions">
			<Button
				variant="danger"
				shape="rounded"
				onclick={() => {
					removing = false;
					multisig.remove(account.address);
				}}>{m.settings.remove}</Button
			>
			<Button variant="secondary" shape="rounded" onclick={() => (removing = false)}
				>{cancelLabel}</Button
			>
		</div>
	{:else}
		<div class="row-actions">
			<Button variant="secondary" shape="rounded" onclick={() => (removing = true)}
				>{m.settings.remove}</Button
			>
		</div>
	{/if}
</section>

<style>
	.multisig {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
		font-family: var(--font-ui);
	}

	header {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-md);
	}

	h2 {
		margin: 0;
		font-size: calc(var(--text-xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	.rule {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.set {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	.set-head {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-md);
	}

	h3 {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.tag {
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.gone .name,
	.gone .address {
		text-decoration: line-through;
		color: var(--color-fg-muted);
	}

	.members {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	.members li {
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

	.here {
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.row-actions {
		display: flex;
		gap: var(--space-sm);
	}

	.row-actions > :global(*) {
		width: auto;
		padding-inline: var(--space-xl);
	}

	.label {
		display: inline-flex;
		align-items: center;
		gap: var(--space-sm);
	}

	hr {
		width: 100%;
		margin: var(--space-md) 0;
		border: 0;
		border-top: var(--border-hairline) solid var(--color-border-base);
	}

	.backup {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		padding: 0;
		border: 0;
		background: none;
		font: inherit;
		text-align: start;
		cursor: pointer;
	}

	.backup:disabled {
		cursor: default;
	}

	.state {
		display: inline-flex;
		align-items: center;
		gap: var(--space-xs);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.state.done {
		color: var(--color-success-text);
	}

	.chev {
		display: flex;
		color: var(--color-fg-subtle);
	}

	.note {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.rename {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.name-field {
		min-height: var(--size-control-lg);
		padding: 0 var(--space-md);
		border: var(--border-hairline) solid var(--color-border-strong);
		border-radius: var(--radius-lg);
		background: var(--color-bg-base);
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	.error {
		margin: 0;
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-error-base);
	}
</style>
