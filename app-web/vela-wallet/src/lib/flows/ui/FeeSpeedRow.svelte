<script lang="ts">
	/**
	 * The speed control under the fee row (spec 068).
	 *
	 * **Folded by default, always.** Most sends do not need a decision about
	 * speed, and a picker sitting open on the send form would ask everybody to
	 * make one. Folded it is a single quiet line — the word "Speed" and the
	 * tier THIS send is running at — and what it shows is the person's own
	 * stored default, never a hardcoded one, so this line and the Settings row
	 * can never say different things.
	 *
	 * Opened, each option carries its OWN fee. That is the whole point: the
	 * trade between cost and speed has to be visible at the moment of choosing,
	 * not one tap later. Each figure is a real quote of the real transaction at
	 * that tier — nothing here scales one number into three.
	 *
	 * The options are NAMED by speed — 超快 / 标准 / 较慢 — never by a number
	 * and never by what they cost: the control's own label is "Speed", so an
	 * option called "Economy" would be answering a question nobody asked. What
	 * each speed buys is the line under it, which is what stops the slow tier
	 * reading as a defect. It is line 2, under the name; the gas bid shares
	 * that line, under the fee (078 round 2 — two lines, not three with a hole
	 * under every name). When the two cannot share it, the reason wraps under
	 * itself and the bid, a figure, is never cut.
	 *
	 * Under each fee, quietly, is the effective gas price
	 * that tier buys (issue 684). That is a number, and it is deliberately NOT
	 * the name: `fee_policy` clamps every tier to the $0.01 floor on any chain
	 * whose real cost is under a cent, so on seven networks the owner found
	 * three identical fees and read the picker as broken. The tiers do differ —
	 * each signs a different tip — and this is the row where that shows. It
	 * stays subordinate: caption-sized, muted, under the money — and it carries
	 * its name ("Gas Bid"), because an unnamed number under a fee reads as a
	 * second charge.
	 *
	 * Since issue 685 that figure is a range, `0.02011 ~ 0.03016 gwei`: what
	 * the speed bids now, and its cap — how high it will go if the base fee
	 * spikes. The bid alone moved only by the tip, so on a chain where the tip
	 * is a sliver of the base fee three fees 1.8× apart sat over gas prices
	 * 0.5% apart. Which is why the label says BID ("Gas Bid" / "Gas 出价"),
	 * where it said "Gas Price" / "Gas 价格" under issue 684's single figure: a
	 * price quoted as `270 ~ 805` reads as "what I pay may climb to 805", and
	 * it never can — the person's fee is the figure above, fixed when they
	 * sign, and the relay pays the chain. A bid that reaches up to 805 is the
	 * truth about both ends, and on issue 684's one figure too.
	 */
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import type { FeeSpeedModel } from '../model';

	interface Props {
		speed: FeeSpeedModel;
		/** Fold / unfold. Absent ⇒ the drawn control, with nothing behind it. */
		ontoggle?: () => void;
		/** A tier id — one-shot, for this send only. */
		onselect?: (id: string) => void;
	}

	let { speed, ontoggle, onselect }: Props = $props();

	/**
	 * Each option's last drawn gas-bid width, by tier. While a tier
	 * re-measures its bid is blank, and a blank slot is narrower than a figure
	 * — the description beside it would take the room and re-wrap, and the
	 * option would change height twice: the picker jumping on every tap and
	 * refresh. Held at the width it last had, line 2 keeps its shape until the
	 * new figure lands. (The first figure a mounted control ever draws has no
	 * width to hold; the label's width is reserved for it instead.)
	 */
	let gasWidths = $state<Record<string, number>>({});
</script>

