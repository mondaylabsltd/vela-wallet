<script lang="ts">
	/**
	 * The add/edit contact form's BODY (spec 024): two fields and a save. The
	 * container is the caller's — a bottom sheet on the phone, the third column
	 * on the desktop — because a form that slides up from the bottom of a
	 * desktop window is a phone control drawn at the wrong size, and the
	 * desktop's rule is "every phone sheet becomes a column or a dialog".
	 *
	 * The address-shape gate is a FORM gate, matching the Expo form and the
	 * core's own `is_address` predicate (42-char 0x-hex); the core's save API
	 * merges whatever it is given, so the form is where garbage stops
	 * (deviation record, spec US2/AS4).
	 */
	import NameField from '$lib/ui/onboarding/NameField.svelte';
	import Button from '$lib/ui/Button.svelte';
	import type { ContactDraft, ContactFormCopy } from '../forms';

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
	/** The core's `is_address` shape: 0x + 40 hex. */
	const validAddress = $derived(/^0x[0-9a-fA-F]{40}$/.test(address.trim()));
	const canSave = $derived(validAddress && name.trim() !== '');

	function save() {
		touched = true;
		if (!canSave) return;
		onsave({ name: name.trim(), address: address.trim() });
	}
</script>

<div class="form">
	<NameField
		label={copy.nameLabel}
		placeholder={copy.namePlaceholder}
		value={name}
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
			errorText={touched && !validAddress ? copy.invalidAddress : undefined}
			oninput={(next) => {
				address = next;
				touched = true;
			}}
		/>
	{/if}
	<div class="actions">
		<Button variant="primary" shape="rounded" disabled={!canSave} onclick={save}>
			{copy.save}
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
