<script lang="ts">
	/**
	 * The Clear Signer's page (spec 071): the phone's sheet body and the
	 * desktop panel's section — one body, so the two layouts cannot say
	 * different things about where a signature is checked.
	 *
	 * The field holds what the person is TYPING; the page in force is the
	 * core's. Save hands the text to `sign_pref`, which normalises and stores
	 * it — or refuses it, and then the old page stands and the sentence under
	 * the field says why, over the text still there to be fixed. The rpId line
	 * is shown whenever the page in force is off `getvela.app`: a copy there
	 * can show a request but its ceremony will find none of this wallet's keys,
	 * and the person should hear that here rather than at the worst moment.
	 */
	import Button from '$lib/ui/Button.svelte';
	import type { SignerPageModel } from '../model';
	import UrlField from './UrlField.svelte';

	interface Props {
		page: SignerPageModel;
		onsave?: (text: string) => void;
		onreset?: () => void;
	}

	let { page, onsave, onreset }: Props = $props();

	// What the person is typing, over the page in force. A new page in force —
	// saved, reset — is what the field shows next; a refused one changes
	// nothing in force, so the typed text stays. Through `inForce` on purpose:
	// the model is rebuilt on every unrelated change, and only a different
	// ADDRESS may take the person's typing away.
	const inForce = $derived(page.field.value);
	let draft = $derived(inForce);
</script>

<form
	class="signer-page"
	onsubmit={(event) => {
		event.preventDefault();
		onsave?.(draft);
	}}
>
	<UrlField field={{ ...page.field, value: draft }} oninput={(value) => (draft = value)} />

	{#if page.error !== undefined}
		<p class="note error" role="alert">{page.error}</p>
	{/if}
	{#if page.foreign !== undefined}
		<p class="note">{page.foreign}</p>
	{/if}

	<div class="actions">
		{#if page.reset !== undefined}
			<button type="button" class="reset" onclick={() => onreset?.()}>{page.reset}</button>
		{/if}
		<Button variant="secondary" shape="rounded" onclick={() => onsave?.(draft)}>
			{page.save}
		</Button>
	</div>
</form>

<style>
	.signer-page {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
		padding-block: var(--space-md) var(--space-xl);
	}

	.note {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-subtle);
	}

	.note.error {
		color: var(--color-error-base);
	}

	.actions {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		gap: var(--space-xl);
	}

	.reset {
		padding: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-info-base);
		cursor: pointer;
	}
</style>
