<script lang="ts">
	/**
	 * Creating a multisig, in three steps: its name and who its members are
	 * (in order — the order is part of the address; the name is a label
	 * written with its record), how many must approve, and a review that
	 * shows the address before anything is signed. One passkey prompt at
	 * the end: the creator's proof, with which the genesis service records the
	 * multisig. Nothing is deployed and no fee is paid until its first
	 * transfer out.
	 *
	 * The first network is where that record is written, so only the
	 * networks the service says it serves (`/api/health`'s chain table) are
	 * offered — all of them while that is not known.
	 */
	import { untrack } from 'svelte';
	import Button from '$lib/ui/Button.svelte';
	import Dropdown from '$lib/settings/ui/Dropdown.svelte';
	import { fillTemplate } from '$lib/i18n/fill';
	import { pluralForm } from '$lib/i18n/plural';
	import { loadAccounts } from '$lib/onboarding/core/storage';
	import { spaceNetworksSync } from '$lib/services/networks';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { identiconSvgForClient } from '$lib/wallet/identicon';
	import type { MultisigMessages } from '../messages';
	import { createMultisig, draftGenesis, newSaltNonce } from '../core/accounts';
	import { memberLabel, shortenAddress } from '../core/display';
	import { sameAddress, validName } from '../core/kernel';
	import { localOwners } from '../core/owners';
	import { addLink, shareOrigin } from '../core/link';
	import { memberProblem } from '../core/members';
	import { servedChains, servedOnly } from '../core/service';
	import CustomServiceNote from './CustomServiceNote.svelte';
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
	const saltNonce = newSaltNonce();

	/** The chains the service serves; `null` while not known (nothing narrowed). */
	let served = $state<Set<number> | null>(null);
	void servedChains().then((chains) => (served = chains));
	const networks = $derived(servedOnly(spaceNetworks, served));

	let step = $state<'members' | 'threshold' | 'review' | 'created'>('members');
	// The account in front seeds the members once; the person edits from there.
	let owners = $state<string[]>(
		untrack(() => (current ? [current] : accounts.slice(0, 1).map((a) => a.address)))
	);
	let typed = $state('');
	let typedError = $state<string | null>(null);
	let threshold = $state(1);
	let name = $state('');
	let chainId = $state(
		spaceNetworks.find((n) => n.chainId === 100)?.chainId ?? spaceNetworks[0]?.chainId ?? 100
	);
	// The service's table came: a network it does not serve is not the pick.
	$effect(() => {
		if (networks.length > 0 && !networks.some((n) => n.chainId === chainId)) {
			chainId = (networks.find((n) => n.chainId === 100) ?? networks[0]).chainId;
		}
	});
	let creatorChoice = $state<string | null>(null);
	/** vis-19: which of the review's two choices has its menu open. */
	let menu = $state<'network' | 'creator' | null>(null);
	const toggleMenu = (which: 'network' | 'creator') => (menu = menu === which ? null : which);
	let busy = $state(false);
	let failure = $state<string | null>(null);
	let created = $state<string | null>(null);
	let copied = $state(false);

	const local = $derived(localOwners(owners));
	const creator = $derived(local.find((o) => o.address === creatorChoice) ?? local[0] ?? null);
	const cleanName = $derived(name.trim());
	const nameOk = $derived.by(() => {
		try {
			return cleanName !== '' && validName(cleanName);
		} catch {
			return false;
		}
	});
	const membersOk = $derived(owners.length >= 2 && local.length >= 1 && nameOk);
	const draft = $derived.by(() => {
		try {
			return owners.length > 0 && nameOk
				? draftGenesis({ owners, threshold, name: cleanName }, saltNonce)
				: null;
		} catch {
			return null;
		}
	});

	function toggle(address: string) {
		owners = owners.some((o) => sameAddress(o, address))
			? owners.filter((o) => !sameAddress(o, address))
			: [...owners, address];
	}

	function addTyped() {
		const value = typed.trim();
		const problem = memberProblem(value, owners);
		if (problem) {
			typedError = {
				invalidAddress: m.send.invalidAddress,
				checksum: m.send.invalidChecksum,
				notMember: m.create.notMember,
				duplicate: m.create.duplicate
			}[problem];
			return;
		}
		owners = [...owners, value];
		typed = '';
		typedError = null;
	}

	/** The member count the threshold was last set for: back and Next keep the person's pick. */
	let thresholdFor = 0;

	function toThreshold() {
		if (!membersOk) return;
		// A sensible start: all of two, else all but one — one lost passkey is
		// survivable. Only when the members changed; else it stays the person's.
		if (thresholdFor !== owners.length) {
			threshold = owners.length <= 2 ? owners.length : owners.length - 1;
			thresholdFor = owners.length;
		}
		step = 'threshold';
	}

	const thresholdNote = $derived(
		threshold === owners.length
			? m.create.thresholdAll
			: threshold === 1
				? m.create.thresholdOne
				: fillTemplate(pluralForm(m.create.thresholdSpare, owners.length - threshold), {
						count: owners.length - threshold
					})
	);
	const risky = $derived(threshold === owners.length || threshold === 1);

	async function create() {
		if (!creator || !draft || !nameOk) return;
		busy = true;
		failure = null;
		try {
			const account = await createMultisig(
				{ owners, threshold, chainId, creator, name: cleanName },
				saltNonce
			);
			created = account.address;
			step = 'created';
		} catch (error) {
			const name = error instanceof DOMException ? error.name : '';
			if (name !== 'NotAllowedError' && name !== 'AbortError') {
				failure = error instanceof Error ? error.message : String(error);
			}
		} finally {
			busy = false;
		}
	}

	async function copy() {
		if (!created) return;
		try {
			// The link adds it in Vela with one tap; the address is in it.
			await navigator.clipboard.writeText(addLink(shareOrigin(location.origin), locale, created));
			copied = true;
			setTimeout(() => (copied = false), 1600);
		} catch {
			// the address stays on screen
		}
	}
