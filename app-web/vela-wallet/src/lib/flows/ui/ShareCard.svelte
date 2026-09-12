<script lang="ts">
	/**
	 * R4 — what "Save image" produces (spec 021; redrawn spec 028 Phase 10 to
	 * the founder's reference).
	 *
	 * Not a screen. It is a 480×700 render product that ends up in someone's
	 * photo library and then in a chat, so its geometry is fixed, its colours
	 * are mode-invariant, and it carries the wordmark: away from the app, the
	 * card has to say what it is on its own. The foot is white with one curve
	 * across its top, and the mark on it is the APP ICON — the same icon that
	 * sits beside the picture on a phone — never the in-app sailboat.
	 *
	 * The identicon sits in the middle of the code (founder direction): a card
	 * whose address was doctored would carry artwork that no longer matches the
	 * characters printed under it.
	 *
	 * `share-image.ts` composes the very same card as an SVG string for the
	 * saved PNG; the two are kept in step by hand, and the gallery shows this one.
	 */
	import Identicon from '$lib/wallet/ui/Identicon.svelte';
	import TokenIcon from '$lib/wallet/ui/TokenIcon.svelte';
	import AppIcon from '$lib/ui/AppIcon.svelte';
	import type { ShareCardModel } from '../model';
	import QRCard from './QRCard.svelte';

	interface Props {
		model: ShareCardModel;
	}

	let { model }: Props = $props();
</script>

<div class="card">
	<p class="headline">{model.headline}</p>

	<div class="sheet">
		<QRCard label={model.headline} code={model.code}>
			{#snippet centre()}
				<Identicon svg={model.identiconSvg} size="row" />
			{/snippet}
		</QRCard>

		<p class="name">{model.name}</p>
		<p class="address">
			{#each model.lines as line, i (i)}<span>{line}</span>{/each}
		</p>
		<p class="note">
			<TokenIcon
				size="inline"
				ticker={model.networkMark.ticker}
				badgeColor={model.networkMark.badgeColor}
				logoUrls={model.networkMark.logoUrls}
				badgeHidden
			/>
			{model.networkNote}
		</p>
	</div>

	<div class="foot">
		<div class="brand">
			<AppIcon size={44} />
			<span class="wordmark">{model.wordmark}</span>
		</div>
	</div>
</div>

<style>
	/*
	 * Every colour on this card is fixed rather than themed. The image is
	 * saved once and viewed anywhere — a card that rendered in dark mode and
	 * was then opened on a white chat background would be a different card.
	 */
	.card {
		display: flex;
		flex-direction: column;
		align-items: center;
		width: var(--layout-shareCardW);
		height: var(--layout-shareCardH);
		background: var(--color-accent-base);
		color: var(--color-onAccent);
		overflow: hidden;
	}

	.headline {
		margin: 0;
		padding-block: var(--space-3xl) var(--space-2xl);
		font-family: var(--font-display);
		font-size: var(--text-3xl);
		font-weight: var(--weight-bold);
		text-align: center;
	}

	.sheet {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-md);
		width: calc(100% - var(--space-2xl) * 2);
		padding: var(--space-2xl);
		border-radius: var(--radius-2xl);
		background: var(--color-onAccent);
		color: var(--color-fixed-shadowInk);
	}

	.name {
		margin: 0;
		padding-top: var(--space-md);
		font-size: var(--text-lg);
		font-weight: var(--weight-bold);
	}

	.address {
		display: flex;
		flex-direction: column;
		align-items: center;
		margin: 0;
		font-family: var(--font-mono);
		font-size: var(--text-sm);
		line-height: var(--leading-normal);
		opacity: var(--opacity-dim);
	}

	.note {
		display: inline-flex;
		align-items: center;
		gap: var(--space-sm);
		margin: 0;
		margin-top: var(--space-sm);
		padding: var(--space-xs) var(--space-lg) var(--space-xs) var(--space-xs);
		border-radius: var(--radius-full);
		border: var(--border-hairline) solid var(--color-border-base);
		font-size: var(--text-sm);
	}

	/* The foot is the white end of the card's own palette; its top edge is
	   one arc across the whole width (half the width each side, a fixed
	   rise), which is the reference's curve rather than two rounded corners. */
	.foot {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 100%;
		margin-top: auto;
		padding-block: var(--space-4xl) var(--space-2xl);
		border-radius: 50% 50% 0 0 / var(--space-4xl) var(--space-4xl) 0 0;
		background: var(--color-onAccent);
		color: var(--color-fixed-shadowInk);
	}

	.brand {
		display: flex;
		align-items: center;
		gap: var(--space-md);
	}

	.wordmark {
		font-family: var(--font-display);
		font-size: var(--text-3xl);
		font-weight: var(--weight-bold);
	}
</style>
