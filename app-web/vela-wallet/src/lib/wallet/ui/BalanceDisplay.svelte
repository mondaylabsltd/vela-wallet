<script lang="ts">
	import type { BalanceModel } from '../model';
	import { UTILITY_ICONS } from '../icons';
	import Icon from './Icon.svelte';
	import SkeletonRow from './SkeletonRow.svelte';

	interface Props {
		balance: BalanceModel;
		onstatus?: () => void;
		ontoggle?: () => void;
		/**
		 * Issue 462: the refresh control was pressed. Never called while it
		 * turns — a second press would only start a second round. Absent in the
		 * gallery, where the control is a picture.
		 */
		onrefresh?: () => void;
	}

	let { balance, onstatus, ontoggle, onrefresh }: Props = $props();

	const spinning = $derived(balance.refresh?.spinning === true);

	function refresh(): void {
		if (spinning) return;
		onrefresh?.();
	}

	/**
	 * The figure is ONE line, always — as wide as it is, or scaled to the room.
	 *
	 * It used to wrap wherever it ran out (`overflow-wrap: anywhere`), which
	 * broke a number in the middle of itself — "₫112,500,00 / 0.00" at 320 px —
	 * and made the hero two lines tall, so the whole page dropped a line when a
	 * long figure landed where the one-line skeleton had stood (measured:
	 * 44.8 px, on the first read and again when the display currency
	 * committed). A figure that does not fit its line is drawn smaller, by a
	 * transform: the line box is the slot's and does not change, so nothing
	 * under the hero moves whatever the figure's length.
	 */
	// `bind:this` hands back `null` when its element leaves (the skeleton
	// taking the figure's place), not `undefined`.
	let slot = $state<HTMLElement | null>(null);
	let figure = $state<HTMLElement | null>(null);
	let fit = $state(1);
	$effect(() => {
		const room = slot;
		const drawn = figure;
		if (!room || !drawn) {
			fit = 1;
			return;
		}
		const measure = () => {
			// The figure's own width, whatever the transform on it.
			const need = drawn.scrollWidth;
			const have = room.clientWidth;
			fit = need > have && need > 0 && have > 0 ? have / need : 1;
		};
		measure();
		// The room changes with the window, the figure with its digits and its
		// face (a web font landing is a resize too).
		const watch = new ResizeObserver(measure);
		watch.observe(room);
		watch.observe(drawn);
		return () => watch.disconnect();
	});
</script>

