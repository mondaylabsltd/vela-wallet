<script lang="ts">
	/**
	 * The keys that control this wallet, and their Ethereum backup (spec 062).
	 *
	 * One block on both layouts. A key row says three things a person can act
	 * on: what it is called, WHO is holding it (the vault's own mark and name
	 * when the core's catalog knows — Apple Passwords, 1Password, a security
	 * key), and whether it is synced or lives on one device. The backup sits
	 * under the keys it backs up, as their last row, instead of floating on the
	 * page as a sentence about nothing in particular.
	 *
	 * De-containered, hairline-divided, like the rest of settings: these are
	 * facts to read, not cards to tap. Only the backup is a button, and only
	 * while there is something to do.
	 */
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import PasskeyProviderMark from '$lib/ui/onboarding/v2/PasskeyProviderMark.svelte';
	import { providerLabel } from '$lib/onboarding/core/passkey-directory.svelte';
	import { isDarkTheme } from '$lib/theme.svelte';
	import type { WalletKeysModel } from '../model';

	interface Props {
		model: WalletKeysModel;
		onbackup?: () => void;
	}

	let { model, onbackup }: Props = $props();
</script>

<section class="keys" aria-label={model.title}>
	<header>
		<h2>
			{model.title}
			{#if model.count !== ''}<span class="count">{model.count}</span>{/if}
		</h2>
		<p class="subtitle">{model.subtitle}</p>
	</header>

	{#if model.loading}
		<!-- The shape of one row, so the block does not jump when the answer lands. -->
		<div class="row skeleton" aria-hidden="true">
			<span class="mark-slot"></span>
			<span class="who"><span class="bar wide"></span><span class="bar"></span></span>
		</div>
	{:else}
		<ul>
			{#each model.rows as row, index (index)}
				{@const holder =
					providerLabel(row.key.provider_name, row.key.aaguid, isDarkTheme()) ?? row.holderFallback}
				<li class="row">
					<span class="mark-slot">
						<PasskeyProviderMark key={row.key} label={holder} glyphFallback />
					</span>
					<span class="who">
						<span class="name">{row.name}</span>
						<span class="meta">
							{holder}{#if row.fingerprint !== ''}<span class="sep" aria-hidden="true">·</span><span
									class="fingerprint">{row.fingerprint}</span
								>{/if}
						</span>
					</span>
					{#if row.badge}
						<span class="badge" data-tone={row.badge.tone}>{row.badge.text}</span>
					{/if}
				</li>
			{/each}
		</ul>
		{#if model.note}<p class="note">{model.note}</p>{/if}
	{/if}

	{#if model.backup}
		{@const backup = model.backup}
		<button
			type="button"
			class="row backup"
			data-tone={backup.tone}
			disabled={!backup.actionable}
			onclick={() => onbackup?.()}
		>
			<span class="mark-slot"><Icon icon={UTILITY_ICONS.upload} size="md" /></span>
			<span class="who">
				<span class="name">{backup.title}</span>
				<span class="meta state">{backup.subtitle}</span>
			</span>
			{#if backup.actionable}
				<Icon icon={UTILITY_ICONS['chevron-right']} size="sm" />
			{:else if backup.tone === 'positive'}
				<span class="done"><Icon icon={UTILITY_ICONS.check} size="sm" /></span>
			{/if}
		</button>
	{/if}
</section>

<style>
	.keys {
		display: flex;
		flex-direction: column;
		font-family: var(--font-ui);
	}

	header {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		margin-bottom: var(--space-md);
	}

	h2 {
		display: flex;
		align-items: baseline;
		gap: var(--space-md);
		margin: 0;
		color: var(--color-fg-base);
		font-size: calc(var(--text-xl) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
	}

	.count {
		color: var(--color-fg-muted);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-regular);
		font-variant-numeric: tabular-nums;
	}

	.subtitle,
	.note {
		margin: 0;
		color: var(--color-fg-muted);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-relaxed);
	}

	.note {
		padding-block: var(--space-md);
	}

	ul {
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.row {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		width: 100%;
		padding-block: var(--space-lg);
		border-bottom: var(--border-hairline) solid var(--color-border-base);
	}

	.mark-slot {
		display: grid;
		flex: none;
		place-items: center;
		/* The settings rows' own icon column, so names line up with the page. */
		width: var(--icon-2xl);
		height: var(--space-4xl);
		color: var(--color-fg-muted);
	}

	.who {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: var(--space-xs);
		min-width: 0;
	}

	.name {
		overflow: hidden;
		color: var(--color-fg-base);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.meta {
		color: var(--color-fg-muted);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
	}

	.sep {
		margin-inline: var(--space-sm);
	}

	.fingerprint {
		font-family: var(--font-mono);
	}

	.badge {
		flex: none;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
	}

	.badge[data-tone='synced'] {
		color: var(--color-success-base);
	}

	.badge[data-tone='local'] {
		color: var(--color-fg-muted);
	}

	/* The backup: the block's last row, and its only button. */
	.backup {
		padding-inline: 0;
		border: none;
		border-bottom: none;
		color: inherit;
		background: none;
		font-family: var(--font-ui);
		text-align: start;
		cursor: pointer;
	}

	.backup:disabled {
		cursor: default;
	}

	.backup[data-tone='positive'] .state,
	.done {
		color: var(--color-success-base);
	}

	.backup[data-tone='caution'] .state {
		color: var(--color-warning-base);
	}

	.backup:not(:disabled):hover .name {
		color: var(--color-accent-base);
	}

	.skeleton .bar {
		display: block;
		width: 30%;
		height: var(--space-md);
		border-radius: var(--radius-full);
		background: var(--color-border-base);
	}

	.skeleton .bar.wide {
		width: 45%;
	}

	.skeleton .mark-slot {
		border-radius: var(--radius-full);
		background: var(--color-border-base);
	}
</style>
