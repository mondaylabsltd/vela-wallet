<script lang="ts">
	/**
	 * Labeled account-name field: label → input → (error) red inline
	 * over-length line. The error appears WITHOUT shifting the field above it.
	 *
	 * The v2 design gives the label the small uppercase treatment it uses for
	 * every field label, and drops the helper caption entirely — the
	 * placeholder is an EXAMPLE of a good answer, which explains the field
	 * better than a sentence under it. `hint` is therefore optional.
	 *
	 * So is `label`: on the create screen the heading directly above the field
	 * already says "name your wallet", and a label there restated it in smaller
	 * type. Omitted, the input keeps its accessible name from `aria-label`, so
	 * the label is gone from the SCREEN and not from the accessibility tree.
	 */
	interface Props {
		/** Resolved strings. Omit `label` to render the field unlabelled; it
		 *  still names itself to assistive technology via `placeholder`. */
		label?: string;
		/**
		 * The id of a heading that names the field when no `label` is drawn
		 * (vis-16): the placeholder is an example ("Everyday wallet"), and as
		 * the name it told a screen reader the field was called that.
		 */
		labelledby?: string;
		placeholder: string;
		hint?: string;
		/** Present → error styling + red inline line (A3). */
		errorText?: string;
		/** Initial value from state; edits stay local, reported via oninput. */
		value?: string;
		/** The most characters it takes (a name's cap). */
		maxlength?: number;
		oninput?: (value: string) => void;
	}

	let {
		label,
		labelledby,
		placeholder,
		hint,
		errorText,
		value = '',
		maxlength,
		oninput
	}: Props = $props();

	const id = $props.id();

	// Initial value by design: edits are local visual state (FR-011); the
	// panel re-keys this atom per state.
	// svelte-ignore state_referenced_locally
	let text = $state(value);

	const hasError = $derived(errorText !== undefined);
</script>

<div class="field">
	{#if label}
		<label class="label" for={id}>{label}</label>
	{/if}
	<input
		class="input"
		class:error={hasError}
		data-field
		{id}
		type="text"
		{placeholder}
		{maxlength}
		autocomplete="off"
		spellcheck="false"
		aria-label={label || labelledby ? undefined : placeholder}
		aria-labelledby={label ? undefined : labelledby}
		aria-invalid={hasError}
		aria-describedby={[hasError ? `${id}-error` : null, hint !== undefined ? `${id}-hint` : null]
			.filter(Boolean)
			.join(' ') || undefined}
		bind:value={text}
		oninput={() => oninput?.(text)}
	/>
	{#if errorText !== undefined}
		<p class="errorline" id="{id}-error">{errorText}</p>
	{/if}
	{#if hint !== undefined}
		<p class="hint" id="{id}-hint">{hint}</p>
	{/if}
</div>

<style>
	.field {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}

	/* Focus is app.css's `data-field` rule — one quiet edge, no ring. This
	   field used to thicken its border in accent (2026-08-25); the founder's
	   2026-09-05 call retired every accent focus on text entry at once. */

	.label {
		color: var(--color-fg-muted);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		letter-spacing: 0.06em;
		text-transform: uppercase;
	}

	.input {
		width: 100%;
		height: var(--size-control-lg);
		padding-inline: var(--space-xl);
		background: var(--color-bg-sunken);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-lg);
		color: var(--color-fg-base);
		font-family: var(--font-ui);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
	}

	.input::placeholder {
		color: var(--color-fg-subtle);
	}

	.input.error {
		border-color: var(--color-error-base);
	}

	.errorline {
		margin: 0;
		color: var(--color-error-base);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
	}

	.hint {
		margin: 0;
		color: var(--color-fg-muted);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
	}
</style>
