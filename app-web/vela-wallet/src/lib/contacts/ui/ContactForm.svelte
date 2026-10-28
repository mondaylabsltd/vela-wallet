<script lang="ts">
	/**
	 * The add/edit contact form's BODY (spec 024): two fields and a save. The
	 * container is the caller's — a bottom sheet on the phone, the third column
	 * on the desktop — because a form that slides up from the bottom of a
	 * desktop window is a phone control drawn at the wrong size, and the
	 * desktop's rule is "every phone sheet becomes a column or a dialog".
	 *
	 * The address gate is the core's one rule (`addressEntry`): 42-char 0x-hex,
	 * and a mixed case that is its EIP-55 checksum (set-4) — a wrong one is a
	 * typo or an altered copy, said as that, never saved lowercased as if it
	 * were fine. The core's save refuses it too; the form says why.
	 */
	import NameField from '$lib/ui/onboarding/NameField.svelte';
	import Button from '$lib/ui/Button.svelte';
	import { addressEntry } from '$lib/core/kernels';
	import { cleanName, NAME_MAX_CHARS, type ContactDraft, type ContactFormCopy } from '../forms';
	import { fill } from '$lib/wallet/messages';

	interface Props {
		copy: ContactFormCopy;
		/** Present when editing — the address is identity and stays fixed. */
		initial?: ContactDraft;
		onsave: (draft: ContactDraft) => void;
	}

	let { copy, initial, onsave }: Props = $props();

	// Seeds by design: the form is keyed per open; edits stay local.
	// svelte-ignore state_referenced_locally
	let name = $state(initial?.name ?? '');
	// svelte-ignore state_referenced_locally
	let address = $state(initial?.address ?? '');
	let touched = $state(false);

	// svelte-ignore state_referenced_locally
	const editing = initial !== undefined;
	const entry = $derived(addressEntry(address.trim()));
	const validAddress = $derived(entry === 'valid');
	const canSave = $derived(validAddress && cleanName(name) !== '');
	/** set-3: adding an address already saved updates that contact — said first. */
	const existing = $derived(
		!editing && validAddress ? (copy.existingName?.(address.trim()) ?? null) : null
	);

	function save() {
		touched = true;
		if (!canSave) return;
		onsave({ name: cleanName(name), address: address.trim() });
	}
</script>

<div class="form">
	<NameField
		label={copy.nameLabel}
		placeholder={copy.namePlaceholder}
		value={name}
		maxlength={NAME_MAX_CHARS}
		oninput={(next) => (name = next)}
	/>
	{#if editing}
		<div class="fixed-address">
			<span class="label">{copy.addressLabel}</span>
			<span class="value">{address}</span>
		</div>
	{:else}
		<NameField
			label={copy.addressLabel}
			placeholder={copy.addressPlaceholder}
			value={address}
			errorText={!touched || validAddress
				? undefined
				: entry === 'checksum'
					? copy.invalidChecksum
					: copy.invalidAddress}
			hint={existing !== null && copy.duplicateAddress !== undefined
				? fill(copy.duplicateAddress, { name: existing })
				: undefined}
			oninput={(next) => {
				address = next;
				touched = true;
			}}
		/>
	{/if}
	<div class="actions">
		<Button variant="primary" shape="rounded" disabled={!canSave} onclick={save}>
			{existing !== null && copy.updateExisting !== undefined ? copy.updateExisting : copy.save}
		</Button>
	</div>
</div>

<style>
	.form {
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
		padding-block-end: var(--space-xl);
	}

	.fixed-address {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	.fixed-address .label {
		color: var(--color-fg-muted);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
	}

	.fixed-address .value {
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-base);
		word-break: break-all;
	}

	.actions {
		margin-block-start: var(--space-md);
	}
</style>