<section class="speed">
	<button type="button" class="summary" aria-expanded={speed.open} onclick={ontoggle}>
		<span class="label">{speed.label}</span>
		<span class="current">{speed.value}</span>
		<span class="chevron" class:up={speed.open}>
			<Icon icon={UTILITY_ICONS['chevron-down']} size="sm" />
		</span>
	</button>

	{#if speed.freeNote !== undefined}
		<!-- Folded AND open (issue 686): the summary above says "Fast" while
		     Settings says the person's own slower default, and a screen that
		     says one thing while the preference says another must say why —
		     right under the word it explains, without anybody having to open
		     anything to find out. -->
		<p class="free">{speed.freeNote}</p>
	{/if}

	{#if speed.open && speed.singleNote !== undefined}
		<!-- Nothing to choose (issue 686): every speed settled at the same fee
		     with no gas price to tell them apart, so three options would offer a
		     choice that does nothing. One statement instead, where the options
		     were — and no "for this transaction only", since nothing here can be
		     changed for this transaction or any other. -->
		<p class="single">{speed.singleNote}</p>
	{:else if speed.open}
		<!-- Said before the options, not after: a person about to change the
		     speed of one payment needs to know, BEFORE they tap, that they are
		     not changing what every later payment does. A preference that
		     drifts silently is one nobody can trust. -->
		<p class="once">{speed.onceNote}</p>
		<ul>
			{#each speed.options as option (option.id)}
				<li>
					<button
						type="button"
						class="option"
						class:selected={option.selected}
						aria-pressed={option.selected}
						onclick={() => onselect?.(option.id)}
					>
						<span class="name">{option.label}</span>
						<span class="values">
							<span class="value">{option.value}</span>
							{#if option.valueFiat}
								<span class="value">{option.valueFiat}</span>
							{/if}
						</span>
						<span class="tick" aria-hidden="true">
							{#if option.selected}
								<Icon icon={UTILITY_ICONS.check} size="sm" />
							{/if}
						</span>
						<!-- Line 2 (078 round 2): what the speed buys on the left, the gas
						     bid on the right — under the name and under the fee, where
						     each belongs. It used to be two lines of its own, the bid
						     right-aligned over an empty left half and the description
						     under that: a hole under every name. -->
						{#if option.detail !== undefined || speed.gasPriceLine}
							<span class="sub">
								<span class="detail">{option.detail ?? ''}</span>
								{#if speed.gasPriceLine}
									<!-- Named, and drawn: a bare "3,244 wei" reads as a second
									     amount being charged. Held open EMPTY while the set is
									     measuring, so the option keeps its shape. -->
									<span
										class="gas"
										bind:offsetWidth={gasWidths[option.id]}
										style:min-width={option.gasPrice === undefined &&
										gasWidths[option.id] !== undefined
											? `${gasWidths[option.id]}px`
											: undefined}
									>
										{#if option.gasPrice !== undefined}
											<span class="gas-label">{speed.gasPriceLabel}</span>
											<span class="gas-value">{option.gasPrice}</span>
										{:else}
											<!-- Saying nothing, but holding the room: the label's
											     width, unseen, and a line exactly as tall as a
											     figure's. -->
											<span class="gas-reserve" aria-hidden="true">{speed.gasPriceLabel}</span>
											<span class="gas-value">&nbsp;</span>
										{/if}
									</span>
								{/if}
							</span>
						{/if}
					</button>
				</li>
			{/each}
		</ul>
	{/if}
</section>

<style>
	.speed {
		display: flex;
		flex-direction: column;
	}

	.summary {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		width: 100%;
		padding: var(--space-sm) var(--space-lg);
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
		text-align: start;
		cursor: pointer;
	}

	.label {
		flex: 0 0 auto;
	}

	/* The tier in force, carrying the weight: it is the answer somebody is
	   scanning for, and the word "Speed" beside it is only the question. */
	.current {
		flex: 1 1 auto;
		min-width: 0;
		overflow: hidden;
		white-space: nowrap;
		text-overflow: ellipsis;
		text-align: end;
		color: var(--color-fg-base);
	}

	.chevron {
		display: flex;
		flex: 0 0 auto;
		transition: transform var(--motion-duration-fast) ease-out;
	}

	.chevron.up {
		transform: rotate(180deg);
	}

	.once,
	.free {
		margin: 0;
		padding: 0 var(--space-lg) var(--space-xs);
		font-family: var(--font-ui);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	/* Sits where the options were, on the same raised card, so opening the
	   control on a one-speed network still lands on something that looks like
	   an answer rather than a picker that failed to draw. */
	.single {
		margin: 0;
		padding: var(--space-md) var(--space-lg);
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
	}

	ul {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
		overflow: hidden;
	}

	/* Two lines (078 round 2): [name …… fee] ✓ over [description …… gas bid].
	   A grid, so each line-2 piece sits under the line-1 piece it belongs to
	   — the reason under the name, the bid under the money — and the tick
	   keeps a column of its own, top-aligned with line 1. */
	.option {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto var(--space-lg);
		grid-template-areas:
			'name values tick'
			'sub sub .';
		align-items: baseline;
		column-gap: var(--space-md);
		row-gap: var(--space-xs);
		width: 100%;
		padding: var(--space-md) var(--space-lg);
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		text-align: start;
		cursor: pointer;
	}

	/* The speed in force is TEXT colour and weight, never accent — accent is
	   for moving money and submitting (design language), and a speed is
	   neither. The tick below carries the same colour. */
	.option.selected .name {
		color: var(--color-fg-base);
		font-weight: var(--weight-semibold);
	}

	/* The money never wraps and the name, a short word, elides instead: three
	   rows being COMPARED must be one height each, their figures ending on one
	   right edge ("Стандартно" once took two lines on the narrowest phone
	   while its neighbours took one). */
	.name {
		grid-area: name;
		min-width: 0;
		overflow: hidden;
		white-space: nowrap;
		text-overflow: ellipsis;
	}

	.values {
		grid-area: values;
		display: flex;
		flex-wrap: nowrap;
		justify-content: flex-end;
		column-gap: var(--space-sm);
	}

	.value {
		white-space: nowrap;
		font-family: var(--font-numeric);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-variant-numeric: tabular-nums;
		color: var(--color-fg-base);
	}

	/* Reserved whether or not the tick is drawn, so choosing a row does not
	   shuffle the figures beside it. Centred on line 1, whatever line 2 does. */
	.tick {
		grid-area: tick;
		align-self: center;
		display: flex;
		color: var(--color-fg-base);
	}

	/* The reason and the bid, both caption-sized and quiet: the fee and the
	   name are what is being compared. The bid never gives way — it is a
	   figure — so when the two cannot share the line, the reason wraps under
	   itself, in its own column; it is never cut, since a truncated reason is
	   worse than none.

	   One floor under that: the reason keeps a column of at least six of its
	   own characters. Only the widest range the formatter can write, under the
	   longest label, at the narrowest phone, leaves less — and a column one
	   word wide stacks the sentence into a tower. There, and only there, the
	   bid drops under the reason, still whole and on the fee's right edge. */
	.sub {
		grid-area: sub;
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		column-gap: var(--space-md);
		row-gap: var(--space-xs);
		min-width: 0;
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-subtle);
	}

	.detail {
		flex: 1 1 6em;
		min-width: 0;
	}

	/* The UI face with tabular DIGITS for the figure — not a monospace face for
	   the whole string: "Gas Bid" is a word, and only the number has columns. */
	.gas {
		flex: 0 0 auto;
		display: flex;
		column-gap: var(--space-xs);
		margin-inline-start: auto;
		white-space: nowrap;
		font-family: var(--font-ui);
	}

	.gas-value {
		font-family: var(--font-numeric);
		font-variant-numeric: tabular-nums;
	}

	.gas-reserve {
		visibility: hidden;
	}

	@media (prefers-reduced-motion: reduce) {
		.chevron {
			transition: none;
		}
	}
</style>
