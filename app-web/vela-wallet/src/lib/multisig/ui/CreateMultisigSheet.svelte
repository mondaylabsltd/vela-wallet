<script lang="ts">
	import { listOf } from '$lib/i18n/list';
	/**
	 * Creating a multisig, in three steps: its name and who its members are
	 * (in order — the order is part of the address; the name is a label
	 * written with its record), how many must approve, and a review that
	 * shows the address before anything is signed. One passkey prompt at
	 * the end: the creator's proof, with which the genesis service records the
	 * multisig. Nothing is deployed and no fee is paid until its first
	 * transfer out.
	 *
	 * The record goes to the registry's index chain (the service's home,
	 * Gnosis) whatever network is picked: the multisig is the same on every
	 * chain with the Safe v1.4.1 contracts. So every network the person has
	 * is offered but one the core finds without them (`safeContractsOn`); the
	 * service's own chain table bounds nothing here.
	 *
	 * r4a-14: one member is a multisig too (1 of 1, a valid Safe — said what
	 * it means); members come from this device, the contacts, an address
	 * (checksummed as it is added) or an ENS name (shown as the address it
	 * gives, which is what is added); each may carry a name — a label kept
	 * with the multisig and carried by its invite (r4D-5), never authority.
	 * The first network is the one the person last used a multisig on, or
	 * holds funds on — never one picked for them when nothing says.
	 *
	 * r4a-13: the moment the passkey answers, the multisig is kept on this
	 * device; the created screen says, honestly, whether it is shared yet.
	 *
	 * The core's accounts machine does it (`new_draft` — the salt, fixed per
	 * draft; `draft` — the address the review shows; `check_networks`;
	 * `create` — one passkey prompt, the account kept, then its record); this
	 * sheet draws its `create` view.
	 */
	import { onMount, untrack } from 'svelte';
	import Button from '$lib/ui/Button.svelte';
	import Dropdown from '$lib/settings/ui/Dropdown.svelte';
	import { contactsBook } from '$lib/contacts/core/contacts-book.svelte';
	import { checksumAddress } from '$lib/core/kernels';
	import { fillTemplate } from '$lib/i18n/fill';
	import { pluralForm } from '$lib/i18n/plural';
	import { loadAccounts } from '$lib/onboarding/core/storage';
	import { networkSpace } from '$lib/network-space/network-space.svelte';
	import { isHiddenNetwork, spaceNetworksSync } from '$lib/services/networks';
	import { chainFilter } from '$lib/wallet/chain-filter.svelte';
	import { balance } from '$lib/wallet/core/balance.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import Identicon from '$lib/wallet/ui/Identicon.svelte';
	import { identiconSvgForClient } from '$lib/wallet/identicon';
	import type { MultisigMessages } from '../messages';
	import { chainName, localName, ruleTitle, shortenAddress } from '../core/display';
	import { looksLikeEns, resolveEns, type EnsAnswer } from '../core/ens-name';
	import { inviteLink } from '../core/invite';
	import { sameAddress, validName } from '../core/kernel';
	import { MEMBER_NAME_MAX, shareOrigin } from '../core/link';
	import { firstNetworkDefault, heldChains, likelyNetworks } from '../core/likely-networks';
	import { localOwners } from '../core/owners';
	import { memberProblem } from '../core/members';
	import { names } from '../core/names.svelte';
	import { multisig, type MemberInfo } from '../core/store.svelte';
	import { failureText } from '../shell/failure';
	import { localAccounts } from '../shell/local-accounts';
	import CustomServiceNote from './CustomServiceNote.svelte';
	import MemberKindChoice from './MemberKindChoice.svelte';
	import NetworkChoice from './NetworkChoice.svelte';
	import Sheet from './Sheet.svelte';

	interface Props {
		m: MultisigMessages;
		locale: string;
		/** The Vela account the wallet shows: a member from the start. */
		current: string | null;
		wide: boolean;
		closeLabel: string;
		onclose: () => void;
		oncreated: (address: string) => void;
	}

	let { m, locale, current, wide, closeLabel, onclose, oncreated }: Props = $props();

	const accounts = loadAccounts();
	const spaceNetworks = spaceNetworksSync();

	// A new draft (its salt, fixed for this sheet), and which networks hold
	// the Safe contracts a new multisig needs.
	multisig.send({ type: 'new_draft' });
	multisig.send({ type: 'check_networks', chain_ids: spaceNetworks.map((n) => n.chainId) });
	/** The machine's create sheet. */
	const sheet = $derived(multisig.view?.create ?? null);

	/** The networks the core found without the Safe contracts a multisig needs. */
	const without = $derived(
		Object.entries(sheet?.networks ?? {})
			.filter(([, verdict]) => (verdict as { kind?: string } | null)?.kind === 'missing')
			.map(([id]) => Number(id))
	);
	// r5reg-19: read as they come — the wallet's network now, then where the
	// multisigs here and this device's own transactions were (the accounts
	// machine's: `used_networks`, `recent_networks`), then where it holds
	// funds (balances land after the sheet opens; Arc read under "More
	// networks" when they had not yet).
	const hints = $derived({
		here: chainFilter.chainId === null ? [] : [chainFilter.chainId],
		used: multisig.view?.used_networks ?? [],
		recent: multisig.view?.recent_networks ?? [],
		held: heldChains(balance.view.tokens)
	});
	// A network that did not answer stays offered: nothing is deployed until
	// the first transfer out, and nothing says it lacks them. The likely ones
	// first (r4a-14).
	const networks = $derived(
		likelyNetworks(
			spaceNetworks.filter((n) => !without.includes(n.chainId)),
			hints
		)
	);

	let step = $state<'members' | 'threshold' | 'review' | 'created'>('members');
	// The account in front seeds the members once; the person edits from there.
	let owners = $state<string[]>(
		untrack(() => (current ? [current] : accounts.slice(0, 1).map((a) => a.address)))
	);
	/** What the person said of each member added by address, contact or name (r4D-5). */
	let info = $state<Record<string, MemberInfo>>({});
	let typed = $state('');
	let typedName = $state('');
	let typedError = $state<string | null>(null);
	/** An ENS name typed, and what Ethereum said it points to (r4D-5). */
	let ens = $state<EnsAnswer | null>(null);
	let ensBusy = $state(false);
	let picking = $state(false);
	/**
	 * How many must approve — the accounts machine's (`create.threshold`):
	 * the sensible start for these members, or the person's pick for them
	 * (`step_threshold`).
	 */
	const thresholdView = $derived(sheet?.threshold ?? null);
	const threshold = $derived(thresholdView?.value ?? 1);
	let name = $state('');
	/** Next was pressed with no name: the field says so (r4a-14). */
	let nameAsked = $state(false);
	// r4a-14: the network the person last used, or holds funds on — never
	// Sepolia by accident; nothing, when nothing says (the person picks).
	let chainId = $state<number | null>(untrack(() => firstNetworkDefault(spaceNetworks, hints)));
	// A network found without the Safe contracts is not the pick.
	$effect(() => {
		if (chainId !== null && !networks.some((n) => n.chainId === chainId)) chainId = null;
	});
	/** The person picked a network: what lands later never picks over them. */
	let chose = $state(false);
	$effect(() => {
		const likely = firstNetworkDefault(networks, hints);
		untrack(() => {
			if (!chose && chainId === null && likely !== null) chainId = likely;
		});
	});
	let creatorChoice = $state<string | null>(null);
	/** vis-19: the review's creator menu. */
	let menu = $state<'creator' | null>(null);
	let busy = $state(false);
	let failure = $state<string | null>(null);
	let created = $state<string | null>(null);
	/** r4a-13: the record is being asked for (the multisig is already kept here). */
	let sharing = $state(false);
	/** The service refused the record: said, with what it said. */
	let refused = $state<string | null>(null);
	let copied = $state(false);

	onMount(() => {
		// The address book, for the contacts picker (read once per app).
		void contactsBook.ensure(current);
	});

	/**
	 * r7-9: what was entered before is the machine's draft (kept until it is
	 * created or discarded): the sheet opens on it — once the machine has read
	 * the store, and only over a sheet nobody has typed in yet. Until then
	 * nothing is kept, so a blank sheet never replaces it.
	 */
	let draftRead = $state(false);
	/** The sheet opened on a kept draft: said, with Start over. */
	let resumed = $state(false);
	$effect(() => {
		const view = multisig.view;
		if (!view || draftRead) return;
		untrack(() => {
			const kept = view.create.kept;
			if (kept && name.trim() === '' && owners.length <= 1) {
				owners = [...kept.owners];
				info = Object.fromEntries(
					Object.entries(kept.members)
						.filter(([, i]) => !!i.name)
						.map(([a, i]) => [a.toLowerCase(), { name: i.name ?? undefined }])
				);
				name = kept.name;
				// Its threshold is the machine's pick again (`create.threshold`).
				if (kept.chainId !== null) {
					chainId = kept.chainId;
					chose = true;
				}
				resumed = true;
			}
			draftRead = true;
		});
	});
	// Every change is kept by the machine (a blank sheet keeps nothing).
	$effect(() => {
		if (!draftRead || step === 'created') return;
		const members = Object.fromEntries(
			owners.flatMap((o) => {
				const said = info[o.toLowerCase()]?.name;
				return said ? [[o, { name: said }]] : [];
			})
		);
		multisig.send({
			type: 'keep_draft',
			draft: { owners: [...owners], members, name, threshold, chainId: chose ? chainId : null }
		});
	});

	/** r7-9: start over — the kept draft goes, the sheet is fresh. */
	function discardDraft() {
		multisig.send({ type: 'discard_draft' });
		owners = current ? [current] : accounts.slice(0, 1).map((a) => a.address);
		info = {};
		name = '';
		nameAsked = false;
		chose = false;
		chainId = firstNetworkDefault(networks, hints);
		resumed = false;
	}

	/**
	 * r7-9: this device's networks of the other space (testnets on mainnet,
	 * and back), which this sheet does not offer — said, with the way there.
	 */
	const otherSpace = $derived(
		(sheet?.other_space ?? []).filter((id) => !isHiddenNetwork(id)).map(chainName)
	);
	const spaceWords = $derived(networkSpace.words);

	const local = $derived(localOwners(owners));
	const creator = $derived(local.find((o) => o.address === creatorChoice) ?? local[0] ?? null);
	const cleanName = $derived(name.trim());
	/** r4a-14: the name's size in the registry's unit — bytes, not characters. */
	const nameBytes = $derived(new TextEncoder().encode(cleanName).length);
	const NAME_LIMIT = 64;
	const nameOk = $derived.by(() => {
		try {
			return cleanName !== '' && validName(cleanName);
		} catch {
			return false;
		}
	});
	/** r4a-14: what is wrong with the name, said of the name (not one catch-all). */
	const nameError = $derived(
		cleanName === ''
			? nameAsked
				? m.create.nameNeeded
				: null
			: nameBytes > NAME_LIMIT
				? fillTemplate(m.create.nameTooLong, { bytes: nameBytes, limit: NAME_LIMIT })
				: nameOk
					? null
					: m.create.nameInvalid
	);
	const membersOk = $derived(owners.length >= 1 && local.length >= 1 && nameOk);
	// The draft's address, as the members, the rule and the name change.
	$effect(() => {
		if (owners.length === 0 || !nameOk) return;
		multisig.send({ type: 'draft', owners: [...owners], threshold, name: cleanName });
	});
	const draft = $derived(
		owners.length > 0 && nameOk && sheet?.draft_address ? { address: sheet.draft_address } : null
	);

	/**
	 * r7: a member's kind — the accounts machine's (`read_kinds`, `pick_kind`,
	 * its view's `member_kinds`): what the chains say of it on every network
	 * it may be used on (Vela's passkey signer configured for it: a Vela
	 * account; other code there: a smart contract account), else the person's
	 * word (here, or kept on this device), else asked — "Other wallet" hinted
	 * when no Vela account is found, kept only if picked. The same address on
	 * every network. Each member is read once for the networks offered.
	 */
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- what was asked, never drawn
	const asked = new Set<string>();
	$effect(() => {
		const chainIds = networks.map((n) => n.chainId);
		const members = owners
			.filter((o) => !localName(o))
			.map((address) => ({ address, chain_ids: chainIds }))
			.filter((m) => !asked.has(`${m.address.toLowerCase()}:${chainIds.join(',')}`));
		if (members.length === 0) return;
		for (const m of members) asked.add(`${m.address.toLowerCase()}:${chainIds.join(',')}`);
		multisig.send({ type: 'read_kinds', members });
	});
	const kindOf = (address: string) => multisig.view?.member_kinds[address.toLowerCase()] ?? null;

	/** A member's label here: this device's account name, what the person typed, else the index's. */
	function label(address: string): string {
		return localName(address) ?? info[address.toLowerCase()]?.name ?? names.label(address);
	}

	function toggle(address: string) {
		owners = owners.some((o) => sameAddress(o, address))
			? owners.filter((o) => !sameAddress(o, address))
			: [...owners, address];
	}

	/** Add `address` (any case) as a member, checksummed (r4a-14), with what is known of it. */
	function addMember(address: string, memberName?: string | null): boolean {
		const problem = memberProblem(address, owners);
		if (problem) {
			typedError = {
				invalidAddress: m.send.invalidAddress,
				checksum: m.send.invalidChecksum,
				notMember: m.create.notMember,
				duplicate: m.create.duplicate
			}[problem];
			return false;
		}
		const member = checksumAddress(address);
		const clean = memberName?.trim().slice(0, MEMBER_NAME_MAX);
		if (clean) info = { ...info, [member.toLowerCase()]: { name: clean } };
		owners = [...owners, member];
		typed = '';
		typedName = '';
		typedError = null;
		ens = null;
		return true;
	}

	async function addTyped() {
		const value = typed.trim();
		// r4D-5: a name is read on Ethereum and shown as its address first.
		if (looksLikeEns(value)) {
			ensBusy = true;
			typedError = null;
			ens = await resolveEns(value);
			ensBusy = false;
			return;
		}
		addMember(value, typedName);
	}

	/** The contacts that could be members: not here already. */
	const contacts = $derived(
		(contactsBook.view?.contacts ?? []).filter(
			(c) => /^0x[0-9a-fA-F]{40}$/.test(c.address) && !owners.some((o) => sameAddress(o, c.address))
		)
	);

	function toThreshold() {
		if (!membersOk) {
			nameAsked = true;
			return;
		}
		// Where it starts is the machine's (`create.threshold`): sensible for
		// these members, unless the person picked for them already.
		step = 'threshold';
	}

	/** What the threshold means — the machine's note, in the corpus' words. */
	const thresholdNote = $derived.by(() => {
		const note = thresholdView?.note;
		switch (note?.kind) {
			case 'all':
				return m.create.thresholdAll;
			case 'one':
				return m.create.thresholdOne;
			case 'spare':
				return fillTemplate(pluralForm(m.create.thresholdSpare, note.count), {
					count: note.count
				});
			default:
				return m.create.solo;
		}
	});
	const risky = $derived(thresholdView?.risky ?? false);

	/** What the person knows of the members, for the multisig here and its invite (r4D-5). */
	function membersInfo(): Record<string, MemberInfo> {
		return Object.fromEntries(
			owners.map((o) => {
				// Each member's kind is the machine's, kept with it as chosen.
				const said = info[o.toLowerCase()]?.name;
				return [o, said ? { name: said } : {}];
			})
		);
	}

	async function create() {
		if (!creator || !draft || !nameOk || chainId === null) return;
		busy = true;
		failure = null;
		// The creator's passkey is this device's: the machine is told who is here.
		multisig.send({ type: 'local_accounts', accounts: localAccounts() });
		const first = await multisig.request(
			{
				type: 'create',
				choice: {
					owners: [...owners],
					threshold,
					name: cleanName,
					chain_id: chainId,
					creator: creator.address,
					members: membersInfo()
				}
			},
			// Kept here (the created screen now, whatever the service does), or
			// not created at all.
			(view) => (view.create.address || view.create.phase !== 'working' ? view.create : undefined)
		);
		busy = false;
		if (first.address) {
			created = first.address;
			step = 'created';
			sharing = first.phase === 'working';
			const done = await multisig.until((view) =>
				view.create.phase !== 'working' ? view.create : undefined
			);
			refused = done.refused;
			sharing = false;
			return;
		}
		// The person closed the passkey: nothing to say.
		if (first.phase === 'failed') failure = failureText(first.error);
	}

	/** The created multisig as the machine holds it now (its sharing moves on its own). */
	const kept = $derived(created ? (multisig.find(created) ?? null) : null);
	const keptRow = $derived(created ? multisig.row(created) : null);
	/** r5reg-18: a multisig of one — no other members to send anything to. */
	const solo = $derived(!!kept && kept.owners.length === 1);
	const sharedState = $derived(
		!kept
			? null
			: keptRow?.shared
				? solo
					? m.create.sharedSolo
					: m.create.shared
				: refused
					? fillTemplate(m.create.refused, { reason: refused })
					: sharing
						? m.create.savedSharing
						: m.create.savedLater
	);

	async function copy() {
		if (!kept) return;
		try {
			// What the button says: the invite link — its networks and members ride along.
			await navigator.clipboard.writeText(inviteLink(shareOrigin(location.origin), locale, kept));
			// S4 (r8): the invite was handed on from here: the home's first step ticks.
			multisig.inviteShared(kept.address);
			copied = true;
			setTimeout(() => (copied = false), 1600);
		} catch {
			// the address stays on screen
		}
	}