<div class="balance">
	<!-- The currency is named once it is known (`BalanceModel.currency`): the
	     label never says the placeholder's "USD" and then changes its mind. -->
	<p class="label">
		{balance.currency === undefined ? balance.label : `${balance.label} · ${balance.currency}`}
	</p>

	{#if balance.state === 'loading'}
		<!-- The skeleton stands in the figure's own line box: when the figure
		     arrives — the first read landing, or the display currency
		     committing — nothing under it moves. -->
		<div class="amount-slot"><SkeletonRow kind="block" /></div>
	{:else if balance.state === 'hidden'}
		<p class="amount hidden-row">
			<span class="mask">{balance.integer}</span>
			<button type="button" class="toggle" aria-label={balance.a11yShow} onclick={ontoggle}>
				<Icon icon={UTILITY_ICONS['eye-off']} size="lg" />
			</button>
		</p>
	{:else}
		<!-- The figure's line: the same slot the skeleton stands in, so the
		     figure landing — and a longer one replacing it — moves nothing. -->
		<div class="amount-slot" bind:this={slot}>
			{#if ontoggle !== undefined}
				<!-- Live (spec 025): the figure itself is the tap-to-hide target — the
				     H5 design's gesture, with the hidden state's eye-off as its inverse.
				     Absent a handler (the gallery), the amount stays a plain figure. -->
				<button
					type="button"
					class="amount figure amount-toggle"
					class:fitted={fit < 1}
					style:--fit={fit < 1 ? fit : undefined}
					aria-label={balance.a11yHide}
					onclick={ontoggle}
					bind:this={figure}
				>
					<span class="integer">{balance.integer}</span><span class="decimals"
						>{balance.decimalMark ?? '.'}{balance.decimals}</span
					>
				</button>
			{:else}
				<p
					class="amount figure"
					class:fitted={fit < 1}
					style:--fit={fit < 1 ? fit : undefined}
					bind:this={figure}
				>
					<span class="integer">{balance.integer}</span><span class="decimals"
						>{balance.decimalMark ?? '.'}{balance.decimals}</span
					>
				</p>
			{/if}
		</div>
	{/if}

	{#if balance.refresh !== undefined}
		<!-- Issue 462: "↻ Updated 2m", the same control on all four apps. One
		     box for both states, so a press moves nothing: the glyph turns in
		     its own square, and the two labels share one grid cell — the box is
		     as wide as the longer of them, the one not shown is laid out under
		     the other, invisible. Inert while it turns, never dimmed. Its name
		     is what it says — "Updating…" while it turns, "Updated 2m" at rest —
		     and "Refresh balance" while no read has settled and the glyph stands
		     alone: a glyph is aria-hidden and the laid-out-but-invisible label
		     names nothing, so without it a screen reader heard a bare "button"
		     exactly when a person most needs the control. -->
		<button
			type="button"
			class="refresh"
			class:spinning
			data-testid="balance-refresh"
			aria-label={spinning
				? balance.refresh.updating
				: (balance.refresh.updated ?? balance.refresh.a11yIdle)}
			aria-busy={spinning}
			aria-disabled={spinning || onrefresh === undefined ? 'true' : undefined}
			onclick={refresh}
		>
			<span class="glyph"><Icon icon={UTILITY_ICONS['refresh-cw']} size="sm" /></span>
			<span class="words">
				<span class:shown={!spinning}>{balance.refresh.updated ?? ''}</span>
				<span class:shown={spinning}>{balance.refresh.updating}</span>
			</span>
		</button>
	{/if}

	<!--
		The line a status is said on, kept from the first frame (PR 3 note 26b).

		"Can't reach Gnosis right now", "Some tokens couldn't be priced", the
		new wallet's "Live · listening for payments": each arrived as a line of
		its own under the figure, and each arrival pushed the refresh control
		and the whole page under the hero down a line — and its going pulled
		them back up. The line is the hero's now, whether or not anything is
		said on it, and it stands under the refresh control: what a person
		presses never moves, and a status lands in room that was already there.
	-->
	<div class="said">
		{#if balance.status !== undefined}
			<button type="button" class="status {balance.status.kind}" onclick={onstatus}>
				<Icon
					icon={balance.status.kind === 'warning'
						? UTILITY_ICONS['triangle-alert']
						: UTILITY_ICONS['refresh-cw']}
					size="sm"
				/>
				<span>{balance.status.text}</span>
				<Icon icon={UTILITY_ICONS['chevron-right']} size="sm" />
			</button>
		{:else if balance.state === 'zero-live' && balance.liveText !== undefined}
			<p class="live">
				<span class="live-dot" aria-hidden="true"></span>
				{balance.liveText}
			</p>
		{/if}
	</div>
</div>

<style>
	/* The tappable figure wears the paragraph's type exactly; only the
	   affordance (cursor) is added. */
	.amount-toggle {
		background: none;
		border: 0;
		padding: 0;
		margin: 0;
		font: inherit;
		color: inherit;
		text-align: inherit;
		cursor: pointer;
	}

	.balance {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--space-md);
	}

	p {
		margin: 0;
	}

	.label {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-medium);
		color: var(--color-fg-subtle);
		letter-spacing: var(--letterSpacing-sectionLabel);
	}

	.amount {
		font-family: var(--font-display);
		font-size: calc(var(--text-5xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		line-height: var(--leading-amountHero);
		color: var(--color-fg-base);
		overflow-wrap: anywhere;
	}

	/* One line. A figure that fits is drawn as it is set — no transform at
	   all, so nothing about its rendering changes. */
	.figure {
		flex: none;
		white-space: nowrap;
		overflow-wrap: normal;
	}

	/* One that does not is drawn to fit (`--fit`, under 1): scaled from its
	   start, so a long figure ends where the column ends. */
	.figure.fitted {
		transform: scale(var(--fit));
		transform-origin: 0 50%;
	}

	:global([dir='rtl']) .figure.fitted {
		transform-origin: 100% 50%;
	}

	.decimals {
		font-size: calc(var(--text-3xl) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	/* One line of the amount's face, exactly: the bar inside keeps its own
	   size and sits on the line's middle. The bar alone was a third shorter
	   than the figure, so everything under the hero dropped when it landed. */
	.amount-slot {
		display: flex;
		align-items: center;
		width: 100%;
		height: calc(var(--text-5xl) * var(--text-scale, 1) * var(--leading-amountHero));
		/* A figure wider than the slot is LAID OUT that wide and drawn to fit;
		   its layout box must not give the page a sideways scroll. */
		overflow-x: clip;
	}

	/* The status line's room: one line of the status, its own padding
	   included, there before anything is said in it. */
	.said {
		display: flex;
		align-items: flex-start;
		width: 100%;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		/* Said out loud rather than left to the face's own metrics, so the
		   room kept is exactly the room a status takes. */
		line-height: var(--leading-normal);
		min-height: calc(1lh + 2 * var(--space-sm));
	}

	.hidden-row {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
	}

	.mask {
		letter-spacing: var(--space-sm);
	}

	.toggle {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-hitTarget);
		height: var(--size-hitTarget);
		border: none;
		background: none;
		color: var(--color-fg-subtle);
		cursor: pointer;
		border-radius: var(--radius-full);
	}

	.live {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		/* The status line's own block padding: the two stand on one line. */
		padding-block: var(--space-sm);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
	}

	.live-dot {
		width: var(--space-md);
		height: var(--space-md);
		border-radius: var(--radius-full);
		background: var(--color-success-base);
		animation: pulse calc(var(--motion-entrance-fadeUp) * 2) ease-in-out infinite alternate;
	}

	@keyframes pulse {
		from {
			opacity: 1;
		}

		to {
			opacity: var(--opacity-backdrop);
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.live-dot {
			animation: none;
		}
	}

	.status {
		display: inline-flex;
		align-items: center;
		gap: var(--space-md);
		padding: var(--space-sm) 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		/* A line that wraps reads from its start, beside its glyph — a button
		   centres its text by default, and Vela's own-fault sentence (PR 2
		   note 11) is the first status long enough to wrap. */
		text-align: start;
		cursor: pointer;
		border-radius: var(--radius-sm);
	}

	.warning {
		color: var(--color-warning-base);
	}

	.refreshing {
		color: var(--color-fg-muted);
	}

	/* Quiet, like the status line above it: subtle ink, fuller on hover. */
	.refresh {
		display: inline-flex;
		align-items: center;
		gap: var(--space-sm);
		padding: var(--space-sm) 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
		cursor: pointer;
		border-radius: var(--radius-sm);
		transition: color var(--motion-hover) ease-out;
	}

	.refresh:hover:not([aria-disabled='true']) {
		color: var(--color-fg-muted);
	}

	.refresh[aria-disabled='true'] {
		cursor: default;
	}

	.glyph {
		display: inline-flex;
	}

	.spinning .glyph {
		animation: spin calc(var(--motion-duration-slow) * 2) linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	/* Still, the words alone say it is reading. */
	@media (prefers-reduced-motion: reduce) {
		.spinning .glyph {
			animation: none;
		}
	}

	.words {
		display: inline-grid;
		text-align: start;
		white-space: nowrap;
	}

	.words > span {
		grid-area: 1 / 1;
		visibility: hidden;
	}

	.words > .shown {
		visibility: visible;
	}
</style>
