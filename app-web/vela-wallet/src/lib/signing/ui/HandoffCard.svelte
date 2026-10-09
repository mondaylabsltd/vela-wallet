<script lang="ts">
	/**
	 * The hand-off card (spec 102, D4) — what the signing sheet is when the
	 * account reviews and signs on a trusted page.
	 *
	 * It is deliberately NOT a preview. The page decodes the operation itself
	 * and is the authority; a second summary here, drawn from the app's own
	 * (many-library) reading, could disagree with it and leave a person unsure
	 * which to believe. So the card says four things and nothing else: that the
	 * request is reviewed on the page, which key will confirm it, the one line
	 * that backs the word "trusted" (what was checked, and when), and Open —
	 * which a page that failed its check never gets. Under the key, one quiet
	 * row restates the fee and speed the sheet settled (D-18) — no control:
	 * the page signs the operation as it was priced.
	 *
	 * After Open it is a wait: where to look, and a way back to the page.
	 */
	import type { HandoffModel } from '../model';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import Button from '$lib/ui/Button.svelte';
	import IntegrityLine from '$lib/settings/ui/IntegrityLine.svelte';

	interface Props {
		handoff: HandoffModel;
		onopen?: () => void;
	}

	let { handoff, onopen }: Props = $props();
</script>

<section class="handoff" aria-labelledby="handoff-title">
	<!-- The mark says what the line says: a shield only for a page that passed. -->
	<span class="mark" data-tone={handoff.integrity.tone} aria-hidden="true"
		><Icon
			icon={handoff.integrity.tone === 'ok'
				? UTILITY_ICONS['shield-check']
				: UTILITY_ICONS['triangle-alert']}
			size="xl"
		/></span
	>
	<h2 id="handoff-title" class="title">{handoff.waiting?.title ?? handoff.title}</h2>
	<p class="key">{handoff.waiting?.hint ?? handoff.key}</p>
	{#if handoff.fee !== undefined && handoff.waiting === undefined}
		<!-- What the sheet settled, restated; no control (D-18). -->
		<p class="fee">
			<span class="fee-label">{handoff.fee.label}</span>
			<span class="fee-value">{handoff.fee.value}</span>
		</p>
	{/if}

	<div class="page">
		<span class="where">
			<span class="name">{handoff.page.name}</span>
			{#if handoff.page.name !== handoff.page.host}<span class="host">{handoff.page.host}</span
				>{/if}
		</span>
		<IntegrityLine line={handoff.integrity} />
	</div>

	<div class="actions">
		{#if handoff.waiting}
			<Button variant="secondary" shape="rounded" onclick={() => onopen?.()}>
				{handoff.waiting.reopen}
			</Button>
		{:else}
			<Button
				variant="primary"
				shape="rounded"
				disabled={!handoff.open.enabled}
				onclick={() => onopen?.()}
			>
				{handoff.open.label}
			</Button>
		{/if}
	</div>
</section>

<style>
	.handoff {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-md);
		padding-block: var(--space-3xl) var(--space-xl);
		text-align: center;
	}

	.mark {
		display: flex;
		align-items: center;
		justify-content: center;
		width: calc(var(--space-5xl) + var(--space-md));
		height: calc(var(--space-5xl) + var(--space-md));
		border-radius: var(--radius-full);
		background: var(--color-success-soft);
		color: var(--color-success-base);
	}

	.mark[data-tone='warn'] {
		background: var(--color-warning-soft);
		color: var(--color-warning-base);
	}

	.mark[data-tone='error'] {
		background: var(--color-error-soft);
		color: var(--color-error-base);
	}

	.title {
		margin: var(--space-md) 0 0;
		font-size: calc(var(--text-xl) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		line-height: var(--leading-tight);
		color: var(--color-fg-base);
	}

	.key {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
	}

	.fee {
		display: flex;
		flex-wrap: wrap;
		justify-content: center;
		column-gap: var(--space-md);
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-subtle);
	}

	.fee-value {
		color: var(--color-fg-muted);
		font-variant-numeric: tabular-nums;
	}

	/* What is trusted, in one place: the page and what was checked about it. */
	.page {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
		align-self: stretch;
		margin-block-start: var(--space-lg);
		padding: var(--space-lg) var(--space-xl);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
		text-align: start;
	}

	.where {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		column-gap: var(--space-md);
	}

	.name {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.host {
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
		overflow-wrap: anywhere;
	}

	.actions {
		display: flex;
		align-self: stretch;
		margin-block-start: var(--space-xl);
	}

	.actions > :global(*) {
		flex: 1;
	}
</style>
