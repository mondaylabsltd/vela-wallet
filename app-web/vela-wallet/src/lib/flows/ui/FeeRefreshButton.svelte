<script lang="ts">
	/**
	 * The fee's refresh control (spec 068) — the send form's, and since spec 079
	 * the signing sheet's too, so the two fee rows ask again in one way (the
	 * design sheet: Send and signing "must not drift"; the owner on the signing
	 * sheet: "似乎没有刷新网络费的按钮呀").
	 *
	 * It sits in its row's flex line, as that row's last item, and takes the
	 * row's whole height (`align-self: stretch`) so the edge takes a tap; what
	 * is SEEN is a small circle at the vertical centre.
	 */
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';

	interface Props {
		/** The control's accessible name. */
		label: string;
		/** A measurement is out: the glyph turns and a second tap is refused. */
		refreshing?: boolean;
		/** Ask the chain again. Absent ⇒ the drawn row, with no live session. */
		onrefresh?: () => void;
	}

	let { label, refreshing = false, onrefresh }: Props = $props();
</script>

<!-- A tap must never be ambiguous: while the measurement is out the icon
     turns (and the control refuses a second tap), so "did that do
     anything?" has an answer on screen instead of in the network tab. -->
<button
	type="button"
	class="refresh"
	data-focus-inner
	aria-label={label}
	aria-busy={refreshing}
	disabled={onrefresh === undefined || refreshing}
	onclick={onrefresh}
>
	<!-- The turn belongs to the GLYPH, not to the button. The button's
	     padding is deliberately lopsided (none at the start, so the icon
	     sits against the figure it refreshes), which puts its geometric
	     centre away from the icon — rotating the button swung the icon
	     around that off-centre point in an orbit instead of turning it
	     in place. This span shrink-wraps the square icon, so the two
	     centres are the same one. -->
	<span class="turn" class:spinning={refreshing}>
		<Icon icon={UTILITY_ICONS['refresh-cw']} size="sm" />
	</span>
</button>

<style>
	/* Quiet on purpose: a fee that is fine is the normal case, and a loud
	   refresh button next to a good number invites a tap nobody needs.

	   Exactly the card's height (078 round 2): the same top and bottom edges
	   at every text size, so a two-line fee at the largest size does not leave
	   a small target floating in the middle of a tall card. The glyph stays
	   centred inside it. */
	.refresh {
		display: flex;
		align-items: center;
		align-self: stretch;
		flex: 0 0 auto;
		padding: 0 var(--space-sm) 0 0;
		border: none;
		background: none;
		color: var(--color-fg-subtle);
		cursor: pointer;
	}

	.refresh:disabled {
		cursor: default;
	}

	/* What is SEEN of the control (078 round 3): a small round icon button in
	   the card, a fixed circle at the vertical centre whatever the card's
	   height. The button around it stays the card's height so the whole edge
	   takes a tap, but it draws nothing — no box stretches with a two-line
	   card; its hover and its keyboard ring go on the circle. A circle round
	   the glyph, so `spin` still turns it on its own centre. */
	.turn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-control-sm);
		height: var(--size-control-sm);
		border-radius: var(--radius-full);
	}

	@media (hover: hover) {
		.refresh:not(:disabled):hover .turn {
			background: var(--color-bg-sunken);
			color: var(--color-fg-base);
		}
	}

	/* `data-focus-inner` takes the button out of the global ring (app.css);
	   the ring goes on the circle. The transparent outline stays, for forced
	   colours, as the global rule keeps it. */
	.refresh:focus-visible {
		outline: var(--space-xs) solid transparent;
	}

	.refresh:focus-visible .turn {
		box-shadow:
			0 0 0 var(--space-xs) var(--color-fixed-focusRingInner),
			0 0 0 var(--space-sm) var(--color-fixed-focusRingOuter);
	}

	.turn.spinning {
		/* A calm turn, not a frantic one: the token is the longest duration the
		   system defines, taken twice, because a full rotation in 400ms reads
		   as alarm on a screen where somebody is about to pay. */
		animation: spin calc(var(--motion-duration-slow) * 2) linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	/* The turn is feedback, not decoration; somebody who asked for no motion
	   still gets the disabled control and the busy state. */
	@media (prefers-reduced-motion: reduce) {
		.turn.spinning {
			animation: none;
		}
	}
</style>
