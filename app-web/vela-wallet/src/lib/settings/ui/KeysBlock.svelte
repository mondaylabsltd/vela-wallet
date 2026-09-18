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

	/** Which rows are open. By position: the list is the founding order. */
	let open = $state<Record<number, boolean>>({});
	/** `"row:label"` of the value just copied, for the button's one-second "Copied". */
	let copied = $state('');
	let copiedTimer: ReturnType<typeof setTimeout> | undefined;

	async function copy(id: string, value: string) {
		try {
			await navigator.clipboard.writeText(value);
			copied = id;
			clearTimeout(copiedTimer);
			copiedTimer = setTimeout(() => (copied = ''), 1200);
		} catch {
			// No clipboard permission: the value is on screen and selectable.
		}
	}
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
				{@const expandable = row.details.length > 0}
				<li class="key" class:open={open[index] === true}>
					<!--
						The row is the summary; opening it shows what the registry holds
						about this key — the explorer's facts, here where the person is
						deciding to back them up. Nothing to open when only the device
						answered, and then it is not a button.
					-->
					{#snippet summary()}
						<span class="mark-slot">
							<PasskeyProviderMark key={row.key} label={holder} glyphFallback />
						</span>
						<span class="who">
							<span class="name">{row.name}</span>
							<span class="meta">
								{holder}{#if row.fingerprint !== ''}<span class="sep" aria-hidden="true">·</span
									><span class="fingerprint">{row.fingerprint}</span>{/if}
							</span>
						</span>
						<span class="pills">
							{#each row.pills as pill (pill.text)}
								<span class="pill" data-tone={pill.tone}>{pill.text}</span>
							{/each}
						</span>
					{/snippet}
					{#if expandable}
						<button
							type="button"
							class="row summary"
							aria-expanded={open[index] === true}
							onclick={() => (open[index] = !open[index])}
						>
							{@render summary()}
							<span class="chevron"><Icon icon={UTILITY_ICONS['chevron-down']} size="sm" /></span>
						</button>
					{:else}
						<div class="row summary">{@render summary()}</div>
					{/if}

					{#if expandable && open[index] === true}
						<dl class="details">
							{#each row.details as detail (detail.label)}
								{@const id = `${index}:${detail.label}`}
								<div class="detail">
									<dt>{detail.label}</dt>
									<dd>
										<span class="value" class:mono={detail.mono}>{detail.value}</span>
										{#if detail.copy}
											<button type="button" class="copy" onclick={() => copy(id, detail.value)}>
												{copied === id ? model.copy.done : model.copy.action}
											</button>
										{/if}
									</dd>
								</div>
							{/each}
						</dl>
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
		<p class="explain">{model.backupExplain}</p>
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

	.key {
		border-bottom: var(--border-hairline) solid var(--color-border-base);
	}

	.summary {
		border: none;
		border-bottom: none;
		background: none;
		padding-inline: 0;
		color: inherit;
		font-family: var(--font-ui);
		text-align: start;
	}

	button.summary {
		cursor: pointer;
	}

	.pills {
		display: flex;
		flex: none;
		flex-wrap: wrap;
		justify-content: flex-end;
		gap: var(--space-sm);
	}

	/*
		Below the desktop breakpoint there is no room for a name AND two pills on
		one line — the name lost ("Parallel O…"). The pills take their own line
		under the text, and the chevron stays beside the name.
	*/
	@media not (min-width: 1280px) {
		.summary {
			flex-wrap: wrap;
			row-gap: var(--space-md);
		}

		.chevron {
			order: 2;
		}

		.pills {
			order: 3;
			flex-basis: 100%;
			justify-content: flex-start;
			padding-inline-start: calc(var(--icon-2xl) + var(--space-lg));
		}
	}

	/* The explorer's pills: a hairline capsule, tinted by what it vouches for. */
	.pill {
		padding: var(--space-xs) var(--space-md);
		border: var(--border-hairline) solid currentColor;
		border-radius: var(--radius-full);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		white-space: nowrap;
	}

	.pill[data-tone='verified'] {
		color: var(--color-info-base);
	}

	.pill[data-tone='synced'] {
		color: var(--color-success-base);
	}

	.pill[data-tone='local'] {
		color: var(--color-fg-muted);
	}

	.chevron {
		display: grid;
		flex: none;
		place-items: center;
		color: var(--color-fg-muted);
		transition: transform var(--motion-fast, 120ms) ease-out;
	}

	.open .chevron {
		transform: rotate(180deg);
	}

	@media (prefers-reduced-motion: reduce) {
		.chevron {
			transition: none;
		}
	}

	.details {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
		margin: 0;
		padding: var(--space-xl);
		margin-bottom: var(--space-lg);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
	}

	.detail {
		display: grid;
		grid-template-columns: minmax(0, 7.5em) minmax(0, 1fr);
		gap: var(--space-lg);
		align-items: start;
	}

	dt {
		color: var(--color-fg-muted);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		letter-spacing: 0.06em;
		text-transform: uppercase;
		padding-top: var(--space-xs);
	}

	dd {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-md);
		margin: 0;
		min-width: 0;
	}

	.value {
		min-width: 0;
		color: var(--color-fg-base);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		overflow-wrap: anywhere;
	}

	.value.mono {
		font-family: var(--font-mono);
		/* A key is compared by eye; give the glyphs room. */
		line-height: var(--leading-relaxed);
	}

	.copy {
		flex: none;
		padding: var(--space-xs) var(--space-md);
		border: var(--border-hairline) solid var(--color-border-strong);
		border-radius: var(--radius-md);
		background: none;
		color: var(--color-fg-muted);
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		cursor: pointer;
	}

	.copy:hover {
		color: var(--color-fg-base);
	}

	.explain {
		margin: 0;
		/* Under the backup's text, not under its icon. */
		padding-inline-start: calc(var(--icon-2xl) + var(--space-lg));
		color: var(--color-fg-muted);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-relaxed);
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
