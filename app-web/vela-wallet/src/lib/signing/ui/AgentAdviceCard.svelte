<script lang="ts">
	import type { AgentAdviceCardModel } from '$lib/agent/advisor';

	/**
	 * The person's AI agent's opinion (spec 103, `vela_advise`), in a card of
	 * its own under Vela's own lines and over the fee — labelled as the
	 * agent's, never one of the sheet's words. It colours nothing but its
	 * chip: the confirm, the sheet's risk and every line above stay as Vela
	 * drew them, and nothing waits for it.
	 *
	 * Plain text only — the core already stripped what could hide or reorder
	 * it, and nothing here renders markup or links: the sources are hosts.
	 * Given for an earlier version of the request on screen, or at a fee that
	 * has since moved past the core's band, it is greyed and says which; the
	 * fee it was given for is said under it either way.
	 */
	interface Props {
		card: AgentAdviceCardModel;
	}

	let { card }: Props = $props();
</script>

<section
	class="advice"
	data-testid="agent-advice"
	data-stale={card.stale ? '' : undefined}
	aria-label={card.title}
>
	<header>
		<h3>{card.title}</h3>
		<span class="chip" data-verdict={card.verdict} data-testid="agent-advice-verdict">
			{card.verdictWord}
		</span>
	</header>
	{#if card.stale}
		<p class="stale" data-testid="agent-advice-stale">{card.staleLine}</p>
	{/if}
	{#if card.feeLine}
		<p class="fee" data-testid="agent-advice-fee">{card.feeLine}</p>
	{/if}
	<div class="said">
		<p class="summary">{card.summary}</p>
		{#if card.points.length > 0}
			<ul>
				{#each card.points as point, n (n)}
					<li>{point}</li>
				{/each}
			</ul>
		{/if}
		{#if card.hosts.length > 0}
			<p class="sources" data-testid="agent-advice-sources">
				{card.sourcesLabel}: {card.hosts.join(', ')}
			</p>
		{/if}
	</div>
	<p class="footer">{card.footer}</p>
</section>

<style>
	.advice {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
		padding: var(--space-lg) var(--space-xl);
		border: var(--border-emphasis) dashed var(--color-border-strong);
		border-radius: var(--radius-xl);
		background: var(--color-bg-sunken);
	}

	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-md);
	}

	h3 {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-muted);
	}

	/* The agent's verdict. Never green: a reassurance is the wallet's to give. */
	.chip {
		flex-shrink: 0;
		padding: var(--space-xs) var(--space-md);
		border-radius: var(--radius-full);
		border: var(--border-hairline) solid var(--color-border-strong);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-medium);
		color: var(--color-fg-base);
	}
	.chip[data-verdict='caution'] {
		background: var(--color-warning-soft);
		border-color: var(--color-warning-border);
		color: var(--color-warning-base);
	}
	.chip[data-verdict='danger'] {
		background: var(--color-error-soft);
		border-color: var(--color-error-base);
		color: var(--color-error-base);
	}
	.chip[data-verdict='unsure'] {
		color: var(--color-fg-muted);
	}

	p,
	ul {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-base);
		overflow-wrap: anywhere;
	}

	.said {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	ul {
		padding-inline-start: var(--space-xl);
	}

	.sources,
	.stale,
	.fee,
	.footer {
		color: var(--color-fg-muted);
	}

	.footer {
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
	}

	/* Given for an earlier version: greyed, never drawn as current. */
	[data-stale] .said,
	[data-stale] .chip {
		opacity: var(--opacity-dim);
	}
</style>
