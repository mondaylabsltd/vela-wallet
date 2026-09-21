<script lang="ts">
	/**
	 * The founding key list — the screen spec 014 never had, and the only place
	 * a multi-key wallet can be assembled.
	 *
	 * Everything on it is a rendering of `CreateView`; nothing here decides. The
	 * three gates the core enforces (at most seven keys, every key confirmed, a
	 * sole key must be backed up) surface as a disabled control with a stated
	 * reason rather than as a click that quietly does nothing.
	 */
	import Button from '$lib/ui/Button.svelte';
	import AddMethodPicker from './AddMethodPicker.svelte';
	import { keyBadge, providerLineFor } from '$lib/onboarding/core/copy';
	import PasskeyProviderMark from './PasskeyProviderMark.svelte';
	import { providerLabel } from '$lib/onboarding/core/passkey-directory.svelte';
	import { isDarkTheme } from '$lib/theme.svelte';
	import type { CreateKeyRow } from '$lib/onboarding/generated/CreateKeyRow';
	import type { KeyMethod } from '$lib/onboarding/generated/KeyMethod';

	interface Props {
		keys: CreateKeyRow[];
		canAddKey: boolean;
		canFinish: boolean;
		needsSecondKey: boolean;
		busy: boolean;
		maxKeys: number;
		strings: (key: string, params?: Record<string, string | number>) => string;
		onAddKey: (method: KeyMethod) => void;
		onConfirmKey: (index: number) => void;
		onRemoveKey: (index: number) => void;
		onFinish: () => void;
	}

	let {
		keys,
		canAddKey,
		canFinish,
		needsSecondKey,
		busy,
		maxKeys,
		strings,
		onAddKey,
		onConfirmKey,
		onRemoveKey,
		onFinish
	}: Props = $props();

	let pickerOpen = $state(false);

	const full = $derived(keys.length >= maxKeys);

	// An EMPTY list keeps the three methods expanded: the first key's method is
	// the person's choice too (the whole Xiaomi lock-out fix), and a list with
	// nothing on it plus a collapsed "+" is a puzzle, not a step.
	const pickerShown = $derived((pickerOpen || keys.length === 0) && canAddKey);

	const subtitle = $derived(
		needsSecondKey
			? strings('onboarding.create.keysSubtitleBlocked')
			: full
				? strings('onboarding.create.keysSubtitleFull')
				: strings('onboarding.create.keysSubtitle')
	);

	function pick(method: KeyMethod) {
		pickerOpen = false;
		onAddKey(method);
	}
</script>

