<script lang="ts">
	/**
	 * The keys were minted and the wallet is saved here; the group never landed.
	 *
	 * Nothing is lost and nothing is re-minted. ob-2 (owner, 2026-10-28): the
	 * core writes the account to this device BEFORE the publish, beside a
	 * pending record that carries the whole publish — so this screen says
	 * "saved on this device", never "created" and never "not saved", and a
	 * reload or a closed page loses nothing: the session publishes the record
	 * on its own until it lands, on every launch.
	 *
	 * It does not wait for a tap: the publish replays the proofs the keys
	 * already made (no prompt), so it is retried on its own while the page is
	 * open — when the connection comes back, and on a widening timer, the wait
	 * counted down where the person can see it. Entering the wallet is offered
	 * meanwhile; starting over is not, because the wallet exists (the core's
	 * `show_start_over` says so).
	 */
	import { onMount } from 'svelte';
	import Button from '$lib/ui/Button.svelte';
	import TechDetails from '../TechDetails.svelte';

	const RETRY_DELAYS_MS = [5_000, 10_000, 20_000, 40_000, 60_000];

	interface Props {
		/** The publish's own error, forwarded verbatim — it goes into the bug
		 *  report, so prettifying it here would lose the only detail worth
		 *  filing. */
		detail: string | null;
		/**
		 * r18-8 (§32): the core's `sync_error_key` — the failure's own sentence
		 * (this test build refused to write to the live index). Said in place
		 * of "Vela keeps trying on its own", and nothing is tried on its own:
		 * the refusal would only repeat (Retry stays the person's).
		 */
		wordsKey?: string | null;
		busy: boolean;
		strings: (key: string) => string;
		onRetry: () => void;
		/** Enter the saved wallet; the session finishes the sync from there. */
		onEnter: () => void;
		/** The core's `show_start_over` — false once the wallet is saved. */
		showStartOver?: boolean;
		onStartOver?: () => void;
		/** How many times this screen has shown already, for the wait below. */
		round?: number;
		/** The widening wait between retries on their own (the last repeats). */
		retryDelaysMs?: number[];
	}

	let {
		detail,
		wordsKey = null,
		busy,
		strings,
		onRetry,
		onEnter,
		showStartOver = false,
		onStartOver,
		round = 0,
		retryDelaysMs = RETRY_DELAYS_MS
	}: Props = $props();

	/** Whole seconds to the next try on its own, as drawn. */
	let remaining = $state<number | null>(null);

	// One retry per showing: a failed retry brings this screen back with the
	// next `round`, and so the next, longer wait.
	onMount(() => {
		if (wordsKey) return;
		const retry = () => {
			if (!busy) onRetry();
		};
		const delay = retryDelaysMs[Math.min(round, retryDelaysMs.length - 1)];
		const due = Date.now() + delay;
		const tick = () => {
			remaining = Math.max(0, Math.ceil((due - Date.now()) / 1000));
		};
		tick();
		const ticker = setInterval(tick, 1000);
		const timer = setTimeout(retry, delay);
		window.addEventListener('online', retry);
		return () => {
			clearTimeout(timer);
			clearInterval(ticker);
			window.removeEventListener('online', retry);
		};
	});

	const next = $derived(
		busy
			? strings('onboarding.create.statusSyncingKey')
			: remaining === null
				? null
				: strings('onboarding.create.syncFailedNext').replace('{{seconds}}', String(remaining))
	);
</script>

<section class="screen">
	<header class="intro">
		<h1 class="title">{strings('onboarding.create.syncFailedTitle')}</h1>
		<p class="subtitle">{strings('onboarding.create.syncFailedMessage')}</p>
		{#if wordsKey}
			<p class="hint" role="status" data-testid="sync-failed-words">{strings(wordsKey)}</p>
		{:else}
			<p class="hint">{strings('onboarding.create.syncFailedHint')}</p>
		{/if}
		{#if next !== null}
			<p class="next" role="status">{next}</p>
		{/if}
	</header>

	{#if detail}
		<TechDetails
			label={strings('onboarding.create.technicalDetails')}
			code={detail}
			context={strings('onboarding.create.statusSyncingKey')}
		/>
	{/if}

	<div class="spacer"></div>

	<div class="actions">
		<Button variant="primary" shape="rounded" disabled={busy} onclick={onEnter}>
			{strings('onboarding.create.enterWalletBtn')}
		</Button>
		<Button variant="secondary" shape="rounded" loading={busy} onclick={onRetry}>
			{strings('onboarding.create.retryUploadBtn')}
		</Button>
		{#if showStartOver && onStartOver}
			<Button variant="secondary" shape="rounded" disabled={busy} onclick={onStartOver}>
				{strings('onboarding.create.startOverBtn')}
			</Button>
		{/if}
	</div>
</section>

<style>
	.screen {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: var(--space-3xl);
	}

	.intro {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}

	.title {
		margin: 0;
		color: var(--color-fg-base);
		font-size: calc(var(--text-3xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		line-height: var(--leading-tight);
		letter-spacing: -0.015em;
	}

	.subtitle {
		margin: 0;
		color: var(--color-fg-muted);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		line-height: var(--leading-normal);
	}

	.hint,
	.next {
		margin: 0;
		color: var(--color-fg-muted);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
	}

	.next {
		font-variant-numeric: tabular-nums;
	}

	.spacer {
		flex: 1;
		min-height: var(--space-md);
	}

	.actions {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
	}
</style>
