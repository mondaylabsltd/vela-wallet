<script lang="ts">
	/**
	 * The keys were minted; the group never landed.
	 *
	 * Nothing is lost and nothing is re-minted. The core keeps the whole
	 * founding set and a pending record it wrote BEFORE the first publish
	 * attempt, so retry resumes at the publish — which is why this screen offers
	 * a retry rather than starting over, and why "start over" is the quiet
	 * secondary rather than the obvious escape.
	 */
	import Button from '$lib/ui/Button.svelte';
	import TechDetails from '../TechDetails.svelte';

	interface Props {
		/** The publish's own error, forwarded verbatim — it goes into the bug
		 *  report, so prettifying it here would lose the only detail worth
		 *  filing. */
		detail: string | null;
		busy: boolean;
		strings: (key: string) => string;
		onRetry: () => void;
		onStartOver: () => void;
	}

	let { detail, busy, strings, onRetry, onStartOver }: Props = $props();
</script>

<section class="screen">
	<header class="intro">
		<h1 class="title">{strings('onboarding.create.syncFailedTitle')}</h1>
		<p class="subtitle">{strings('onboarding.create.syncFailedMessage')}</p>
		<p class="hint">{strings('onboarding.create.syncFailedHint')}</p>
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
		<Button variant="primary" shape="rounded" loading={busy} onclick={onRetry}>
			{strings('onboarding.create.retryUploadBtn')}
		</Button>
		<Button variant="secondary" shape="rounded" disabled={busy} onclick={onStartOver}>
			{strings('onboarding.create.startOverBtn')}
		</Button>
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

	.hint {
		margin: 0;
		color: var(--color-fg-muted);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
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
