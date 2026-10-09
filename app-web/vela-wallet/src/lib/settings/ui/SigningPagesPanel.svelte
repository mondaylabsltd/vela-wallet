<script lang="ts">
	/**
	 * Settings → Signing pages (spec 102, ST18 / DST9): the pages this device
	 * trusts to show and sign requests.
	 *
	 * A LIST, not the free-text field spec 071 had. A field any message could
	 * talk a person into overwriting ("just paste this address") was a door;
	 * a list keeps every page the person chose, the official one always first
	 * and never removable, and which page an ACCOUNT signs on stays that
	 * account's own choice ("Where you review and sign").
	 *
	 * Every row says the two things a person needs before trusting a page:
	 * which keys it can reach ("Keys on …" — a page can only use its own
	 * domain's passkeys, R1) and what was checked about it (its integrity line).
	 * Removing a page changes no account (D-12).
	 */
	import type { SigningPagesModel } from '../model';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import IntegrityLine from './IntegrityLine.svelte';
	import UrlField from './UrlField.svelte';

	interface Props {
		panel: SigningPagesModel;
		/** Absent in the gallery, where the panel is a picture. */
		onadd?: (url: string) => void;
		onrename?: (url: string) => void;
		onremove?: (url: string) => void;
	}

	let { panel, onadd, onrename, onremove }: Props = $props();

	let draft = $state('');
</script>

<ul class="pages">
	{#each panel.rows as row (row.url)}
		<li class="page">
			<span class="glyph" aria-hidden="true"
				><Icon icon={UTILITY_ICONS['shield-check']} size="lg" /></span
			>
			<div class="body">
				<div class="head">
					<span class="name">{row.name}</span>
					{#if !row.official}
						<span class="actions">
							<button type="button" class="action" onclick={() => onrename?.(row.url)}
								>{panel.renameLabel}</button
							>
							<button type="button" class="action danger" onclick={() => onremove?.(row.url)}
								>{panel.removeLabel}</button
							>
						</span>
					{/if}
				</div>
				<span class="where"
					><span class="host">{row.host}</span><span class="sep" aria-hidden="true">·</span><span
						>{row.keysOn}</span
					></span
				>
				{#if row.integrity !== undefined}
					<IntegrityLine line={row.integrity} />
				{/if}
			</div>
		</li>
	{/each}
</ul>

<div class="add">
	<UrlField
		field={{ ...panel.add, value: draft || panel.add.value }}
		action={panel.addAction}
		oninput={(value) => (draft = value)}
		onaction={() => onadd?.(draft || panel.add.value)}
	/>
</div>

<style>
	.pages {
		display: flex;
		flex-direction: column;
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.page {
		display: flex;
		align-items: flex-start;
		gap: var(--space-lg);
		padding-block: var(--space-xl);
		border-bottom: var(--border-hairline) solid var(--color-border-base);
	}

	.glyph {
		display: flex;
		flex-shrink: 0;
		color: var(--color-fg-muted);
	}

	.body {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: var(--space-sm);
		min-width: 0;
	}

	.head {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-md);
	}

	.name {
		min-width: 0;
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
		overflow-wrap: anywhere;
	}

	.where {
		display: flex;
		flex-wrap: wrap;
		column-gap: var(--space-md);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.host {
		font-family: var(--font-mono);
		overflow-wrap: anywhere;
	}

	.actions {
		display: flex;
		flex-shrink: 0;
		gap: var(--space-lg);
	}

	/* Links, not CTAs: accent is reserved for actions that move value. */
	.action {
		padding: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-info-base);
		cursor: pointer;
	}

	.action.danger {
		color: var(--color-error-base);
	}

	.add {
		padding-block-start: var(--space-3xl);
	}
</style>
