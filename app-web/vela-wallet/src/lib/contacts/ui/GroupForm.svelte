<script lang="ts">
	/**
	 * The new/rename group form's BODY (spec 024) — one named field and a
	 * save. The container is the caller's: a bottom sheet on the phone, a
	 * centred dialog on the desktop (one field is a dialog's worth, as
	 * settings' add-network is).
	 */
	import NameField from '$lib/ui/onboarding/NameField.svelte';
	import Button from '$lib/ui/Button.svelte';
	import type { GroupFormCopy } from '../forms';

	interface Props {
		copy: GroupFormCopy;
		initialName?: string;
		onsave: (name: string) => void;
	}

	let { copy, initialName, onsave }: Props = $props();

	// Seed by design: the form is keyed per open; edits stay local.
	// svelte-ignore state_referenced_locally
	let name = $state(initialName ?? '');
	const canSave = $derived(name.trim() !== '');
</script>

<div class="form">
	<NameField
		label={copy.nameLabel}
		placeholder={copy.namePlaceholder}
		value={name}
		oninput={(next) => (name = next)}
	/>
	<div class="actions">
		<Button
			variant="primary"
			shape="rounded"
			disabled={!canSave}
			onclick={() => canSave && onsave(name.trim())}
		>
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

	.actions {
		margin-block-start: var(--space-md);
	}
</style>
