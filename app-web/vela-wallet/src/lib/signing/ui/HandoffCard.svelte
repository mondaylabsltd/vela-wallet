<script lang="ts">
	/**
	 * The hand-off card (spec 102, D4) — what the signing sheet is when the
	 * account reviews and signs on a trusted page.
	 *
	 * It is deliberately NOT a preview. The page decodes the operation itself
	 * and is the authority; a second summary here, drawn from the app's own
	 * (many-library) reading, could disagree with it and leave a person unsure
	 * which to believe. So the card says four things and nothing else: that the
	 * request is reviewed on the page, which key will confirm it (a row, as the
	 * sheet draws "Signing account": 「确认方式 | 手机或平板」), the one line that
	 * backs the word "trusted" (what was checked, and when), and Open — which a
	 * page that failed its check never gets.
	 *
	 * No fee: the sheet's own fee row stays above the card, where the fee and
	 * speed are chosen before the page opens (D-18), so the screen says the fee
	 * once.
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

	/** The page's host, unless its name already says it ("Self-hosted · sign.example.com"). */
	const host = $derived(
		handoff.page.name.includes(handoff.page.host) ? undefined : handoff.page.host
	);
</script>

<section class="handoff" aria-labelledby="handoff-title">
	<!-- The mark says what the line says: a shield only for a page that passed. -->
	<span class="mark" data-tone={handoff.integrity.tone} aria-hidden="true"
		><Icon
			icon={handoff.integrity.tone === 'ok'
				? UTILITY_ICONS['shield-check']
				: handoff.integrity.tone === 'checking'
					? UTILITY_ICONS.clock
					: UTILITY_ICONS['triangle-alert']}
			size="xl"
		/></span
	>
	<h2 id="handoff-title" class="title">{handoff.waiting?.title ?? handoff.title}</h2>
	{#if handoff.waiting}
		<p class="hint">{handoff.waiting.hint}</p>
	{:else}
		<!-- The sheet's own row, label | value — "Signing account" is drawn so. -->
		<div class="key">
			<span class="key-label">{handoff.key.label}</span>
			<span class="key-value">{handoff.key.value}</span>
		</div>
	{/if}

	<div class="page">
		<span class="where">
			<span class="name">{handoff.page.name}</span>
			{#if host !== undefined}<span class="host">{host}</span>{/if}
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

	/* A check still running: no verdict, so no colour. */
	.mark[data-tone='checking'] {
		background: var(--color-bg-sunken);
		color: var(--color-fg-subtle);
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

	.hint {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
	}

	/* The signing sheet's label | value row (`SignerRow`): same sizes, colours
	   and alignment, across the card's whole width. */
	.key {
		display: flex;
		align-self: stretch;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-lg);
		margin-block-start: var(--space-lg);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		text-align: start;
	}

	.key-label {
		color: var(--color-fg-muted);
	}

	.key-value {
		min-width: 0;
		color: var(--color-fg-base);
		text-align: end;
		overflow-wrap: anywhere;
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
