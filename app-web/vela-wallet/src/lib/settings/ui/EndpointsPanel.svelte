<script lang="ts">
	/**
	 * ST12 / DST6 — the four services the wallet leans on, each editable, each
	 * with its own latency badge and a line saying what it is for. The footer
	 * is 恢复默认 (and, on the desktop, the self-hosting guide beside it):
	 * anybody who can point these at their own boxes needs a way back.
	 */
	import type { EndpointsModel } from '../model';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import UrlField from './UrlField.svelte';

	interface Props {
		panel: EndpointsModel;
		onreset?: () => void;
		/** Live wiring (spec 024). Absent = the gallery's pure picture. */
		onfield?: (id: string, value: string) => void;
		onfieldblur?: (id: string) => void;
	}

	let { panel, onreset, onfield, onfieldblur }: Props = $props();
</script>

<p class="description">{panel.description}</p>

<div class="fields">
	{#each panel.fields as field (field.id)}
		<UrlField
			{field}
			oninput={(value) => onfield?.(field.id, value)}
			onblur={() => onfieldblur?.(field.id)}
		/>
	{/each}
</div>

<footer>
	<button type="button" class="reset" onclick={onreset}>
		<Icon icon={UTILITY_ICONS['refresh-cw']} size="sm" />
		<span>{panel.reset}</span>
	</button>
	{#if panel.guide !== undefined}
		<!--
			Spec 081 FR-004: this said "Self-hosting guide →" and opened the
			repository root — a person looking for how to run these services
			landed on a source tree. The guide is a page, and this is it.
		-->
		<a
			class="guide"
			href="https://getvela.app/docs/self-hosting"
			target="_blank"
			rel="noreferrer noopener">{panel.guide}</a
		>
	{/if}
</footer>

<style>
	.description {
		margin: 0 0 var(--space-3xl);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
		white-space: pre-line;
	}

	.fields {
		display: flex;
		flex-direction: column;
		gap: var(--space-3xl);
	}

	footer {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-lg);
		margin-block-start: var(--space-4xl);
	}

	.reset {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		min-height: var(--size-control-md);
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		cursor: pointer;
	}

	.guide {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-info-base);
	}
</style>
