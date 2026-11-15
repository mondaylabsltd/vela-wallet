<script lang="ts">
	/**
	 * The tinted explanation box (spec 023).
	 *
	 * Eight mocks use it — the sign-out warning, the chain-ID mismatch, the
	 * "contracts are not deployed" hint, the erase-device losses, the RPC-down
	 * warning and its restored confirmation, the relayer disclaimer and the
	 * index-unreachable notice. One shape, four tones; `success` swaps the
	 * triangle for a check because a green triangle reads as an alarm.
	 */
	import type { CalloutModel } from '../model';
	import { UTILITY_ICONS } from '#lib/wallet/icons.ts';
	import Icon from '#lib/wallet/ui/Icon.svelte';

	interface Props {
		callout: CalloutModel;
	}

	let { callout }: Props = $props();

	const DEFAULT_ICON = {
		warning: 'triangle-alert',
		danger: 'triangle-alert',
		info: 'info',
		success: 'check'
	} as const;

	const glyph = $derived(UTILITY_ICONS[callout.icon ?? DEFAULT_ICON[callout.tone]]);
</script>

<div class="callout {callout.tone}">
	<span class="glyph"><Icon icon={glyph} size="md" /></span>
	<p>{callout.text}</p>
</div>

<style>
	.callout {
		display: flex;
		align-items: flex-start;
		gap: var(--space-lg);
		padding: var(--space-lg);
		border-radius: var(--radius-lg);
		border: var(--border-hairline) solid transparent;
	}

	.glyph {
		display: flex;
		flex-shrink: 0;
		/* Optical alignment with the first line, not the box. */
		margin-block-start: var(--space-xs);
	}

	p {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		white-space: pre-line;
	}

	.warning {
		background: var(--color-warning-soft);
		border-color: var(--color-warning-border);
		color: var(--color-warning-base);
	}

	.danger {
		background: var(--color-error-soft);
		border-color: color-mix(in srgb, var(--color-error-base) 30%, transparent);
		color: var(--color-error-base);
	}

	.info {
		background: var(--color-info-soft);
		border-color: color-mix(in srgb, var(--color-info-base) 30%, transparent);
		color: var(--color-info-base);
	}

	.success {
		background: var(--color-success-soft);
		border-color: color-mix(in srgb, var(--color-success-base) 30%, transparent);
		color: var(--color-success-base);
	}
</style>
