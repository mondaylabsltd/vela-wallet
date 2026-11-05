<script lang="ts">
	import { listOf } from '$lib/i18n/list';
	/**
	 * Adding a multisig by its address — all another member needs. The
	 * genesis comes from the service and is accepted only if the core
	 * re-derives the address from it (`addMultisig`). Its networks are where
	 * it is deployed or has proposals in the mailbox, or what its invite names
	 * (r4D-4) — never where its record is (the registry is an index); when
	 * nothing says, the person picks one here (`chooseNetwork`), the likely
	 * ones first, and then taps Add (r4b-8) — never the first network of the
	 * space, silently, and never added on the pick itself.
	 *
	 * An invite opens this sheet with what it carries — the address, its
	 * networks, its members' labels — and adds nothing until the person's
	 * Add. A damaged invite says so at once (r4b-13).
	 *
	 * Or importing Safes from a Safe{Wallet} export: the file is read by the
	 * core on this device, the person picks the Safes, and each is imported
	 * with a genesis the core re-derives its address from (`importExported`).
	 * r7: before that, each member not on this device is asked its kind —
	 * "Vela account" or "Other wallet", shown as found where Vela can tell
	 * (or a smart contract account), "Other wallet" only hinted — and the
	 * answer kept with each Safe it is a member of: a label, never authority.
	 *
	 * Who can act for it is the chain's, or the core's counterfactual rule
	 * (§3.5): a genesis it refuses is said with every reason and nothing adds
	 * it; a deployed one something besides its members can act for is shown
	 * with what that is, and added only on the person's explicit tap.
	 *
	 * The core's accounts machine does it (`add`, `parse_export`, `import`);
	 * this sheet draws what it answers.
	 */
	import { untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import Button from '$lib/ui/Button.svelte';
	import { addressEntry } from '$lib/core/kernels';
	import { fillTemplate } from '$lib/i18n/fill';
	import { chainName, spaceNetworksSync } from '$lib/services/networks';
	import { balance } from '$lib/wallet/core/balance.svelte';
	import { localName, shortenAddress } from '../core/display';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import type { MultisigMessages } from '../messages';
	import type { Finding } from '../core/refusal';
	import { indexName } from '../core/activity.svelte';
	import { sameAddress, type ExportedSafe } from '../core/kernel';
	import type { AddLink, InvitedMember } from '../core/link';
	import { heldChains, likelyNetworks } from '../core/likely-networks';
	import { names } from '../core/names.svelte';
	import { multisig } from '../core/store.svelte';
	import type { MultisigAddOutcome } from '../generated/MultisigAddOutcome';
	import type { MultisigImportOutcome } from '../generated/MultisigImportOutcome';
	import Identicon from '$lib/wallet/ui/Identicon.svelte';
	import { identiconSvgForClient } from '$lib/wallet/identicon';
	import AuthorityNotice from './AuthorityNotice.svelte';
	import CustomServiceNote from './CustomServiceNote.svelte';
	import MemberKindChoice from './MemberKindChoice.svelte';
	import NetworkChoice from './NetworkChoice.svelte';
	import Sheet from './Sheet.svelte';

	interface Props {
		m: MultisigMessages;
		prefill?: string;
		/**
		 * r4b-13: what the link's own lookup already found (it was not
		 * found, the service away), or that the link is damaged — said at
		 * once; the address is not looked up again until the person taps Add.
		 */
		prefillFailure?: string | null;
		/** The invite it was opened from: its networks, its members' labels. */
		link?: AddLink;
		/**
		 * It was opened from an invite that does not read: said at once —
		 * `checksum` when its address's letters were changed on the way (r5reg-22).
		 */
		broken?: false | 'damaged' | 'checksum';
		/** The page's locale, for the way to Settings → Add network. */
		locale?: string;
		wide: boolean;
		closeLabel: string;
		onclose: () => void;
		onadded: (address: string) => void;
	}

	let {
		m,
		prefill = '',
		prefillFailure = null,
		link,
		broken = false,
		locale = 'en',
		wide,
		closeLabel,
		onclose,
		onadded
	}: Props = $props();

	// The link's address seeds the field once; the person may edit it.
	let address = $state(untrack(() => prefill));
	let busy = $state(false);
	// r4b-13: a damaged invite is said the moment it opens, not at Add; so is
	// what a proposal link's own lookup already found.
	let failure = $state<string | null>(
		untrack(() =>
			broken === 'checksum' ? m.add.badChecksum : broken ? m.add.badInvite : prefillFailure
		)
	);
	// r14-5: what is said is a read that did not come back (the core's
	// `AddOutcome::offers_retry`, `view.add.retry`): Add says Try again.
	let retry = $state(false);
	// The core's one address rule (send-2), checksum included.
	const entry = $derived(addressEntry(address.trim()));
	const valid = $derived(entry === 'valid');

	const networks = spaceNetworksSync();
	/**
	 * The invite this sheet shows, as the accounts machine keeps it
	 * (`invite`): which of its networks this device has, the first it lacks
	 * and that network's name. Adding its multisig uses its networks and
	 * keeps what it says of the members — the machine's, on Add.
	 */
	const invite = $derived.by(() => {
		const kept = multisig.view?.invite ?? null;
		return link && kept && sameAddress(kept.link.multisig, link.multisig) ? kept : null;
	});
	/** The invite's networks this device has, its first network first. */
	const linkHas = $derived(invite?.on_device ?? []);
	/** Its first network, which this device lacks: said, with Add network. */
	const linkLacks = $derived(invite?.missing ?? null);
	/** The chain index's name, for a network the core's catalog does not know. */
	let indexedName = $state<string | null>(null);
	$effect(() => {
		const lacks = linkLacks;
		if (lacks === null || invite?.missing_name) return;
		void indexName(lacks).then((name) => (indexedName = name));
	});
	const lackingName = $derived(invite?.missing_name ?? indexedName);
	/**
	 * r5reg-24: this device's own name for an invited member that is one of
	 * its accounts (the invite names it by address), else `null`.
	 */
	function mine(member: InvitedMember): string | null {
		return member.address ? localName(member.address) : null;
	}
	/** The invite still names this address (the person did not type another). */
	const fromLink = $derived(!!link && address.trim().toLowerCase() === link.multisig.toLowerCase());

	/** Nothing says which network it is used on: the person's pick, asked for. */
	let choosing = $state(false);
	let chosen = $state<number | null>(null);
	/** The core refused its genesis: why, every reason — and nothing adds it. */
	let refused = $state<Finding[] | null>(null);
	/** Deployed, and more than its members can act for it: added only on the person's tap. */
	let risky = $state<Finding[] | null>(null);
	/** The likely networks first (r4D-4): the invite's, the ones used here, where funds are. */
	const offered = $derived(
		likelyNetworks(networks, {
			named: fromLink ? linkHas : [],
			used: multisig.view?.used_networks ?? [],
			held: heldChains(balance.view.tokens)
		})
	);

	async function add(acceptRisks = false) {
		retry = false;
		if (!valid) {
			failure = entry === 'checksum' ? m.send.invalidChecksum : m.send.invalidAddress;
			return;
		}
		if (choosing && chosen === null) return;
		busy = true;
		failure = null;
		refused = null;
		if (!acceptRisks) risky = null;
		// The person's pick when asked; an invite's own network is the machine's.
		const named = choosing && chosen !== null ? chosen : undefined;
		const typed = address.trim();
		const outcome = await multisig.request(
			{ type: 'add', address: typed, network: named ?? null, accept_risks: acceptRisks },
			(view) =>
				view.add.phase === 'done' && sameAddress(view.add.address, typed)
					? view.add.outcome
					: undefined
		);
		busy = false;
		said(outcome);
	}

	function said(outcome: MultisigAddOutcome) {
		switch (outcome.kind) {
			case 'added':
				// The invite's networks and its members' labels landed with it
				// (the accounts machine's, on Add).
				onadded(outcome.address);
				return;
			case 'chooseNetwork':
				choosing = true;
				return;
			case 'refused':
				refused = outcome.findings as Finding[];
				return;
			case 'highRisk':
				risky = outcome.findings as Finding[];
				return;
			case 'unverified':
				failure = fillTemplate(m.authority.unverified, { network: chainName(outcome.chain_id) });
				retry = true;
				return;
			case 'busy':
				// r14-5: the network's nodes were rate-limiting — named, with Try again.
				failure = fillTemplate(m.add.busy, { network: chainName(outcome.chain_id) });
				retry = true;
				return;
			default:
				failure = m.add[outcome.kind];
				retry = multisig.view?.add?.retry ?? false;
		}
	}

	/**
	 * Settings → Add network for the invite's network. The invite waits for
	 * it — the accounts machine's record, through a reload — and its sheet
	 * opens again once the network is added.
	 */
	function addNetwork(chainId: number) {
		// eslint-disable-next-line svelte/no-navigation-without-resolve -- Settings, resolved, with its query
		void goto(`${resolve('/[locale]/settings', { locale })}?addNetwork=${chainId}`);
	}

	// -- importing from Safe{Wallet} ----------------------------------------------

	let exported = $state<ExportedSafe[] | null>(null);
	let picked = $state<string[]>([]);
	let fileFailure = $state<string | null>(null);
	let results = $state<
		Record<
			string,
			| 'busy'
			| 'ok'
			| 'notFound'
			| 'unavailable'
			| 'unsupported'
			| 'notDeployed'
			| 'refused'
			| 'highRisk'
			| 'unverified'
		>
	>({});
	/** What the core found for a Safe it refused, or one only its person's tap imports. */
	let found = $state<Record<string, Finding[]>>({});
	/** Where a Safe could not be checked. */
	let unchecked = $state<Record<string, number>>({});
	const importing = $derived(Object.values(results).some((r) => r === 'busy'));
	const finished = $derived(
		exported !== null &&
			Object.keys(results).length > 0 &&
			!importing &&
			picked.every((a) => results[a] !== undefined)
	);

	async function readFile(event: Event) {
		const file = (event.currentTarget as HTMLInputElement).files?.[0];
		if (!file) return;
		fileFailure = null;
		results = {};
		let text: string;
		try {
			text = await file.text();
		} catch {
			fileFailure = m.import.notExport;
			return;
		}
		// Read by the core, on this device (`parse_export`).
		const read = await multisig.request(
			{ type: 'parse_export', json: text },
			(view) => view.import
		);
		if (read.error !== null) {
			fileFailure = m.import.notExport;
			return;
		}
		const safes = read.safes as ExportedSafe[];
		if (safes.length === 0) {
			fileFailure = m.import.empty;
			return;
		}
		exported = safes;
		// Everything new is picked to start with; one already here is shown as such.
		picked = safes.filter((s) => !multisig.find(s.address)).map((s) => s.address);
	}

	function toggle(address: string) {
		picked = picked.includes(address) ? picked.filter((a) => a !== address) : [...picked, address];
	}

	/** r7: the members' kinds are asked before anything is imported. */
	let asking = $state(false);
	/**
	 * r7: the picked Safes' members not on this device (any of their
	 * networks' owners, once each), with the networks each is a member on —
	 * where code at its address is looked for.
	 */
	const toAsk = $derived.by(() => {
		const out: { address: string; chainIds: number[] }[] = [];
		for (const safe of exported ?? []) {
			if (!picked.includes(safe.address)) continue;
			for (const c of safe.chains) {
				for (const owner of c.owners) {
					if (localName(owner)) continue;
					const seen = out.find((o) => sameAddress(o.address, owner));
					if (!seen) out.push({ address: owner, chainIds: [c.chainId] });
					else if (!seen.chainIds.includes(c.chainId)) seen.chainIds.push(c.chainId);
				}
			}
		}
		return out;
	});

	/**
	 * r7: each member's kind is the accounts machine's: asked for when the
	 * kinds step opens (`read_kinds`, on the networks each is a member on),
	 * picked there (`pick_kind`), and kept with the Safe by the machine when
	 * it is imported — what each was declared as, or Vela found.
	 */
	const kindOf = (address: string) => multisig.view?.member_kinds[address.toLowerCase()] ?? null;

	/**
	 * The pick's next step is the members' kinds — unless every member is on
	 * this device, or they were asked already (an import is being retried).
	 */
	const askFirst = $derived(toAsk.length > 0 && Object.keys(results).length === 0);
	function next() {
		if (!askFirst) return void importPicked();
		multisig.send({
			type: 'read_kinds',
			members: toAsk.map((m) => ({ address: m.address, chain_ids: m.chainIds }))
		});
		asking = true;
	}

	async function importPicked() {
		if (!exported) return;
		asking = false;
		for (const safe of exported.filter((s) => picked.includes(s.address))) {
			if (results[safe.address] === 'ok') continue;
			await importOne(safe);
		}
	}

	async function importOne(safe: ExportedSafe, acceptRisks = false) {
		results = { ...results, [safe.address]: 'busy' };
		const key = safe.address.toLowerCase();
		const outcome: MultisigImportOutcome = await multisig.request(
			{ type: 'import', address: safe.address, accept_risks: acceptRisks },
			(view) => (view.import.busy === null ? view.import.outcomes[key] : undefined)
		);
		if (outcome.kind === 'refused' || outcome.kind === 'highRisk')
			found = { ...found, [safe.address]: outcome.findings as Finding[] };
		if (outcome.kind === 'unverified') {
			unchecked = { ...unchecked, [safe.address]: outcome.chain_id };
		}
		results = { ...results, [safe.address]: outcome.kind === 'imported' ? 'ok' : outcome.kind };
	}

	function rule(safe: ExportedSafe): string {
		const c = safe.chains[0];
		return c ? fillTemplate(m.create.thresholdValue, { m: c.threshold, n: c.owners.length }) : '';
	}
</script>

<Sheet title={m.add.title} {closeLabel} {wide} busy={busy || importing} {onclose}>
	{#if exported && asking}
		<!-- r7: each member's kind, declared before the Safes are imported. -->
		<h3>{m.import.kindsTitle}</h3>
		<p class="hint">{m.import.kindsBody}</p>
		<ul class="members" data-testid="import-kinds">
			{#each toAsk as member (member.address)}
				<li>
					<span class="member">
						<Identicon
							svg={identiconSvgForClient(member.address)}
							size="compact"
							address={member.address}
						/>
						<span class="text"
							><span class="name">{names.label(member.address)}</span
							>{#if names.label(member.address) !== shortenAddress(member.address)}<span
									class="meta">{shortenAddress(member.address)}</span
								>{/if}</span
						>
					</span>
					{#if kindOf(member.address)}
						<MemberKindChoice
							{m}
							choice={kindOf(member.address)!}
							name={names.label(member.address)}
							onpick={(kind) => multisig.send({ type: 'pick_kind', address: member.address, kind })}
						/>
					{/if}
				</li>
			{/each}
		</ul>
		<div class="nav">
			<Button variant="secondary" shape="rounded" onclick={() => (asking = false)}
				>{m.create.back}</Button
			>
			<Button variant="primary" shape="rounded" onclick={() => void importPicked()}
				>{fillTemplate(m.import.importSome, { total: picked.length })}</Button
			>
		</div>
	{:else if exported}
		<h3>{m.import.pick}</h3>
		<ul class="safes">
			{#each exported as safe (safe.address)}
				{@const here = !!multisig.find(safe.address) && results[safe.address] === undefined}
				{@const result = results[safe.address]}
				<li>
					<label class:on={picked.includes(safe.address)}>
						<input
							type="checkbox"
							checked={picked.includes(safe.address)}
							disabled={here || importing || result === 'ok'}
							onchange={() => toggle(safe.address)}
						/>
						<span class="text">
							<span class="name">{safe.name ?? shortenAddress(safe.address)}</span>
							<span class="meta"
								>{shortenAddress(safe.address)} · {rule(safe)} · {safe.chains
									.map(
										(c) => chainName(c.chainId) + (c.deployed ? '' : ` (${m.import.undeployed})`)
									)
									.join(', ')}</span
							>
						</span>
						<span class="state">
							{#if here}
								{m.import.already}
							{:else if result === 'busy'}
								{m.import.importing}
							{:else if result === 'ok'}
								<Icon icon={UTILITY_ICONS.check} size="sm" />{m.import.imported}
							{:else if result === 'refused' || result === 'highRisk'}
								<!-- Said below, in full. -->
							{:else if result === 'unverified'}
								<span class="error"
									>{fillTemplate(m.authority.unverified, {
										network: chainName(unchecked[safe.address] ?? 0)
									})}</span
								>
							{:else if result}
								<span class="error"
									>{result === 'notDeployed' ? m.import.notDeployed : m.add[result]}</span
								>
							{/if}
						</span>
					</label>
					{#if result === 'refused' || result === 'highRisk'}
						<div class="found">
							<AuthorityNotice
								{m}
								kind={result === 'refused' ? 'refused' : 'risk'}
								findings={found[safe.address] ?? []}
							/>
							{#if result === 'highRisk'}
								<Button
									variant="danger"
									shape="rounded"
									disabled={importing}
									onclick={() => void importOne(safe, true)}>{m.authority.riskConfirm}</Button
								>
							{/if}
						</div>
					{/if}
				</li>
			{/each}
		</ul>
		{#if finished}
			<Button variant="primary" shape="rounded" onclick={() => onadded(picked[0] ?? '')}
				>{m.create.done}</Button
			>
		{:else}
			<Button
				variant="primary"
				shape="rounded"
				loading={importing}
				disabled={importing || picked.length === 0}
				onclick={next}
				>{askFirst
					? m.create.next
					: fillTemplate(m.import.importSome, { total: picked.length })}</Button
			>
		{/if}
	{:else}
		<p class="intro">{m.add.intro}</p>
		<CustomServiceNote {m} />
		<form
			onsubmit={(event) => {
				event.preventDefault();
				void add();
			}}
		>
			<input
				class="mono"
				data-field
				placeholder={m.add.placeholder}
				autocomplete="off"
				spellcheck="false"
				bind:value={address}
				oninput={() => {
					failure = null;
					retry = false;
					refused = null;
					risky = null;
					choosing = false;
					chosen = null;
				}}
				aria-invalid={failure !== null || refused !== null}
			/>
			{#if failure}<p class="error" role="alert">{failure}</p>{/if}
			{#if refused}<AuthorityNotice {m} kind="refused" findings={refused} />{/if}
			{#if risky}
				<AuthorityNotice {m} kind="risk" findings={risky} />
				<Button
					variant="danger"
					shape="rounded"
					loading={busy}
					disabled={busy}
					onclick={() => void add(true)}>{m.authority.riskConfirm}</Button
				>
			{/if}
			{#if fromLink && link && (link.chainIds.length > 0 || link.members?.some((x) => x.name || mine(x)))}
				<!-- What the invite says, before anything is added (r4D-4, r4D-5). -->
				<div class="invite" data-testid="invite-summary">
					{#if linkHas.length > 0}
						<p class="line">
							{fillTemplate(m.add.usedOn, { networks: listOf(linkHas.map(chainName), locale) })}
						</p>
					{:else if linkLacks !== null}
						<p class="line" role="status">
							<!-- r13-6: one sentence — the network NAMED in the core's words
							     (was "On Arc Testnet (chain 5042002) — a network this device
							     doesn't have" inside "…which this device doesn't have yet"). -->
							{fillTemplate(m.add.networkMissing, {
								network: lackingName ?? chainName(linkLacks)
							})}
						</p>
						<Button variant="secondary" shape="rounded" onclick={() => addNetwork(linkLacks)}
							>{m.proposal.addNetwork}</Button
						>
					{/if}
					{#if link.members && link.members.length > 0}
						<!-- r8-12: every member the invite names, with its kind — one with no
						     label by its address, never left out. -->
						<p class="line">{m.add.invitedMembers}</p>
						<ul class="named">
							{#each link.members as member, i (i)}
								<!-- r5reg-24: the one of them that is this device's own account, said. -->
								{@const you = mine(member)}
								{@const shown =
									member.name ?? you ?? (member.address ? shortenAddress(member.address) : null)}
								{#if shown}
									<li data-testid={you ? 'invite-you' : 'invite-member'}>
										<span class="who">{shown}</span>
										{#if you}<span class="you">{m.add.thatsYou}</span>{/if}
										{#if member.kind}<span class="kind"
												>{member.kind === 'passkey'
													? m.add.kindPasskey
													: member.kind === 'contract'
														? m.members.kindContract
														: m.add.kindWallet}</span
											>{/if}
									</li>
								{/if}
							{/each}
						</ul>
						<p class="hint">{m.add.labelsNote}</p>
					{/if}
				</div>
			{/if}
			{#if choosing}
				<p class="choose" role="status">{m.add.chooseNetwork}</p>
				<NetworkChoice
					label={m.action.network}
					moreLabel={m.add.moreNetworks}
					networks={offered}
					{chosen}
					disabled={busy}
					onchoose={(id) => (chosen = id)}
				/>
			{/if}
			{#if !risky}
				<Button
					variant="primary"
					shape="rounded"
					loading={busy}
					disabled={busy || refused !== null || (choosing && chosen === null)}
					onclick={() => void add()}
					>{retry && failure ? m.review.state.tryAgain : m.add.add}</Button
				>
			{/if}
		</form>

		<section class="import">
			<h3>{m.import.or}</h3>
			<p class="intro">{m.import.body}</p>
			<label class="file">
				<input type="file" accept="application/json,.json" onchange={readFile} />
				<span>{m.import.choose}</span>
			</label>
			{#if fileFailure}<p class="error" role="alert">{fileFailure}</p>{/if}
		</section>
	{/if}
</Sheet>

<style>
	.intro {
		margin: 0 0 var(--space-lg);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	form {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}

	input {
		min-height: var(--size-control-lg);
		padding: 0 var(--space-md);
		border: var(--border-hairline) solid var(--color-border-strong);
		border-radius: var(--radius-lg);
		background: var(--color-bg-base);
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	h3 {
		margin: 0 0 var(--space-sm);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-muted);
	}

	.import {
		margin-block-start: var(--space-xl);
		padding-block-start: var(--space-lg);
		border-top: var(--border-hairline) solid var(--color-border-base);
	}

	.file {
		display: flex;
		align-items: center;
		justify-content: center;
		min-height: var(--size-control-lg);
		border: var(--border-hairline) solid var(--color-border-strong);
		border-radius: var(--radius-lg);
		font-weight: var(--weight-semibold);
		cursor: pointer;
	}

	.file {
		position: relative;
	}

	/* The whole button is the file input's target, invisible over its label. */
	.file input {
		position: absolute;
		inset: 0;
		opacity: 0;
		cursor: pointer;
	}

	.safes {
		list-style: none;
		margin: 0 0 var(--space-lg);
		padding: 0;
	}

	.safes label {
		display: grid;
		grid-template-columns: auto 1fr;
		align-items: start;
		column-gap: var(--space-md);
		row-gap: var(--space-xs);
		padding-block: var(--space-sm);
		border-bottom: var(--border-hairline) solid var(--color-border-base);
	}

	.safes .text {
		display: flex;
		flex-direction: column;
		flex: 1;
		min-width: 0;
	}

	.safes .name {
		font-weight: var(--weight-semibold);
		overflow-wrap: anywhere;
	}

	.safes .meta {
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		overflow-wrap: anywhere;
	}

	/* Under the text, never beside it: a reason must not squeeze the name. */
	.safes .state {
		grid-column: 2;
		display: inline-flex;
		align-items: center;
		gap: var(--space-xs);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.safes .state:empty {
		display: none;
	}

	/* r7: the members' kinds, one member a row — who, then how they approve. */
	.members {
		list-style: none;
		margin: var(--space-md) 0 var(--space-lg);
		padding: 0;
	}

	.members li {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
		padding-block: var(--space-md);
		border-bottom: var(--border-hairline) solid var(--color-border-base);
	}

	.member {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		min-width: 0;
	}

	.member .text {
		display: flex;
		flex-direction: column;
		min-width: 0;
	}

	.member .name {
		font-weight: var(--weight-semibold);
		overflow-wrap: anywhere;
	}

	.member .meta {
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.nav {
		display: flex;
		gap: var(--space-md);
	}

	.nav > :global(*) {
		flex: 1;
	}

	.error {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-error-base);
	}

	.found {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
		padding-block-end: var(--space-sm);
	}

	.invite {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
		padding: var(--space-md);
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
	}

	.line,
	.hint {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-base);
	}

	.hint {
		color: var(--color-fg-muted);
	}

	.named {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	.named li {
		display: flex;
		align-items: baseline;
		gap: var(--space-sm);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
	}

	.who {
		font-weight: var(--weight-semibold);
		overflow-wrap: anywhere;
	}

	.kind {
		color: var(--color-fg-muted);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
	}

	.you {
		color: var(--color-fg-base);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
	}

	.choose {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-base);
	}
</style>
