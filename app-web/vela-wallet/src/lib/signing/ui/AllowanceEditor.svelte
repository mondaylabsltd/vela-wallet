<script lang="ts">
	import KeyValueRows from './KeyValueRows.svelte';
	import type { AllowanceChip, AllowanceInput, KeyValueRow, Tone } from '../model';

	/**
	 * The approval editor (spec 022 §4, never-unlimited mandate).
	 *
	 * The `requested` chip is DISABLED whenever the request is unlimited — not
	 * merely unselected. A wallet that renders "unlimited" as one tap among
	 * four has made the dangerous choice the easy one; this one refuses to
	 * offer it at all and makes you name a finite number instead.
	 */
	interface Props {
		label: string;
		value: string;
		valueTone: Tone;
		chips: AllowanceChip[];
		note?: string;
		resultingTotal?: KeyValueRow;
		/** Present only while `Custom` is the chosen chip. */
		custom?: AllowanceInput;
		onchip?: (id: string) => void;
		/** Every keystroke goes back to the machine that validates it. */
		oncustom?: (text: string) => void;
	}

	let { label, value, valueTone, chips, note, resultingTotal, custom, onchip, oncustom }: Props =
		$props();
</script>

<section class="editor">
	<div class="head">
		<span class="label">{label}</span>
		<span class="value" data-tone={valueTone}>{value}</span>
	</div>
	<div class="chips">
		{#each chips as chip (chip.id)}
			<button
				type="button"
				class="chip"
				class:selected={chip.state === 'selected'}
				disabled={chip.state === 'disabled'}
				aria-pressed={chip.state === 'selected'}
				onclick={() => onchip?.(chip.id)}
			>
				{chip.label}
			</button>
		{/each}
	</div>
	{#if custom}
		<!--
			The cap being typed. The big value above keeps counting as this
			changes — that feedback is what makes typing a cap safe — and the
			value here is the core's, never a local echo.
		-->
		<label class="field" class:invalid={Boolean(custom.error)}>
			<input
				type="text"
				inputmode="decimal"
				value={custom.value}
				placeholder={custom.placeholder}
				aria-label={custom.symbol}
				aria-invalid={Boolean(custom.error)}
				oninput={(event) => oncustom?.((event.currentTarget as HTMLInputElement).value)}
			/>
			<span class="unit">{custom.symbol}</span>
		</label>
		{#if custom.error}
			<p class="error">{custom.error}</p>
		{/if}
	{/if}
	{#if note}
		<p class="note">{note}</p>
	{/if}
</section>
{#if resultingTotal}
	<KeyValueRows rows={[resultingTotal]} />
{/if}

<style>
	.editor {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
		padding: var(--space-xl);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-xl);
	}

	.head {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-lg);
	}

	.label {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.value {
		font-family: var(--font-numeric);
		font-size: calc(var(--text-2xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	.value[data-tone='danger'] {
		color: var(--color-error-base);
	}

	.chips {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-md);
	}

	.chip {
		height: var(--size-control-sm);
		padding-inline: var(--space-lg);
		border: var(--border-hairline) solid var(--color-border-strong);
		border-radius: var(--radius-full);
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
		cursor: pointer;
	}

	.chip:active:not(:disabled) {
		transform: scale(var(--motion-press-button));
	}

	.chip.selected {
		border-color: var(--color-accent-base);
		color: var(--color-accent-base);
		font-weight: var(--weight-semibold);
	}

	.chip:disabled {
		opacity: var(--opacity-disabled);
		cursor: not-allowed;
	}

	.field {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		padding: var(--space-lg);
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
	}

	.field input {
		flex: 1;
		min-width: 0;
		border: none;
		background: none;
		font-family: var(--font-numeric);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	.field input:focus {
		outline: none;
	}

	.field.invalid {
		box-shadow: inset 0 0 0 var(--border-hairline) var(--color-error-base);
	}

	.unit {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.error {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-error-base);
	}

	.note {
		margin: 0;
		/* Two sentences, two lines: joining them with a space produces a run-on
		   in CJK, where a space is not a sentence break. */
		white-space: pre-line;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
	}
</style>
