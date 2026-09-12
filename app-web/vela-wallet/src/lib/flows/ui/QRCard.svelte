<script lang="ts">
	/**
	 * The receive QR card (spec 021 component 18) — R2, R3, R4 and DR2L.
	 *
	 * Two decisions carried from the SPEC sheet:
	 *
	 * - **344 square, fixed.** The card does not scale with the text. At 1.35×
	 *   the copy around it grows and the panel scrolls; the code stays the size
	 *   it was, because a code that shrinks to make room for its caption is a
	 *   code that stops scanning.
	 * - **Something in the middle.** The network mark on R2, the token on R3,
	 *   the account's own identicon on the share card — R4's centre is an
	 *   anti-forgery mark: a share card someone doctored to swap the address
	 *   would carry an identicon that no longer matches it.
	 */
	import type { Snippet } from 'svelte';
	import { RECEIVE_MODULES, RECEIVE_SEED, qrPattern } from '$lib/wallet/qr-pattern';
	import type { QrCode } from '$lib/wallet/qr';

	interface Props {
		label: string;
		/**
		 * The code to draw (spec 028 T412). ABSENT means the drawn placeholder —
		 * which is what the galleries pass, because their screenshots are canon
		 * and a fixture that encoded a real address would put real addresses in
		 * the gallery. Every LIVE surface passes a real code.
		 */
		code?: QrCode;
		/** Drawn over the centre of the code, on its own white cut-out. */
		centre?: Snippet;
	}

	let { label, code, centre }: Props = $props();

	const N = $derived(code ? code.modules : RECEIVE_MODULES);
	/** The placeholder path, built once from the pattern, when there is no code. */
	const path = $derived(code ? code.path : patternPath(RECEIVE_MODULES, RECEIVE_SEED));

	/** The pattern as one path, so both branches render through the same element
	 *  and the placeholder keeps rendering exactly the pixels it used to. */
	function patternPath(n: number, seed: number): string {
		const cells = qrPattern(n, seed);
		let d = '';
		for (let y = 0; y < n; y++) {
			for (let x = 0; x < n; x++) if (cells[y][x]) d += `M${x} ${y}h1v1h-1z`;
		}
		return d;
	}
</script>

<div class="card">
	<svg viewBox="0 0 {N} {N}" role="img" aria-label={label} shape-rendering="crispEdges">
		<!-- One path, not a rect per module: per-cell rendering leaves hairline
		     white gridlines from pixel rounding, and a code with gridlines
		     photographs badly — which is how most people read one. -->
		<path d={path} />
	</svg>
	{#if centre}
		<span class="centre">{@render centre()}</span>
	{/if}
</div>

<style>
	.card {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-qrCard);
		height: var(--size-qrCard);
		/* Never scales with --text-scale, and never grows past the screen. */
		max-width: 100%;
		aspect-ratio: 1;
		padding: var(--space-2xl);
		/* White in BOTH appearances: a code is read by a camera, and inverting
		   it in dark mode is the classic way to make one unscannable. */
		background: var(--color-onAccent);
		border-radius: var(--radius-xl);
	}

	svg {
		width: 100%;
		height: 100%;
		fill: var(--color-fixed-shadowInk);
	}

	.centre {
		position: absolute;
		display: flex;
		align-items: center;
		justify-content: center;
		/* The cut-out reads as part of the card, so it takes the card's white
		   rather than a theme surface that would flip underneath it. */
		background: var(--color-onAccent);
		border-radius: var(--radius-full);
		padding: var(--space-xs);
	}
</style>
