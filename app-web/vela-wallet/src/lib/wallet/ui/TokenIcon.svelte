<script lang="ts">
	import RemoteLogo from './RemoteLogo.svelte';

	interface Props {
		ticker: string;
		/** Chain badge dot color (fixture data, not a theme token). */
		badgeColor: string;
		/**
		 * Logo candidates from the chain-data endpoint, tried in order. The
		 * three-letter glyph is drawn underneath either way: it is what shows
		 * until the first candidate has loaded, and what stays when none does.
		 * Absent in the fixtures, which draw the glyph.
		 */
		logoUrls?: string[];
		/** The badge chain's logo, over the dot the same way. */
		badgeLogoUrl?: string;
		/**
		 * No badge at all: a native coin on its own chain (ETH on Ethereum)
		 * would wear its own logo twice. The phone app's `tokenBadgeChainId`.
		 */
		badgeHidden?: boolean;
		/**
		 * Spec 021: `inline` is the mark inside a line of text — the fee row's
		 * fee token, a fact row's network, a notice banner's chain. A size
		 * PROP and not a CSS wrapper: the glyph has to shrink with the circle,
		 * and scaling the box alone clipped "ETH" out of it.
		 */
		size?: 'row' | 'inline';
	}

	let {
		ticker,
		badgeColor,
		logoUrls,
		badgeLogoUrl,
		badgeHidden = false,
		size = 'row'
	}: Props = $props();

	const glyph = $derived(ticker.slice(0, 3).toUpperCase());
</script>

<span class="token {size}" aria-hidden="true">
	<span class="glyph">{glyph}</span>
	<RemoteLogo urls={logoUrls} />
	{#if !badgeHidden}
		<span class="badge" class:with-logo={badgeLogoUrl !== undefined} style:background={badgeColor}>
			<RemoteLogo urls={badgeLogoUrl === undefined ? undefined : [badgeLogoUrl]} />
		</span>
	{/if}
</span>

<style>
	.token {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: var(--radius-full);
		background: var(--color-bg-sunken);
		border: var(--border-hairline) solid var(--color-border-base);
		flex-shrink: 0;
	}

	.row {
		width: calc(var(--space-2xl) * 2);
		height: calc(var(--space-2xl) * 2);
	}

	.inline {
		width: var(--icon-xl);
		height: var(--icon-xl);
	}

	.glyph {
		font-weight: var(--weight-semibold);
		color: var(--color-fg-muted);
		letter-spacing: var(--letterSpacing-sectionLabel);
	}

	.row .glyph {
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
	}

	/* Two thirds of the row glyph, which is what keeps a three-letter ticker
	   inside a 24px circle instead of spilling over its edges. */
	.inline .glyph {
		font-size: calc(var(--text-xs) * 0.66 * var(--text-scale, 1));
		letter-spacing: 0;
	}

	.badge {
		position: absolute;
		right: 0;
		bottom: 0;
		border-radius: var(--radius-full);
		border: var(--border-emphasis) solid var(--color-bg-base);
	}

	.row .badge {
		width: var(--icon-xs);
		height: var(--icon-xs);
	}

	/* A badge that may carry a logo is a size a logo can be read at; the dot
	   it falls back to simply fills it. */
	.row .badge.with-logo {
		width: var(--icon-base);
		height: var(--icon-base);
	}

	/* The inline mark carries no chain dot: at 24px the dot would be 6px of
	   colour on a glyph that is already crowded, and the row it sits in has
	   already said which chain this is. */
	.inline .badge {
		display: none;
	}
</style>