</script>

<!-- eslint-disable svelte/no-at-html-tags -- every {@html} here is the core's identicon SVG, drawn from an address -->
<Sheet
	title={m.create.title}
	{closeLabel}
	{wide}
	{busy}
	onclose={step === 'created' && created ? () => oncreated(created ?? '') : onclose}
>
	{#if step === 'members'}
		<p class="intro">{m.create.intro}</p>
		<!-- First, as a Vela account's name is: it is part of the address. -->
		<label class="select">
			<span>{m.create.name}</span>
			<input
				class="name-field"
				placeholder={m.create.namePlaceholder}
				autocomplete="off"
				bind:value={name}
			/>
		</label>
		{#if cleanName !== '' && !nameOk}
			<p class="error">{m.create.nameInvalid}</p>
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
						<!-- eslint-disable-next-line svelte/no-at-html-tags -- the core's own SVG, from the address -->
						<span class="avatar" aria-hidden="true">{@html identiconSvgForClient(a.address)}</span>
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
				placeholder={m.create.addressPlaceholder}
				autocomplete="off"
				spellcheck="false"
				bind:value={typed}
				onkeydown={(e) => e.key === 'Enter' && (e.preventDefault(), addTyped())}
			/>
			<Button variant="secondary" shape="rounded" onclick={addTyped}>{m.create.addMember}</Button>
		</div>
		{#if typedError}<p class="error">{typedError}</p>{/if}

		{#if owners.length > 0}
			<h3>{m.create.members} · {owners.length}</h3>
			<ol class="owners">
				{#each owners as o, i (o)}
					<li>
						<span class="ordinal">{i + 1}</span>
						<!-- eslint-disable-next-line svelte/no-at-html-tags -- the core's own SVG, from the address -->
						<span class="avatar" aria-hidden="true">{@html identiconSvgForClient(o)}</span>
						<span class="text"
							><span class="name">{memberLabel(o)}</span
							>{#if memberLabel(o) !== shortenAddress(o)}<span class="address"
									>{shortenAddress(o)}</span
								>{/if}</span
						>
						<!-- vis-16: named for what it does, with whom (it read "Close"). -->
						<button
							type="button"
							class="remove"
							aria-label="{m.action.removeOwner} · {memberLabel(o)}"
							onclick={() => toggle(o)}
						>
							<Icon icon={UTILITY_ICONS['x']} size="sm" />
						</button>
					</li>
				{/each}
			</ol>
			<p class="hint">{m.create.order}</p>
		{/if}
		{#if owners.length > 0 && owners.length < 2}<p class="hint">{m.create.needTwo}</p>{/if}
		{#if owners.length >= 2 && local.length === 0}<p class="error">{m.create.needLocal}</p>{/if}

		<div class="nav">
			<Button variant="primary" shape="rounded" disabled={!membersOk} onclick={toThreshold}
				>{m.create.next}</Button
			>
		</div>
	{:else if step === 'threshold'}
		<h3>{m.create.threshold}</h3>
		<div class="stepper" role="group" aria-label={m.create.threshold}>
			<button
				type="button"
				aria-label={m.create.fewer}
				disabled={threshold <= 1}
				onclick={() => (threshold -= 1)}>−</button
			>
			<output>{fillTemplate(m.create.thresholdValue, { m: threshold, n: owners.length })}</output>
			<button
				type="button"
				aria-label={m.create.more}
				disabled={threshold >= owners.length}
				onclick={() => (threshold += 1)}>+</button
			>
		</div>
		<p class="note" class:risky>{thresholdNote}</p>
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
				<!-- eslint-disable-next-line svelte/no-at-html-tags -- the core's own SVG, from the address -->
				<span class="hero-avatar" aria-hidden="true"
					>{@html identiconSvgForClient(draft.address)}</span
				>
				<p class="hero-name">{cleanName}</p>
				<p class="hero-rule">{fillTemplate(m.panel.title, { m: threshold, n: owners.length })}</p>
				<p class="hero-address">{draft.address}</p>
				<p class="hint">{m.create.sameEverywhere}</p>
			</div>
		{/if}
		<h3>{m.create.members}</h3>
		<ol class="owners compact">
			{#each owners as o, i (o)}
				<li>
					<span class="ordinal">{i + 1}</span><span class="text"
						><span class="name">{memberLabel(o)}</span
						>{#if memberLabel(o) !== shortenAddress(o)}<span class="address"
								>{shortenAddress(o)}</span
							>{/if}</span
					>
				</li>
			{/each}
		</ol>
		<CustomServiceNote {m} />
		<!-- vis-19: the app's own select (the settings dropdown and its rows),
		     not the browser's — a native menu drew in the system's face and
		     colours, the one control in the sheet that did. -->
		<div class="select">
			<span>{m.create.network}</span>
			<Dropdown
				label={m.create.network}
				value={networks.find((n) => n.chainId === chainId)?.displayName ?? ''}
				open={menu === 'network'}
				rows={networks.map((n) => ({
					id: String(n.chainId),
					label: n.displayName,
					selected: n.chainId === chainId
				}))}
				ontoggle={() => toggleMenu('network')}
				onselect={(id) => {
					chainId = Number(id);
					menu = null;
				}}
			/>
		</div>
		{#if networks.length === 0}
			<p class="error" role="alert">{m.service.noNetworks}</p>
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
					ontoggle={() => toggleMenu('creator')}
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
				disabled={busy || !creator || !draft || networks.length === 0}
				onclick={create}>{m.create.create}</Button
			>
		</div>
	{:else if created}
		<div class="hero">
			<span class="done"><Icon icon={UTILITY_ICONS.check} size="lg" /></span>
			<p class="hero-rule">{m.create.created}</p>
			{#if cleanName}<p class="hero-name">{cleanName}</p>{/if}
			<!-- eslint-disable-next-line svelte/no-at-html-tags -- the core's own SVG, from the address -->
			<span class="hero-avatar" aria-hidden="true">{@html identiconSvgForClient(created)}</span>
			<p class="hero-address">{created}</p>
			<p class="hint">{m.create.createdBody}</p>
		</div>
		<div class="nav two">
			<Button variant="secondary" shape="rounded" onclick={copy}>
				<span class="label"
					><Icon icon={copied ? UTILITY_ICONS.check : UTILITY_ICONS.copy} size="sm" />{copied
						? m.proposal.copied
						: m.proposal.copyLink}</span
				>
			</Button>
			<Button variant="primary" shape="rounded" onclick={() => oncreated(created ?? '')}
				>{m.create.done}</Button
			>
		</div>
	{/if}
</Sheet>

<!-- eslint-enable svelte/no-at-html-tags -->

<style>
	.intro {
		margin: 0 0 var(--space-lg);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	h3 {
		margin: var(--space-lg) 0 var(--space-sm);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-subtle);
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

	.avatar {
		display: inline-flex;
		flex: none;
		width: var(--icon-2xl);
		height: var(--icon-2xl);
		border-radius: var(--radius-full);
		overflow: hidden;
	}

	.avatar :global(svg),
	.hero-avatar :global(svg) {
		width: 100%;
		height: 100%;
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
		color: var(--color-fg-subtle);
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

	.typed > :global(:last-child) {
		flex: none;
		width: auto;
	}

	.owners li {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		padding-block: var(--space-sm);
		border-bottom: var(--border-hairline) solid var(--color-border-base);
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

	.remove {
		flex: none;
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
		width: var(--size-identiconDetail);
		height: var(--size-identiconDetail);
		border-radius: var(--radius-full);
		overflow: hidden;
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
		color: var(--color-fg-subtle);
	}

	.label {
		display: inline-flex;
		align-items: center;
		gap: var(--space-sm);
	}
</style>
