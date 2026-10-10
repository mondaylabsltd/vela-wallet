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
		/** The places a key may be minted in — always the three (spec 102). */
		addMethods?: KeyMethod[];
		/**
		 * The corpus key of the heading over the three places (issue 475) — the
		 * core's `CreateView.add_heading_key`: "Add a passkey" with no key yet,
		 * "Add another" with room for one more, "Limit of 7 reached" at the cap.
		 * It is the screen's ONLY add affordance.
		 */
		addHeadingKey: string;
		/**
		 * The core's `CreateView.methods_pinned`: no key yet and one may be
		 * added, so the three places are drawn open under a plain heading, with
		 * no fold to tap. Otherwise they fold under the heading.
		 */
		methodsPinned: boolean;
		/**
		 * Spec 102: a signing page the core's view says was chosen, and the
		 * domain this wallet's keys are minted for — then every key belongs to
		 * THAT site, said before it is too late to choose otherwise. The web
		 * offers no "Use a trusted signing page" entry (P2b-W3), so its view
		 * never holds one; drawn only if the core ever says so.
		 */
		signingDomain?: string;
		signingPage?: string | null;
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
		addMethods,
		addHeadingKey,
		methodsPinned,
		signingDomain = '',
		signingPage = null,
		strings,
		onAddKey,
		onConfirmKey,
		onRemoveKey,
		onFinish
	}: Props = $props();

	let pickerOpen = $state(false);

	const full = $derived(keys.length >= maxKeys);

	// WHEN the three places are held open is the core's (`methods_pinned`,
	// issue 475): with no key yet the first key's method is the person's choice
	// too (the whole Xiaomi lock-out fix), and a list with nothing on it plus a
	// collapsed "+" is a puzzle, not a step. Past that they fold under the
	// heading, and the fold's open/closed state is this screen's.
	const pickerShown = $derived(methodsPinned || (pickerOpen && canAddKey));

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

	{#if signingPage}
		<!--
			Spec 102: a signing page was chosen. Every key below is minted for
			that page's domain and signs only there — said before the first key,
			because that is when the choice is still free.
		-->
		<div class="page">
			<span class="pagedomain">{strings('settings.signing.keysOn', { domain: signingDomain })}</span
			>
			<span class="pageurl">{signingPage}</span>
		</div>
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
				<!-- Captioned by where the key lives (spec 102: three places, no fourth). -->
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

	<!--
		ONE add affordance (issue 475): the core's heading over the three
		places. With no key yet it is a plain heading and the places stand open
		beneath it — there used to be a "+ Add a passkey" button here too, above
		the places it could not fold, saying the same thing twice and doing
		nothing. With a key or more the heading IS the fold ("+ Add another");
		at the cap it is a statement, and there is nothing under it.
	-->
	<div class="add">
		{#if methodsPinned}
			<h2 class="label">{strings(addHeadingKey)}</h2>
		{:else}
			<button
				class="fold"
				type="button"
				disabled={!canAddKey}
				aria-expanded={canAddKey ? pickerShown : undefined}
				onclick={() => (pickerOpen = !pickerOpen)}
			>
				{#if canAddKey}<span class="plus" class:open={pickerShown} aria-hidden="true">+</span>{/if}
				<span>{strings(addHeadingKey)}</span>
			</button>
		{/if}
		<AddMethodPicker open={pickerShown} allowed={addMethods} {strings} onPick={pick} />
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

	/* The chosen page: which site every key will belong to, said once, calmly. */
	.page {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		padding: var(--space-md) var(--space-lg);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-md);
		background: var(--color-bg-sunken);
	}

	.pagedomain {
		color: var(--color-fg-base);
		font-size: var(--text-sm);
		font-weight: var(--weight-semibold);
	}

	.pageurl {
		color: var(--color-fg-muted);
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		overflow-wrap: anywhere;
	}

	.listhead {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	/* A section's label: "Added" over the keys, and — with no key yet — the
	   core's heading over the three places. */
	.label {
		margin: 0;
		color: var(--color-fg-muted);
		font-size: var(--text-sm);
		font-weight: var(--weight-semibold);
		line-height: var(--leading-normal);
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

	/* Neutral, not a warning. "Device-bound" is a KIND of passkey (a security
	   key, Windows Hello), not a fault: it cannot be synced, and there is
	   nothing to go and fix. A warning tone read as "go and sync this", which is
	   exactly what the owner ruled out. The real risk (every key device-bound)
	   is said by the "add a second key" hint, where it can be acted on. */
	.badge[data-tone='local'] {
		color: var(--color-fg-muted);
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

	/* The heading as a fold: tap to open the three places, tap to close. */
	.fold {
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

	.fold:hover:not(:disabled) {
		border-color: var(--color-accent-base);
	}

	.fold:disabled {
		opacity: var(--opacity-disabled);
		cursor: default;
	}

	.plus {
		font-size: var(--text-xl);
		line-height: var(--leading-none);
		transition: transform var(--motion-duration-fast) ease;
	}

	/* Open, the "+" turns to a "×": the same control closes what it opened. */
	.plus.open {
		transform: rotate(45deg);
	}

	@media (prefers-reduced-motion: reduce) {
		.plus {
			transition: none;
		}
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
