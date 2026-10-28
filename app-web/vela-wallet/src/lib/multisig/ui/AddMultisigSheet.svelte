<script lang="ts">
	/**
	 * Adding a multisig by its address — all another member needs. The
	 * genesis comes from the service and is accepted only if the core
	 * re-derives the address from it (`addMultisig`).
	 *
	 * Or importing Safes from a Safe{Wallet} export: the file is read by the
	 * core on this device, the person picks the Safes, and each is imported
	 * with a genesis the core re-derives its address from (`importExported`).
	 */
	import { untrack } from 'svelte';
	import Button from '$lib/ui/Button.svelte';
	import { fillTemplate } from '$lib/i18n/fill';
	import { chainName, spaceNetworksSync } from '$lib/services/networks';
	import { shortenAddress } from '$lib/wallet/identity';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import type { MultisigMessages } from '../messages';
	import { addMultisig, importExported } from '../core/accounts';
	import { parseSafeExport, type ExportedSafe } from '../core/kernel';
	import { multisig } from '../core/store.svelte';
	import Sheet from './Sheet.svelte';

	interface Props {
		m: MultisigMessages;
		prefill?: string;
		wide: boolean;
		closeLabel: string;
		onclose: () => void;
		onadded: (address: string) => void;
	}

	let { m, prefill = '', wide, closeLabel, onclose, onadded }: Props = $props();

	// The link's address seeds the field once; the person may edit it.
	let address = $state(untrack(() => prefill));
	let busy = $state(false);
	let failure = $state<string | null>(null);
	const valid = $derived(/^0x[0-9a-fA-F]{40}$/.test(address.trim()));

	async function add() {
		if (!valid) {
			failure = m.send.invalidAddress;
			return;
		}
		busy = true;
		failure = null;
		const outcome = await addMultisig(
			address.trim(),
			spaceNetworksSync().map((n) => n.chainId)
		);
		busy = false;
		if (outcome.ok) onadded(outcome.account.address);
		else failure = m.add[outcome.reason];
	}

	$effect(() => {
		if (prefill && valid) void add();
	});

	// -- importing from Safe{Wallet} ----------------------------------------------

	let exported = $state<ExportedSafe[] | null>(null);
	let picked = $state<string[]>([]);
	let fileFailure = $state<string | null>(null);
	let results = $state<Record<string, 'busy' | 'ok' | 'notFound' | 'unavailable' | 'unsupported'>>(
		{}
	);
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
		try {
			const safes = parseSafeExport(await file.text());
			if (safes.length === 0) {
				fileFailure = m.import.empty;
				return;
			}
			exported = safes;
			// Everything new is picked to start with; one already here is shown as such.
			picked = safes.filter((s) => !multisig.find(s.address)).map((s) => s.address);
		} catch {
			fileFailure = m.import.notExport;
		}
	}

	function toggle(address: string) {
		picked = picked.includes(address) ? picked.filter((a) => a !== address) : [...picked, address];
	}

	async function importPicked() {
		if (!exported) return;
		for (const safe of exported.filter((s) => picked.includes(s.address))) {
			results = { ...results, [safe.address]: 'busy' };
			const outcome = await importExported(safe);
			results = { ...results, [safe.address]: outcome.ok ? 'ok' : outcome.reason };
		}
	}

	function rule(safe: ExportedSafe): string {
		const c = safe.chains[0];
		return c ? fillTemplate(m.create.thresholdValue, { m: c.threshold, n: c.owners.length }) : '';
	}
</script>

<Sheet title={m.add.title} {closeLabel} {wide} busy={busy || importing} {onclose}>
	{#if exported}
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
							{:else if result}
								<span class="error">{m.add[result]}</span>
							{/if}
						</span>
					</label>
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
				onclick={() => void importPicked()}
				>{fillTemplate(m.import.importSome, { total: picked.length })}</Button
			>
		{/if}
	{:else}
		<p class="intro">{m.add.intro}</p>
		<form
			onsubmit={(event) => {
				event.preventDefault();
				void add();
			}}
		>
			<input
				class="mono"
				placeholder={m.add.placeholder}
				autocomplete="off"
				spellcheck="false"
				bind:value={address}
				aria-invalid={failure !== null}
			/>
			{#if failure}<p class="error" role="alert">{failure}</p>{/if}
			<Button
				variant="primary"
				shape="rounded"
				loading={busy}
				disabled={busy}
				onclick={() => void add()}>{m.add.add}</Button
			>
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
		color: var(--color-fg-subtle);
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

	.error {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-error-base);
	}
</style>
