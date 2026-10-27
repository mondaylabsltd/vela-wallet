<script lang="ts">
	/**
	 * Spec 103 — Settings → AI agent: the switch, the bridge this wallet dials
	 * and the token it presents, then what the connection is doing and where
	 * the person's agent connects to reach it.
	 *
	 * A field is applied when it is left, as on every other endpoint surface:
	 * the connection is redialled once per edit, not once per keystroke.
	 *
	 * Spec 106: the usual way in is one button — connect with clientmcp.dev —
	 * and the fields fold under "Use your own bridge". Connected that way, the
	 * panel says whose it is and where the person connects their agent.
	 */
	import Button from '$lib/ui/Button.svelte';
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
		onlink?: () => void;
		onunlink?: () => void;
		onwallet?: (address: string, on: boolean) => void;
	}

	let { panel, onenable, onbridge, ontoken, onlink, onunlink, onwallet }: Props = $props();
	const link = $derived(panel.link);

	/** The agent's address: masked until Show — it is a credential. */
	let revealed = $state(false);
	let copied = $state(false);
	const canShare = typeof navigator !== 'undefined' && typeof navigator.share === 'function';

	function masked(url: string): string {
		const at = url.lastIndexOf('/');
		return at < 0 ? '••••••••••••' : `${url.slice(0, at + 1)}••••••••••••`;
	}

	async function copyAddress(url: string): Promise<void> {
		try {
			await navigator.clipboard.writeText(url);
			copied = true;
			setTimeout(() => (copied = false), 1800);
		} catch {
			/* Clipboard refused: Show lets the person select it instead. */
			revealed = true;
		}
	}

	/** What is typed into a field and not yet applied. */
	let bridgeDraft: string | null = null;
	let tokenDraft: string | null = null;
</script>

<p class="description">{panel.description}</p>

{#if link.connectedAs !== undefined}
	<section class="linked" data-testid="agent-linked">
		<div>
			<p class="who">{link.connectedAs}</p>
			{#if link.clientId !== undefined}
				<p class="client" data-testid="agent-client-id">{link.clientId}</p>
			{/if}
		</div>
		<div data-testid="agent-status"><StatusPill pill={panel.status} /></div>
	</section>

	<div class="switch"><SwitchRow row={panel.enable} onchange={onenable} /></div>

	{#if link.agentAddress !== undefined}
		{@const address = link.agentAddress}
		<section class="address" data-testid="agent-address">
			<p class="label">{address.label}</p>
			<p class="url" data-testid="agent-address-value">
				{revealed ? address.value : masked(address.value)}
			</p>
			<div class="address-actions">
				<button type="button" class="ghost" onclick={() => (revealed = !revealed)}
					>{revealed ? address.hide : address.show}</button
				>
				{#if canShare}
					<button
						type="button"
						class="ghost"
						onclick={() => void navigator.share({ text: address.value }).catch(() => {})}
						>{address.share}</button
					>
				{/if}
				<button
					type="button"
					class="copy"
					data-testid="agent-address-copy"
					onclick={() => void copyAddress(address.value)}
					>{copied ? address.copied : address.copy}</button
				>
			</div>
			<p class="hint">{address.hint}</p>
		</section>
	{/if}

	{#if link.agent !== undefined}
		<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- clientmcp.dev, outside the app -->
		<a class="agent" href={link.agent.href} target="_blank" rel="noreferrer noopener">
			<span class="agent-label">{link.agent.label} →</span>
			<span class="hint">{link.agent.hint}</span>
		</a>
	{/if}

	<div class="actions">
		<Button variant="secondary" shape="rounded" onclick={onlink}>{link.reconnect}</Button>
		<Button variant="secondary" shape="rounded" onclick={onunlink}>{link.disconnect}</Button>
	</div>
{:else}
	<div class="connect">
		<Button
			variant="secondary"
			shape="rounded"
			testid="agent-connect"
			loading={link.busy !== undefined}
			busyLabel={link.busy}
			onclick={onlink}>{link.connect}</Button
		>
		<p class="hint">{link.connectHint}</p>
		{#if link.problem !== undefined}
			<p class="problem" role="alert" data-testid="agent-link-problem">{link.problem}</p>
		{/if}
	</div>

	<details class="own" open={link.ownOpen}>
		<summary>{link.ownBridge}</summary>
		<div class="switch"><SwitchRow row={panel.enable} onchange={onenable} /></div>

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
	</details>
{/if}

<section class="wallets" data-testid="agent-wallets">
	<p class="wallets-title">{panel.wallets.title}</p>
	<p class="hint">{panel.wallets.hint}</p>
	{#if panel.wallets.none !== undefined}
		<p class="problem" data-testid="agent-wallets-none">{panel.wallets.none}</p>
	{/if}
	{#each panel.wallets.rows as row (row.id)}
		<SwitchRow {row} onchange={(on) => onwallet?.(row.id.replace(/^agent-wallet-/, ''), on)} />
	{/each}
</section>

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

	.linked {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-lg);
		margin-block-end: var(--space-xl);
	}

	.who {
		margin: 0;
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
		overflow-wrap: anywhere;
	}

	.client {
		margin: var(--space-xs) 0 0;
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
		overflow-wrap: anywhere;
	}

	.agent {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		padding: var(--space-lg);
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
		color: var(--color-fg-base);
		text-decoration: none;
		margin-block-end: var(--space-xl);
	}

	.agent-label {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
	}

	.address {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
		padding: var(--space-lg);
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
		margin-block-end: var(--space-xl);
	}

	.address p {
		margin: 0;
	}

	.address-actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-md);
	}

	.address-actions button {
		font: inherit;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		padding: var(--space-sm) var(--space-lg);
		border-radius: var(--radius-full);
		cursor: pointer;
	}

	.ghost {
		border: 0;
		background: transparent;
		color: var(--color-fg-muted);
	}

	.copy {
		border: var(--border-hairline) solid var(--color-border-strong);
		background: var(--color-bg-base);
		color: var(--color-fg-base);
	}

	.actions {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: var(--space-md);
		margin-block-end: var(--space-xl);
	}

	.connect {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--space-md);
		margin-block-end: var(--space-3xl);
	}

	.connect .hint,
	.connect .problem {
		margin: 0;
	}

	.problem {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-error-base);
	}

	.own {
		margin-block-end: var(--space-xl);
	}

	.wallets {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
		margin-block: var(--space-xl);
	}

	.wallets p {
		margin: 0;
	}

	.wallets-title {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.own summary {
		cursor: pointer;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		margin-block-end: var(--space-xl);
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
