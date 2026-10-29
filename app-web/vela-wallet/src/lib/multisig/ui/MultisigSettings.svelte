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
	 */
	import { onMount } from 'svelte';
	import Button from '$lib/ui/Button.svelte';
	import { fillTemplate } from '$lib/i18n/fill';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import Identicon from '$lib/wallet/ui/Identicon.svelte';
	import { identiconSvgForClient } from '$lib/wallet/identicon';
	import { shortenAddress } from '$lib/wallet/identity';
	import type { MultisigMessages } from '../messages';
	import { recordHomeChain, shareAgain } from '../core/accounts';
	import { checkBackup, startBackup, type BackupState } from '../core/backup';
	import { chainName } from '../core/display';
	import { validName } from '../core/kernel';
	import { names } from '../core/names.svelte';
	import { localOwners } from '../core/owners';
	import { MailboxUnreadableError, proposals, proposeRename } from '../core/proposals.svelte';
	import { fetchGenesis } from '../core/service';
	import { networkForChainId } from '$lib/services/networks';
	import { isShared, multisig, type MultisigAccount } from '../core/store.svelte';
	import { multisigUi } from '../core/ui.svelte';
	import CustomServiceNote from './CustomServiceNote.svelte';

	interface Props {
		m: MultisigMessages;
		account: MultisigAccount;
		/** A Vela account on this device that pays for the Ethereum record. */
		payer: string | null;
		cancelLabel: string;
	}

	let { m, account, payer, cancelLabel }: Props = $props();

	/** The members now: a chain it is deployed on, else its genesis's. */
	const members = $derived.by(() => {
		const c = proposals.chains.find((x) => x.members.deployed) ?? proposals.chains[0];
		return c
			? { owners: c.members.owners, threshold: c.members.threshold }
			: { owners: account.owners, threshold: account.threshold };
	});
	const local = $derived(localOwners(members.owners));

	let backup = $state<BackupState>('checking');
	let removing = $state(false);

	async function check() {
		backup = 'checking';
		backup = await checkBackup(account);
	}

	/**
	 * Where its record can be renamed: a registry chain holding it that this
	 * wallet has a network for — the service's home chain first, where it
	 * writes records. `null`: none (or not recorded); `undefined`: not known yet.
	 */
	let renameChain = $state<number | null | undefined>(undefined);

	async function findRenameChain() {
		try {
			const answer = await fetchGenesis(account.address);
			const home = await recordHomeChain();
			const held = (answer.found && !answer.pending ? (answer.recordedOn ?? []) : []).filter(
				(id) => networkForChainId(id) !== null
			);
			renameChain = held.includes(home) ? home : (held[0] ?? null);
		} catch {
			renameChain = null;
		}
	}

	onMount(() => {
		void check();
		void findRenameChain();
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
		if (!newNameOk || renameChain == null || cleanNewName === recordedName) return;
		const chainId = renameChain;
		renameBusy = true;
		renameFailure = null;
		try {
			const { safeTxHash } = await proposeRename(account.address, chainId, cleanNewName);
			renaming = false;
			multisigUi.open({ kind: 'proposal', chainId, safeTxHash });
		} catch (error) {
			renameFailure =
				error instanceof MailboxUnreadableError
					? m.proposal.mailboxDown
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
		const owner = local[0];
		if (!owner || sharing) return;
		sharing = true;
		shareFailure = null;
		try {
			if (await shareAgain(multisig.find(account.address) ?? account, owner)) {
				void findRenameChain();
			} else {
				shareFailure = m.add.unavailable;
			}
		} catch (error) {
			shareFailure = fillTemplate(m.proposal.failed, {
				reason: error instanceof Error ? error.message : String(error)
			});
		} finally {
			sharing = false;
		}
	}

	async function save() {
		if (backup === 'couldNotCheck') return void check();
		if (backup !== 'notBackedUp' || !payer) return;
		const settled = await startBackup(account, payer);
		if (settled.kind === 'submitted') backup = 'backedUp';
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

<section class="multisig" aria-labelledby="multisig-members">
	<header>
		<h2 id="multisig-members">{m.panel.members}</h2>
		<p class="rule">
			{fillTemplate(m.panel.title, { m: members.threshold, n: members.owners.length })}
		</p>
	</header>
	<CustomServiceNote {m} />
	<ul class="members">
		{#each members.owners as owner (owner)}
			{@const mine = local.some((o) => o.address.toLowerCase() === owner.toLowerCase())}
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
				{#if mine}<span class="here">{m.panel.thisDevice}</span>{/if}
			</li>
		{/each}
	</ul>
	{#if local.length === 0}
		<p class="note">{m.panel.readOnly}</p>
	{/if}
	{#if !isShared(multisig.find(account.address) ?? account)}
		<!-- Its record has not landed: another member cannot add it yet. -->
		<p class="note" role="status">{m.panel.notShared}</p>
		{#if local.length > 0}
			<!-- The service has no record of it (an earlier registry's, or one that
			     never landed): an owner here shares it again, one passkey. -->
			<p class="note">{m.settings.shareAgainNote}</p>
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
			<p class="note">{m.settings.renameUnavailable}</p>
		{:else if renameChain !== undefined}
			{#if renaming}
				<label class="field">
					<span>{m.create.name}</span>
					<input
						class="name-field"
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
		color: var(--color-fg-subtle);
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
		color: var(--color-success-base);
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