</script>

<Sheet
	title={m.create.title}
	{closeLabel}
	{wide}
	{busy}
	onclose={step === 'created' && created ? () => oncreated(created ?? '') : onclose}
>
	{#if step === 'members'}
		<p class="intro">{m.create.intro}</p>
		{#if resumed}
			<!-- r7-9: what was entered before, kept by the machine: said, and undone in one tap. -->
			<div class="resumed" role="status" data-testid="draft-kept">
				<p class="hint flush">{m.create.draftKept}</p>
				<button type="button" class="text-button" onclick={discardDraft}
					>{m.create.discardDraft}</button
				>
			</div>
		{/if}
		<!-- First, as a Vela account's name is: it is part of the address. -->
		<label class="select">
			<!-- S4: the byte count only where it matters — near or over the limit. -->
			<span class="field-head"
				><span>{m.create.name}</span>{#if nameBytes >= NAME_LIMIT - 8}<span
						class="count"
						class:over={nameBytes > NAME_LIMIT}
						data-testid="name-bytes"
						>{fillTemplate(m.create.nameBytes, { bytes: nameBytes, limit: NAME_LIMIT })}</span
					>{/if}</span
			>
			<input
				class="name-field"
				data-field
				placeholder={m.create.namePlaceholder}
				autocomplete="off"
				bind:value={name}
				aria-invalid={nameError !== null}
			/>
		</label>
		{#if nameError}
			<p class="error" data-testid="name-error">{nameError}</p>
		{:else}
			<p class="hint">{m.create.nameHint}</p>
		{/if}
		<h3>{m.create.yourAccounts}</h3>
		<ul class="choices">
			{#each accounts as a (a.address)}
				{@const on = owners.some((o) => sameAddress(o, a.address))}
				<li>
					<label class:on>
						<input type="checkbox" checked={on} onchange={() => toggle(a.address)} />
						<!-- A picture, not the viewer's button: the whole row is the
						     checkbox's label, and a button inside it would be nested
						     interactive content (and steal the row's tap). -->
						<Identicon svg={identiconSvgForClient(a.address)} size="row" />
						<span class="text"
							><span class="name">{a.name || shortenAddress(a.address)}</span><span class="address"
								>{shortenAddress(a.address)}</span
							></span
						>
						<span class="box" aria-hidden="true"
							>{#if on}<Icon icon={UTILITY_ICONS.check} size="sm" />{/if}</span
						>
					</label>
				</li>
			{/each}
		</ul>

		<h3>{m.create.byAddress}</h3>
		<div class="typed">
			<input
				class="mono"
				data-field
				placeholder={m.create.addressPlaceholder}
				autocomplete="off"
				spellcheck="false"
				bind:value={typed}
				oninput={() => {
					typedError = null;
					ens = null;
				}}
				onkeydown={(e) => e.key === 'Enter' && (e.preventDefault(), void addTyped())}
			/>
		</div>
		<div class="typed">
			<input
				class="name-field small"
				placeholder={m.create.memberNamePlaceholder}
				aria-label={m.create.memberName}
				autocomplete="off"
				maxlength={MEMBER_NAME_MAX}
				bind:value={typedName}
				onkeydown={(e) => e.key === 'Enter' && (e.preventDefault(), void addTyped())}
			/>
			<Button
				variant="secondary"
				shape="rounded"
				loading={ensBusy}
				disabled={ensBusy || typed.trim() === ''}
				onclick={() => void addTyped()}>{m.create.addMember}</Button
			>
		</div>
		{#if typedError}<p class="error" role="alert">{typedError}</p>{/if}
		{#if ens}
			<!-- r4D-5: the name's address, in full and first; the name is only its label. -->
			<div class="ens" role="status" data-testid="ens-answer">
				{#if ens.kind === 'found'}
					{@const found = ens}
					<p class="line">{fillTemplate(m.create.ensResolved, { name: found.name })}</p>
					<p class="ens-address">{found.address}</p>
					<Button
						variant="secondary"
						shape="rounded"
						onclick={() => addMember(found.address, typedName.trim() || found.name)}
						>{m.create.addMember}</Button
					>
				{:else if ens.kind === 'none'}
					<p class="line">{fillTemplate(m.create.ensNotFound, { name: ens.name })}</p>
				{:else if ens.kind === 'unsupported'}
					<p class="line">{m.create.ensUnsupported}</p>
				{:else}
					<p class="line">{fillTemplate(m.create.ensUnavailable, { name: ens.name })}</p>
				{/if}
			</div>
		{/if}
		{#if contacts.length > 0}
			<button
				type="button"
				class="pick"
				aria-expanded={picking}
				onclick={() => (picking = !picking)}
			>
				<Icon icon={UTILITY_ICONS['user-round']} size="sm" />{m.create.fromContacts}
			</button>
			{#if picking}
				<ul class="contacts" data-testid="contacts-picker">
					{#each contacts as c (c.address)}
						<li>
							<button
								type="button"
								onclick={() => {
									if (addMember(c.address, c.name ?? c.resolved_name)) picking = false;
								}}
							>
								<Identicon svg={identiconSvgForClient(c.address)} size="row" />
								<span class="text"
									><span class="name">{c.name ?? c.resolved_name ?? shortenAddress(c.address)}</span
									><span class="address">{shortenAddress(checksumAddress(c.address))}</span></span
								>
								<Icon icon={UTILITY_ICONS.plus} size="sm" />
							</button>
						</li>
					{/each}
				</ul>
			{/if}
		{/if}

		{#if owners.length > 0}
			<h3>{m.create.members} · {owners.length}</h3>
			<ol class="owners">
				{#each owners as o, i (o)}
					<li>
						<span class="ordinal">{i + 1}</span>
						<Identicon svg={identiconSvgForClient(o)} size="row" address={o} />
						<span class="text"
							><span class="name">{label(o)}</span>{#if label(o) !== shortenAddress(o)}<span
									class="address">{shortenAddress(o)}</span
								>{/if}</span
						>
						<!-- vis-16: named for what it does, with whom (it read "Close"). -->
						<button
							type="button"
							class="remove"
							aria-label="{m.action.removeOwner} · {label(o)}"
							onclick={() => toggle(o)}
						>
							<Icon icon={UTILITY_ICONS['x']} size="sm" />
						</button>
						{#if !localName(o) && kindOf(o)}
							<!-- r7: how they approve, declared — Vela's finding where it has one. -->
							<div class="member-kind">
								<MemberKindChoice
									{m}
									choice={kindOf(o)!}
									name={label(o)}
									onpick={(kind) => multisig.send({ type: 'pick_kind', address: o, kind })}
								/>
							</div>
						{/if}
					</li>
				{/each}
			</ol>
			<p class="hint">{m.create.order}</p>
		{/if}
		{#if owners.length === 1 && local.length === 1}<p class="hint" data-testid="solo-hint">
				{m.create.solo}
			</p>{/if}
		{#if owners.length >= 1 && local.length === 0}<p class="error">{m.create.needLocal}</p>{/if}

		<div class="nav">
			<Button
				variant="primary"
				shape="rounded"
				disabled={owners.length === 0 || local.length === 0}
				onclick={toThreshold}>{m.create.next}</Button
			>
		</div>
	{:else if step === 'threshold'}
		<h3>{m.create.threshold}</h3>
		<div class="stepper" role="group" aria-label={m.create.threshold}>
			<button
				type="button"
				aria-label={m.create.fewer}
				disabled={!thresholdView?.can_lower}
				onclick={() => multisig.send({ type: 'step_threshold', up: false })}>−</button
			>
			<output>{fillTemplate(m.create.thresholdValue, { m: threshold, n: owners.length })}</output>
			<button
				type="button"
				aria-label={m.create.more}
				disabled={!thresholdView?.can_raise}
				onclick={() => multisig.send({ type: 'step_threshold', up: true })}>+</button
			>
		</div>
		<p class="note" class:risky>{thresholdNote}</p>
		<!-- Passkey-native recovery: a lost passkey's member is replaced by the others. -->
		{#if threshold < owners.length}
			<p class="hint" data-testid="create-recovery">
				{fillTemplate(m.members.recovery, { m: threshold, n: owners.length })}
			</p>
		{/if}
		<div class="nav two">
			<Button variant="secondary" shape="rounded" onclick={() => (step = 'members')}
				>{m.create.back}</Button
			>
			<Button variant="primary" shape="rounded" onclick={() => (step = 'review')}
				>{m.create.next}</Button
			>
		</div>
	{:else if step === 'review'}
		{#if draft}
			<div class="hero">
				<span class="hero-avatar"
					><Identicon
						svg={identiconSvgForClient(draft.address)}
						size="detail"
						address={draft.address}
					/></span
				>
				<p class="hero-name">{cleanName}</p>
				<p class="hero-rule">{ruleTitle(m, { m: threshold, n: owners.length })}</p>
				<p class="hero-address">{checksumAddress(draft.address)}</p>
				<p class="hint">{m.create.sameEverywhere}</p>
			</div>
		{/if}
		<h3>{m.create.members}</h3>
		<ol class="owners compact">
			{#each owners as o, i (o)}
				{@const named = label(o) !== shortenAddress(o)}
				<li>
					<!-- r4a-14 / r8: every member reads the same way here, where it is
					     checked before the passkey signs — its name when it has one, and
					     its address in full (EIP-55), never a short form beside a long one. -->
					<span class="ordinal">{i + 1}</span><span class="text"
						>{#if named}<span class="name">{label(o)}</span>{/if}<span
							class="address full"
							data-testid="review-full">{checksumAddress(o)}</span
						></span
					>
				</li>
			{/each}
		</ol>
		<CustomServiceNote {m} />
		<div class="select">
			<span>{m.create.network}</span>
			<p class="hint flush">{m.create.networkHint}</p>
			<NetworkChoice
				label={m.create.network}
				moreLabel={m.add.moreNetworks}
				{networks}
				chosen={chainId}
				disabled={busy}
				onchoose={(id) => {
					chose = true;
					chainId = id;
				}}
			/>
			{#if chainId === null && networks.length > 0}
				<p class="hint flush" role="status">{m.create.networkNeeded}</p>
			{/if}
		</div>
		{#if networks.length === 0}
			<p class="error" role="alert">{m.create.noSafeNetworks}</p>
		{/if}
		{#if otherSpace.length > 0 && spaceWords}
			<!-- r7-9: the other space's networks are not offered here — said, with the way there. -->
			<div class="other-space" data-testid="other-space">
				<p class="hint flush">
					{fillTemplate(m.create.otherSpace, {
						space: networkSpace.testnet ? spaceWords.mainnet : spaceWords.testnet,
						networks: listOf(otherSpace, locale)
					})}
				</p>
				<button type="button" class="text-button" onclick={() => networkSpace.openSwitcher()}
					>{networkSpace.testnet ? spaceWords.toMainnet : spaceWords.toTestnet}</button
				>
			</div>
		{/if}
		{#if local.length > 1}
			<div class="select">
				<span>{m.create.creator}</span>
				<Dropdown
					label={m.create.creator}
					value={creator?.name ?? ''}
					open={menu === 'creator'}
					rows={local.map((o) => ({
						id: o.address,
						label: o.name,
						selected: o.address === creator?.address
					}))}
					ontoggle={() => (menu = menu === 'creator' ? null : 'creator')}
					onselect={(id) => {
						creatorChoice = id;
						menu = null;
					}}
				/>
			</div>
		{/if}
		{#if failure}<p class="error" role="alert">
				{fillTemplate(m.proposal.failed, { reason: failure })}
			</p>{/if}
		<div class="nav two">
			<Button
				variant="secondary"
				shape="rounded"
				disabled={busy}
				onclick={() => (step = 'threshold')}>{m.create.back}</Button
			>
			<Button
				variant="primary"
				shape="rounded"
				loading={busy}
				disabled={busy || !creator || !draft || networks.length === 0 || chainId === null}
				onclick={create}>{m.create.create}</Button
			>
		</div>
	{:else if created}
		<div class="hero">
			<span class="done"><Icon icon={UTILITY_ICONS.check} size="lg" /></span>
			<p class="hero-rule">{m.create.created}</p>
			{#if cleanName}<p class="hero-name">{cleanName}</p>{/if}
			<span class="hero-avatar"
				><Identicon svg={identiconSvgForClient(created)} size="detail" address={created} /></span
			>
			<p class="hero-address">{checksumAddress(created)}</p>
			{#if sharedState}
				<!-- r4a-12, r4a-13: kept here first; shared when the service has it — said as it is. -->
				<p
					class="state"
					class:pending={!kept || !keptRow?.shared}
					role="status"
					data-testid="created-state"
				>
					{sharedState}
				</p>
			{/if}
			<p class="hint" data-testid="created-body">
				{solo ? m.create.createdBodySolo : m.create.createdBody}
			</p>
		</div>
		<div class="nav two">
			<Button variant="secondary" shape="rounded" onclick={copy}>
				<span class="label"
					><Icon icon={copied ? UTILITY_ICONS.check : UTILITY_ICONS.copy} size="sm" />{copied
						? m.proposal.copied
						: m.create.copyInvite}</span
				>
			</Button>
			<Button variant="primary" shape="rounded" onclick={() => oncreated(created ?? '')}
				>{m.create.done}</Button
			>
		</div>
	{/if}
</Sheet>

<style>
	.resumed,
	.other-space {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: var(--space-xs) var(--space-sm);
	}

	.text-button {
		padding: 0;
		border: none;
		background: none;
		font: inherit;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-base);
		text-decoration: underline;
		text-underline-offset: var(--space-xs);
		cursor: pointer;
	}

	.field-head {
		display: flex;
		justify-content: space-between;
		gap: var(--space-sm);
	}

	.count {
		font-family: var(--font-numeric);
		color: var(--color-fg-muted);
	}

	.count.over {
		color: var(--color-error-base);
	}

	input.name-field.small {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
	}

	.typed + .typed {
		margin-block-start: var(--space-sm);
	}

	.ens {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
		margin-block-start: var(--space-sm);
		padding: var(--space-md);
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
	}

	.line {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	.ens-address {
		margin: 0;
		font-family: var(--font-mono);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
		overflow-wrap: anywhere;
	}

	.pick {
		display: inline-flex;
		align-items: center;
		gap: var(--space-sm);
		min-height: var(--size-control-md);
		margin-block-start: var(--space-sm);
		padding: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
		text-decoration: underline;
		text-decoration-thickness: var(--border-hairline);
		text-underline-offset: var(--space-xs);
		cursor: pointer;
	}

	.contacts {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	.contacts button {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		width: 100%;
		padding: var(--space-sm) 0;
		border: none;
		border-bottom: var(--border-hairline) solid var(--color-border-base);
		background: none;
		font-family: var(--font-ui);
		text-align: start;
		color: var(--color-fg-base);
		cursor: pointer;
	}

	.address.full {
		overflow-wrap: anywhere;
	}

	.hint.flush {
		margin: 0;
	}

	.state {
		margin: var(--space-md) 0 0;
		padding: var(--space-sm) var(--space-md);
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	.state.pending {
		background: var(--color-warning-soft);
		border: var(--border-hairline) solid var(--color-warning-border);
	}

	.intro {
		margin: 0 0 var(--space-lg);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	h3 {
		margin: var(--space-lg) 0 var(--space-sm);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-muted);
	}

	.choices,
	.owners {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	.choices label {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		padding: var(--space-sm) var(--space-md);
		margin-block-end: var(--space-xs);
		border-radius: var(--radius-lg);
		border: var(--border-hairline) solid var(--color-border-base);
		cursor: pointer;
	}

	.choices label.on {
		border-color: var(--color-border-strong);
		background: var(--color-bg-sunken);
	}

	.choices input {
		position: absolute;
		opacity: 0;
		pointer-events: none;
	}

	.box {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: var(--icon-lg);
		height: var(--icon-lg);
		border-radius: var(--radius-sm);
		border: var(--border-hairline) solid var(--color-border-strong);
		color: var(--color-fg-base);
	}

	.text {
		display: flex;
		flex-direction: column;
		flex: 1;
		min-width: 0;
	}

	.name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-weight: var(--weight-medium);
		color: var(--color-fg-base);
	}

	.address {
		font-family: var(--font-mono);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.typed {
		display: flex;
		gap: var(--space-sm);
	}

	.typed input {
		flex: 1;
		min-width: 0;
	}

	input.mono,
	input.name-field {
		min-height: var(--size-control-lg);
		padding: 0 var(--space-md);
		border: var(--border-hairline) solid var(--color-border-strong);
		border-radius: var(--radius-lg);
		background: var(--color-bg-base);
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	input.name-field {
		font-family: var(--font-ui);
	}

	input.name-field {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
	}

	.hero-name {
		margin: 0;
		font-size: calc(var(--text-xl) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
		overflow-wrap: anywhere;
	}

	/* The Add button beside the name keeps its own width — never a field
	   alone on its row: the address field, the last child of its own row,
	   read half the sheet wide on a phone (r5reg-19). */
	.typed > :global(:last-child:not(input)) {
		flex: none;
		width: auto;
	}

	.owners li {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-md);
		padding-block: var(--space-sm);
		border-bottom: var(--border-hairline) solid var(--color-border-base);
	}

	/* r7: the kind on its own line, under the member's name (past the ordinal). */
	.member-kind {
		flex-basis: 100%;
		padding-inline-start: calc(var(--icon-lg) + var(--space-md));
	}

	.ordinal {
		flex: none;
		width: var(--icon-lg);
		text-align: center;
		font-family: var(--font-numeric);
		/* The rail's ordinal ink is a decoration's, too faint to read a
		   member's number by (vis-21): the muted ink the addresses carry. */
		color: var(--color-fg-muted);
	}

	/* r4D-12: never drawn smaller than the target floor (--size-targetMin). */
	.remove {
		flex: none;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-width: var(--size-targetMin);
		min-height: var(--size-targetMin);
		padding: var(--space-xs);
		border: 0;
		background: none;
		color: var(--color-fg-subtle);
		cursor: pointer;
	}

	.hint {
		margin: var(--space-sm) 0 0;
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.error {
		margin: var(--space-sm) 0 0;
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-error-base);
	}

	.nav {
		margin-block-start: var(--space-2xl);
	}

	.nav.two {
		display: flex;
		gap: var(--space-md);
	}

	.nav.two > :global(*) {
		flex: 1;
	}

	.stepper {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-xl);
		padding-block: var(--space-xl);
	}

	.stepper button {
		width: var(--size-control-lg);
		height: var(--size-control-lg);
		border-radius: var(--radius-full);
		border: var(--border-hairline) solid var(--color-border-strong);
		background: var(--color-bg-base);
		font-size: calc(var(--text-xl) * var(--text-scale, 1));
		color: var(--color-fg-base);
		cursor: pointer;
	}

	.stepper button:disabled {
		opacity: var(--opacity-disabled);
		cursor: default;
	}

	output {
		min-width: calc(var(--space-5xl) * 2);
		text-align: center;
		font-family: var(--font-display);
		font-size: calc(var(--text-2xl) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.note {
		margin: 0;
		padding: var(--space-md);
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.note.risky {
		background: var(--color-warning-soft);
		border: var(--border-hairline) solid var(--color-warning-border);
		color: var(--color-fg-base);
	}

	.hero {
		display: flex;
		flex-direction: column;
		align-items: center;
		text-align: center;
		padding-block: var(--space-lg);
	}

	.hero-avatar {
		display: inline-flex;
		margin-block: var(--space-md);
	}

	.hero-rule {
		margin: 0;
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.hero-address {
		margin: var(--space-sm) 0 0;
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-base);
		overflow-wrap: anywhere;
	}

	.done {
		display: inline-flex;
		padding: var(--space-md);
		border-radius: var(--radius-full);
		background: var(--color-success-soft);
		color: var(--color-success-base);
	}

	.compact li {
		padding-block: var(--space-xs);
	}

	.select {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		margin-block-start: var(--space-lg);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.label {
		display: inline-flex;
		align-items: center;
		gap: var(--space-sm);
	}
</style>