<section class="screen">
	<header class="intro">
		<h1 class="title">
			{needsSecondKey
				? strings('onboarding.create.keysTitleBlocked')
				: strings('onboarding.create.keysTitle')}
		</h1>
		<p class="subtitle">{subtitle}</p>
	</header>

	{#if needsSecondKey}
		<p class="warning">
			<span class="dot" aria-hidden="true"></span>
			<span>{strings('onboarding.create.needSecondKeyHint')}</span>
		</p>
	{/if}

	<div class="list">
		<div class="listhead">
			<span class="label">{strings('onboarding.create.keysLabel')}</span>
			<span class="count"
				>{strings('onboarding.create.keyCount', { current: keys.length, max: maxKeys })}</span
			>
		</div>

		<ul class="rows">
			{#each keys as key, index (index)}
				{@const badge = keyBadge(key, strings)}
				{@const holder = providerLabel(key.provider_name, key.aaguid, isDarkTheme())}
				{@const where = holder ?? strings(providerLineFor(key.kind))}
				<li class="row">
					<!--
						Who is holding this key, when the core's AAGUID catalog knows:
						the vault's own mark and its own name. When it does not — a
						hardware key, an authenticator that reported nothing — the row
						says where the key LIVES, from the authenticator's own report
						(issue 207). Icon and caption read the same field, so they
						cannot contradict each other.
					-->
					<PasskeyProviderMark {key} label={where} glyphFallback />
					<span class="who">
						<span class="name">{key.name}</span>
						<span class="meta">{where}</span>
					</span>
					<!--
						One trailing slot, as the design draws it. A key that has not
						confirmed its membership has no status to show yet, so the
						retry TAKES that slot rather than crowding in beside it. A key
						whose attestation nobody could read has no badge either: the
						slot stays empty rather than claiming a backup.
					-->
					{#if key.confirmed}
						{#if badge}<span class="badge" data-tone={badge.tone}>{badge.text}</span>{/if}
					{:else}
						<button
							class="confirm"
							type="button"
							disabled={busy}
							onclick={() => onConfirmKey(index)}
						>
							{strings('onboarding.create.confirmKeyBtn')}
						</button>
					{/if}
					{#if index > 0}
						<button
							class="remove"
							type="button"
							disabled={busy}
							aria-label={strings('onboarding.create.removeKeyBtn')}
							onclick={() => onRemoveKey(index)}
						>
							<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18" /></svg>
						</button>
					{/if}
				</li>
			{/each}
		</ul>
	</div>

	<div class="add">
		<button
			class="addtoggle"
			type="button"
			disabled={!canAddKey}
			aria-expanded={pickerShown}
			onclick={() => (pickerOpen = !pickerOpen)}
		>
			<span class="plus" aria-hidden="true">+</span>
			<span>
				{full
					? strings('onboarding.create.keyLimitReached')
					: strings('onboarding.create.addKeyBtn')}
			</span>
		</button>
		<AddMethodPicker open={pickerShown} {strings} onPick={pick} />
	</div>

	<div class="spacer"></div>

	<p class="footnote">{strings('onboarding.create.keysHint')}</p>

	<Button variant="primary" shape="rounded" disabled={!canFinish} loading={busy} onclick={onFinish}>
		{needsSecondKey
			? strings('onboarding.create.addSecondKeyBtn')
			: strings('onboarding.create.createWalletBtn')}
	</Button>
</section>

<style>
	.screen {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: var(--space-xl);
	}

	.intro {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}

	.title {
		margin: 0;
		color: var(--color-fg-base);
		font-size: var(--text-3xl);
		font-weight: var(--weight-bold);
		line-height: var(--leading-tight);
		letter-spacing: -0.015em;
	}

	.subtitle {
		margin: 0;
		color: var(--color-fg-muted);
		font-size: var(--text-lg);
		line-height: var(--leading-normal);
	}

	.warning {
		display: grid;
		grid-template-columns: auto 1fr;
		gap: var(--space-lg);
		align-items: start;
		margin: 0;
		padding: var(--space-xl);
		border-radius: var(--radius-lg);
		background: var(--color-accent-soft);
		color: var(--color-fg-base);
		font-size: var(--text-base);
		line-height: var(--leading-normal);
	}

	.dot {
		width: var(--space-md);
		height: var(--space-md);
		margin-top: var(--space-lg);
		border-radius: var(--radius-full);
		background: var(--color-accent-base);
	}

	.list {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
	}

	.listhead {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.label {
		color: var(--color-fg-muted);
		font-size: var(--text-sm);
		font-weight: var(--weight-semibold);
		letter-spacing: 0.06em;
		text-transform: uppercase;
	}

	.count {
		color: var(--color-fg-muted);
		font-family: var(--font-mono);
		font-size: var(--text-sm);
	}

	.rows {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.row {
		display: flex;
		gap: var(--space-lg);
		align-items: center;
		padding: var(--space-lg) var(--space-xl);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
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
		font-size: var(--text-base);
		font-weight: var(--weight-semibold);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.meta {
		color: var(--color-fg-muted);
		font-size: var(--text-sm);
	}

	.badge {
		font-size: var(--text-sm);
		font-weight: var(--weight-semibold);
		letter-spacing: 0.03em;
	}

	.badge[data-tone='synced'] {
		color: var(--color-success-base);
	}

	.badge[data-tone='local'] {
		color: var(--color-warning-base);
	}

	.confirm {
		padding: 0;
		border: 0;
		background: none;
		color: var(--color-accent-base);
		font-family: var(--font-ui);
		font-size: var(--text-sm);
		font-weight: var(--weight-semibold);
		cursor: pointer;
	}

	/*
	 * The design gives a key row no remove affordance at all — but the core
	 * lets a draft key be dropped, and without one the only way out of a
	 * mistaken key is starting the whole set over. A quiet × after the badge
	 * keeps the row's rhythm while leaving the door open.
	 */
	.remove {
		display: grid;
		flex: 0 0 var(--icon-base);
		place-items: center;
		height: var(--icon-base);
		padding: 0;
		border: 0;
		background: none;
		color: var(--color-fg-subtle);
		cursor: pointer;
		transition: color var(--motion-duration-fast) ease;
	}

	.remove:hover:not(:disabled) {
		color: var(--color-fg-base);
	}

	.remove svg {
		width: var(--icon-sm);
		height: var(--icon-sm);
		fill: none;
		stroke: currentColor;
		stroke-width: var(--icon-stroke-base);
		stroke-linecap: round;
	}

	.confirm:disabled,
	.remove:disabled {
		opacity: var(--opacity-disabled);
		cursor: default;
	}

	.add {
		display: flex;
		flex-direction: column;
	}

	.addtoggle {
		display: flex;
		gap: var(--space-md);
		align-items: center;
		justify-content: center;
		min-height: var(--size-control-md);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-lg);
		background: none;
		color: var(--color-fg-base);
		font-family: var(--font-ui);
		font-size: var(--text-base);
		font-weight: var(--weight-bold);
		cursor: pointer;
		transition: border-color var(--motion-duration-fast) ease;
	}

	.addtoggle:hover:not(:disabled) {
		border-color: var(--color-accent-base);
	}

	.addtoggle:disabled {
		opacity: var(--opacity-disabled);
		cursor: default;
	}

	.plus {
		font-size: var(--text-xl);
		line-height: var(--leading-none);
	}

	.spacer {
		flex: 1;
		min-height: var(--space-md);
	}

	.footnote {
		margin: 0;
		color: var(--color-fg-muted);
		font-size: var(--text-sm);
		line-height: var(--leading-normal);
	}
</style>
