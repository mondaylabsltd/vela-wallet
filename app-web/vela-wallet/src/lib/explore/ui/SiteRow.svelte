<script lang="ts">
	import LetterAvatar from '$lib/ui/LetterAvatar.svelte';
	import type { SiteModel } from '../model';

	interface Props {
		site: SiteModel;
		/**
		 * The second line is a host to be judged by — a resume row's (spec 099
		 * navigation) — so when it must be cut it loses its START, as the
		 * browsing bar's pill does: the end of a host is the registrable
		 * domain, the part that decides who you are talking to.
		 * `app.uniswap.org.evil.xyz` must never read as `app.uniswap.or…`.
		 */
		hostLine?: boolean;
		onopen?: (id: string) => void;
	}

	let { site, hostLine = false, onopen }: Props = $props();
</script>

<button type="button" class="row" onclick={() => onopen?.(site.id)}>
	<LetterAvatar letter={site.letter} tint={site.tint} size={40} />
	<span class="text">
		<span class="name">{site.name}</span>
		{#if hostLine}
			<span class="sub host"><bdi>{site.subtitle ?? site.host}</bdi></span>
		{:else}
			<span class="sub">{site.subtitle ?? site.host}</span>
		{/if}
	</span>
	{#if site.meta}
		<span class="meta">{site.meta}</span>
	{/if}
</button>

<style>
	.row {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		width: 100%;
		padding-block: var(--space-lg);
		padding-inline: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		text-align: start;
		cursor: pointer;
	}

	.row:active {
		transform: scale(var(--motion-press-row));
	}

	.text {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		flex: 1;
		min-width: 0;
	}

	.name {
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.sub {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	/* Cut from the START (the pill's rule, AddressBar.svelte). The line runs
	   right to left so the ellipsis lands on its left edge, and is aligned to
	   its end — the row's start — so a host that fits sits where any second
	   line does. The host inside is isolated left to right, so a port or a
	   digit never reorders. */
	.sub.host {
		direction: rtl;
		text-align: end;
	}

	.sub.host bdi {
		direction: ltr;
		unicode-bidi: isolate;
	}

	.meta {
		flex-shrink: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-accent-base);
	}
</style>
