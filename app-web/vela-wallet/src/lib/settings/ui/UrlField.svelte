<script lang="ts">
	/**
	 * A labelled mono text field (spec 023).
	 *
	 * Every endpoint on ST9b / ST11 / ST12 / SR2 / SR5 is one of these: a label
	 * row that may carry a latency pill, a mono input, and an optional hint
	 * under it. The tone outlines the box when a state has something to say
	 * about the value in it.
	 */
	import type { UrlFieldModel } from '../model';
	import StatusPill from './StatusPill.svelte';

	interface Props {
		field: UrlFieldModel;
		/** Trailing in-field action — the RPC-provider cards' 检查密钥 / 获取密钥. */
		action?: string;
		onaction?: () => void;
		oninput?: (value: string) => void;
		/** Leaving the field is the save signal on every network surface. */
		onblur?: () => void;
	}

	let { field, action, onaction, oninput, onblur }: Props = $props();
</script>

<div class="field">
	{#if field.label !== '' || field.badge !== undefined}
		<div class="head">
			<span class="label">{field.label}</span>
			{#if field.badge !== undefined}
				<StatusPill pill={field.badge} />
			{/if}
		</div>
	{/if}

	<div class="box {field.tone ?? 'default'}" data-field data-tone={field.tone}>
		<input
			type="text"
			value={field.value}
			placeholder={field.placeholder ?? ''}
			aria-label={field.label}
			spellcheck="false"
			autocomplete="off"
			oninput={(event) => oninput?.(event.currentTarget.value)}
			onblur={() => onblur?.()}
		/>
		{#if action !== undefined}
			<button type="button" class="action" onclick={onaction}>{action}</button>
		{/if}
	</div>

	{#if field.hint !== undefined}
		<p class="hint">{field.hint}</p>
	{/if}
</div>

<style>
	.field {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}

	.head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-md);
	}

	.label {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		letter-spacing: var(--letterSpacing-sectionLabel);
		color: var(--color-fg-subtle);
	}

	.box {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		min-height: var(--size-control-md);
		padding-inline: var(--space-lg);
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
		/* hairline border.base even at rest: on dark, sunken and base are one step
		   apart and the box would otherwise have no edge at all. */
		border: var(--border-hairline) solid var(--color-border-base);
	}

	.box.error {
		border-color: var(--color-error-base);
	}

	.box.success {
		border-color: var(--color-success-base);
	}

	/* Focus: app.css's `data-field` edge, in the tone's colour when it has one. */

	input {
		flex: 1;
		min-width: 0;
		border: none;
		background: none;
		font-family: var(--font-mono);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
		outline: none;
	}

	input::placeholder {
		color: var(--color-fg-subtle);
	}

	.action {
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-info-base);
		cursor: pointer;
		white-space: nowrap;
		flex-shrink: 0;
	}

	.hint {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-subtle);
	}
</style>
