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
	import {
		cleanName,
		cleanNote,
		NAME_MAX_CHARS,
		NOTE_MAX_CHARS,
		type ContactDraft,
		type ContactFormCopy
	} from '../forms';
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
	// svelte-ignore state_referenced_locally
	let note = $state(initial?.note ?? '');
	/** Round 3: the note field is drawn where the caller gave it words (live). */
	const noted = $derived(copy.noteLabel !== undefined);
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
		onsave({
			name: cleanName(name),
			address: address.trim(),
			...(noted ? { note: cleanNote(note) } : {})
		});
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
	{#if noted}
		<label class="note">
			<span class="label">{copy.noteLabel}</span>
			<textarea
				rows="3"
				maxlength={NOTE_MAX_CHARS}
				placeholder={copy.notePlaceholder}
				value={note}
				oninput={(event) => (note = event.currentTarget.value)}></textarea>
		</label>
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

	.note {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	.note .label {
		color: var(--color-fg-muted);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
	}

	.note textarea {
		resize: vertical;
		padding: var(--space-md) var(--space-lg);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
		color: var(--color-fg-base);
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
	}
</style>
