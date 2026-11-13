<script lang="ts">
	/**
	 * The onboarding rail — the left column of every onboarding screen, at and
	 * above the desktop breakpoint.
	 *
	 * It exists because the single-column layout stretched to the viewport
	 * (`flex: 1` + `space-between`), so its empty middle grew with every pixel
	 * of window height. That is the right shape on a phone, where the screen is
	 * the height of a hand; on a desktop window it opened a hole that read as a
	 * phone page pulled tall. A rail spends the spare WIDTH on orientation
	 * instead, and content beside it can end where it ends.
	 *
	 * Two things live here. The brand, which never moves, and one slot: the
	 * step's ordinal, name and what it decides while the journey runs, the
	 * product's own line before it starts and after it ends. So the sequence
	 * reads brand → 01 → 02 → 03 → brand.
	 *
	 * The ordinal is set as TYPE, not drawn as a stepper. A vertical stepper
	 * here reads as a control bolted to the side of the page; a mono numeral at
	 * display size carries the same fact and is part of it.
	 *
	 * The desktop app renders the same rail from `ui/rail.rs`.
	 */
	import BrandMark from '$lib/ui/BrandMark.svelte';
	import type { RailSlot } from './rail';

	// NOT `slot`: that is a reserved attribute name in Svelte, and a prop
	// called it would collide with the legacy slot API at every call site.
	let { rail }: { rail: RailSlot } = $props();

	/** `01`, not `1` — the pair reads as a measure rather than as a count. */
	const pad = (n: number) => String(n).padStart(2, '0');
</script>

<aside class="rail">
	<header class="brand">
		<BrandMark size={60} />
		<span class="wordmark">VELA WALLET</span>
	</header>

	{#if rail.kind === 'tagline'}
		<div class="slot">
			<p class="tagline">{rail.text}</p>
			<span class="rule" aria-hidden="true"></span>
		</div>
	{:else}
		<div class="slot">
			<p class="ordinal">
				<span class="figure">{pad(rail.ordinal)}</span><span class="total">/{pad(rail.total)}</span>
			</p>
			<p class="name">{rail.name}</p>
			<p class="detail">{rail.detail}</p>
		</div>
	{/if}
</aside>

<style>
	.rail {
		display: none;
		flex: 0 0 var(--layout-onboardingRail);
		flex-direction: column;
		width: var(--layout-onboardingRail);
		padding: var(--space-5xl) var(--space-4xl);
		border-inline-end: var(--border-hairline) solid var(--color-border-base);
		background: var(--color-bg-sunken);
	}

	/* The rail is an answer to spare width. Below the breakpoint there is
	   none, and the page keeps the single column it already had. */
	@media (min-width: 1280px) {
		.rail {
			display: flex;
		}
	}

	.brand {
		display: flex;
		gap: var(--space-lg);
		align-items: center;
	}

	.wordmark {
		color: var(--color-fg-base);
		font-size: calc(var(--text-xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		letter-spacing: 0.11em;
	}

	/* Centred in whatever the brand leaves — the rail's composition does not
	   move between screens, only its content. */
	.slot {
		margin-block: auto;
	}

	.tagline {
		margin: 0;
		color: var(--color-fg-base);
		font-size: var(--text-railTagline);
		font-weight: var(--weight-bold);
		line-height: var(--leading-tight);
		letter-spacing: -0.015em;
		/* vis-21: near-even lines, and CJK broken where a reader would — ja
		   read "あなたの鍵、あなた / の資産", a word cut in two. */
		text-wrap: balance;
	}

	:global(html[lang='ja']) .tagline {
		word-break: auto-phrase;
	}

	:global(html[lang='ko']) .tagline {
		word-break: keep-all;
	}

	.rule {
		display: block;
		width: var(--layout-railRuleW);
		height: var(--space-xs);
		margin-block-start: var(--space-3xl);
		border-radius: var(--radius-full);
		background: var(--color-fg-base);
	}

	.ordinal {
		display: flex;
		gap: var(--space-railOrdinalGap);
		align-items: baseline;
		margin: 0;
		font-family: var(--font-mono);
		font-weight: var(--weight-medium);
	}

	.figure {
		color: var(--color-rail-ordinal);
		font-size: var(--text-stepOrdinal);
		line-height: 0.82;
		letter-spacing: -0.04em;
	}

	.total {
		color: var(--color-rail-ordinalSoft);
		font-size: var(--text-stepTotal);
	}

	.name {
		margin: var(--space-railNameGap) 0 0;
		color: var(--color-fg-base);
		font-size: var(--text-stepTotal);
		font-weight: var(--weight-bold);
	}

	.detail {
		max-width: var(--layout-railDetailMeasure);
		margin: var(--space-md) 0 0;
		color: var(--color-fg-muted);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-relaxed);
	}
</style>
