<script lang="ts">
	/**
	 * Spec 103 — Settings → AI agent: the switch, the bridge this wallet dials
	 * and the token it presents, then what the connection is doing and where
	 * the person's agent connects to reach it.
	 *
	 * A field is applied when it is left, as on every other endpoint surface:
	 * the connection is redialled once per edit, not once per keystroke.
	 */
	import type { AgentPanelModel } from '../model';
	import KeyValueRow from './KeyValueRow.svelte';
	import StatusPill from './StatusPill.svelte';
	import SwitchRow from './SwitchRow.svelte';
	import UrlField from './UrlField.svelte';

	interface Props {
		panel: AgentPanelModel;
		/** Absent in the gallery, where the panel is a picture. */
		onenable?: (on: boolean) => void;
		onbridge?: (url: string) => void;
		ontoken?: (token: string) => void;
		ongrant?: (on: boolean) => void;
	}

	let { panel, onenable, onbridge, ontoken, ongrant }: Props = $props();

	/** What is typed into a field and not yet applied. */
	let bridgeDraft: string | null = null;
	let tokenDraft: string | null = null;
</script>

<p class="description">{panel.description}</p>

<div class="switch"><SwitchRow row={panel.enable} onchange={onenable} /></div>
{#if panel.grant !== undefined}
	<div class="switch"><SwitchRow row={panel.grant} onchange={ongrant} /></div>
{/if}

<div class="fields">
	<UrlField
		field={panel.bridge}
		oninput={(value) => (bridgeDraft = value)}
		onblur={() => {
			if (bridgeDraft !== null && bridgeDraft !== panel.bridge.value) onbridge?.(bridgeDraft);
			bridgeDraft = null;
		}}
	/>
	<UrlField
		field={panel.token}
		oninput={(value) => (tokenDraft = value)}
		onblur={() => {
			if (tokenDraft !== null && tokenDraft !== panel.token.value) ontoken?.(tokenDraft);
			tokenDraft = null;
		}}
	/>
</div>

<div class="status" data-testid="agent-status">
	<StatusPill pill={panel.status} />
</div>

{#if panel.endpoint !== undefined}
	<section class="endpoint">
		<p class="label">{panel.endpoint.label}</p>
		<p class="url" data-testid="agent-endpoint">{panel.endpoint.value}</p>
		<p class="hint">{panel.endpoint.hint}</p>
	</section>
{/if}

{#if panel.lastAction !== undefined}
	<KeyValueRow row={{ label: panel.lastAction.label, value: panel.lastAction.value, mono: true }} />
{/if}

<style>
	.description {
		margin: 0 0 var(--space-xl);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
	}

	.switch {
		border-bottom: var(--border-hairline) solid var(--color-border-base);
		margin-block-end: var(--space-3xl);
	}

	.fields {
		display: flex;
		flex-direction: column;
		gap: var(--space-3xl);
	}

	.status {
		display: flex;
		margin-block: var(--space-3xl) var(--space-xl);
	}

	.endpoint {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
		padding: var(--space-lg);
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
		margin-block-end: var(--space-xl);
	}

	.endpoint p {
		margin: 0;
	}

	.label {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		letter-spacing: var(--letterSpacing-sectionLabel);
		color: var(--color-fg-subtle);
	}

	.url {
		font-family: var(--font-mono);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
		overflow-wrap: anywhere;
		user-select: all;
	}

	.hint {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-subtle);
	}
</style>
