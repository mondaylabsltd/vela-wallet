<script lang="ts">
	/**
	 * R4 — what "Save image" produces (spec 021; recomposed 2026-09-27 after
	 * the WeChat Pay collection card the founder holds it against).
	 *
	 * Not a screen. It is a render product that ends up in someone's photo
	 * library and then in a chat, so its geometry is fixed, its colours are
	 * mode-invariant, and it carries the app icon and the wordmark: away from
	 * the app, the card has to say what it is on its own.
	 *
	 * Drawn from the very document `share-image.ts` composes for the saved
	 * PNG — this component used to be a second hand-kept copy of the card,
	 * and the two drifted. Here it is inline, so it takes the page's loaded
	 * faces and may point at the network's logo by URL; the saved picture
	 * embeds both.
	 */
	import { buildQrPath } from '$lib/wallet/qr';
	import { RECEIVE_SEED, qrPattern } from '$lib/wallet/qr-pattern';
	import { CARD_PALETTE, composeShareSvg } from '../share-image';
	import type { ShareCardModel } from '../model';

	interface Props {
		model: ShareCardModel;
	}

	let { model }: Props = $props();

	/** The gallery's fixtures carry no real code: the drawn demo pattern, at the card's 37. */
	const PLACEHOLDER_MODULES = 37;

	const svg = $derived.by(() => {
		const code = model.code ?? {
			modules: PLACEHOLDER_MODULES,
			path: buildQrPath(
				qrPattern(PLACEHOLDER_MODULES, RECEIVE_SEED)
					.flat()
					.map((dark) => (dark ? 1 : 0)),
				PLACEHOLDER_MODULES
			)
		};
		return composeShareSvg(
			{ ...model, code },
			CARD_PALETTE,
			'',
			model.networkMark.logoUrls?.[0] ?? null
		);
	});
</script>

<!-- Every string in the document is escaped by the composer; the identicon
     is the core's own artwork. -->
<div class="card" role="img" aria-label={model.headline}>
	<!-- eslint-disable-next-line svelte/no-at-html-tags -->
	{@html svg}
</div>

<style>
	/* The card's own 480, scaled down whole when the stage around it is
	   narrower (`.card-stage` is an inline-size container) — never squeezed
	   by a flex parent, which an inline SVG at 100% would otherwise let
	   happen, having no min-content width of its own. */
	.card {
		display: flex;
		flex: none;
		width: min(var(--layout-shareCardW), 100cqi);
	}

	.card :global(svg) {
		display: block;
		width: 100%;
		height: auto;
	}
</style>
