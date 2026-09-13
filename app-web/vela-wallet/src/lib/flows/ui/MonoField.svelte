<script lang="ts">
	/**
	 * The monospace text field (spec 021 component 7): T3's contract address,
	 * T3b's network query, SD2c's pasted recipient list.
	 *
	 * Addresses are compared character by character by the people pasting them,
	 * which is the whole reason for the mono face — and the reason the error
	 * state colours the BORDER and prints underneath rather than tinting the
	 * text, which would make the characters harder to read at the moment they
	 * most need reading.
	 */
	interface Props {
		label?: string;
		value: string;
		placeholder?: string;
		error?: string;
		/** Multi-line paste target (SD2c). */
		rows?: number;
		oninput?: (value: string) => void;
	}

	let { label, value, placeholder, error, rows, oninput }: Props = $props();

	const invalid = $derived(error !== undefined);
</script>

<div class="wrap">
	{#if label !== undefined}<span class="label">{label}</span>{/if}
	{#if rows !== undefined}
		<textarea
			class="field"
			class:invalid
			data-field
			{rows}
			{placeholder}
			aria-label={label}
			aria-invalid={invalid}
			oninput={(event) => oninput?.(event.currentTarget.value)}>{value}</textarea
		>
	{:else}
		<input
			class="field"
			class:invalid
			data-field
			type="text"
			{value}
			{placeholder}
			aria-label={label}
			aria-invalid={invalid}
			spellcheck="false"
			autocapitalize="none"
			autocorrect="off"
			oninput={(event) => oninput?.(event.currentTarget.value)}
		/>
	{/if}
	{#if error !== undefined}<span class="error">{error}</span>{/if}
</div>

<style>
	.wrap {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	.label {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.field {
		width: 100%;
		padding: var(--space-lg);
		border: var(--border-hairline) solid transparent;
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
		font-family: var(--font-mono);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
		resize: none;
	}

	.field::placeholder {
		color: var(--color-fg-subtle);
	}

	/* Focus: app.css's `data-field` edge. An invalid field keeps its red. */

	.invalid {
		border-color: var(--color-error-base);
	}

	.error {
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-error-base);
	}
</style>
