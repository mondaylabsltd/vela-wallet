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
	 * reading as a defect. It sits under BOTH the name and the fee rather than
	 * squeezed beside them: on the narrowest phone the figures take most of
	 * the row, and a sentence folded into what is left would run three or four
	 * lines per option — three ragged options is not a comparison.
	 *
	 * Beside each name — quietly, under the fee — is the effective gas price
	 * that tier buys (issue 684). That is a number, and it is deliberately NOT
	 * the name: `fee_policy` clamps every tier to the $0.01 floor on any chain
	 * whose real cost is under a cent, so on seven networks the owner found
	 * three identical fees and read the picker as broken. The tiers do differ —
	 * each signs a different tip — and this is the row where that shows. It
	 * stays subordinate: smaller, muted, under the money, on its own line so
	 * it never squeezes the fee or reflows the description — and it carries
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
							<span class="amounts">
								<span class="value">{option.value}</span>
								{#if option.valueFiat}
									<span class="value">{option.valueFiat}</span>
								{/if}
							</span>
						</span>
						<span class="tick" aria-hidden="true">
							{#if option.selected}
								<Icon icon={UTILITY_ICONS.check} size="sm" />
							{/if}
						</span>
						{#if speed.gasPriceLine}
							<!-- Named, and drawn: under the fee in the same numeric
							     face, a bare "3,244 wei" reads as a second amount being
							     charged. Held open EMPTY while the set is measuring, so
							     the option does not lose a line and regain it. -->
							<span class="gas">
								{#if option.gasPrice !== undefined}
									<span class="gas-label">{speed.gasPriceLabel}</span>
									<span class="gas-value">{option.gasPrice}</span>
								{:else}
									<!-- In the figure's own face, so the empty line is
									     exactly as tall as a full one. -->
									<span class="gas-value">&nbsp;</span>
								{/if}
							</span>
						{/if}
						{#if option.detail !== undefined}
							<span class="detail">{option.detail}</span>
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

	/* Wraps for ONE reason: the description below. `column-gap` and `row-gap`
	   are set apart rather than shorthand `gap`, because the space between a
	   name and the fee beside it and the space between a name and its own
	   description are not the same measurement. */
	.option {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
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

	.option.selected .name {
		color: var(--color-fg-base);
		font-weight: var(--weight-semibold);
	}

	.name {
		flex: 1 1 auto;
		min-width: 0;
		overflow: hidden;
		white-space: nowrap;
		text-overflow: ellipsis;
	}

	/* The full width of the option, under the name AND the fee, so it still
	   reads on the narrowest phone instead of being folded into whatever the
	   figures left over. It wraps rather than elides — a truncated reason is
	   worse than no reason — and stays quiet: the fee and the name are what is
	   being compared. */
	.detail {
		flex-basis: 100%;
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-subtle);
	}

	/* The OPPOSITE give-way order to the fee row's, on purpose. That row shows
	   one fee and its label is the only thing naming it, so the money drops to
	   a second line and the label keeps its own. Here there are three rows being
	   COMPARED, and a row that wraps while its neighbours do not makes the list
	   ragged exactly where somebody is reading down it — which is what happened
	   on the narrowest phone in a long-word locale ("Стандартно" took two lines,
	   the other two one). So the money never wraps and the name, a short word,
	   elides instead: three rows of one height, and the figures ending on one
	   right edge.

	   The money ONLY, again, since issue 685 — the gas price moved out to a
	   line of its own below (`.gas`). Inside this column its width became
	   this column's width, and a range is wider than the money: in the 272
	   CSS pixels an option really gets on the narrowest phone, "Стандартно" plus
	   `0.02013 ~ 0.04023 gwei` no longer fit on one line, and because the
	   option wraps its items before it shrinks any of them, the money dropped
	   under the name on that row alone — the ragged list this rule exists to
	   prevent. */
	.values {
		flex: 0 0 auto;
		display: flex;
		flex-direction: column;
		align-items: flex-end;
	}

	.amounts {
		display: flex;
		flex-wrap: nowrap;
		justify-content: flex-end;
		column-gap: var(--space-sm);
	}

	/* Under the money, not beside it. On the narrowest phone the figures
	   already fill the right half of the row, and a fourth thing on that line
	   would either wrap the money (the exact raggedness the rule above exists
	   to prevent) or squeeze the name. Its own line costs one short row of
	   height and nothing else — in particular the description below is
	   untouched.

	   The FULL width of the option since issue 685, not the money's column:
	   a range may reach under the name, so it never takes the name's room on
	   the line above. It still ends on the money's right edge — the inset is
	   the tick's width and the gap before it — so it reads as the fee's own
	   footnote rather than the tick's. */
	.gas {
		flex-basis: 100%;
		display: flex;
		justify-content: flex-end;
		column-gap: var(--space-xs);
		padding-inline-end: calc(var(--space-lg) + var(--space-md));
		box-sizing: border-box;
		white-space: nowrap;
		font-family: var(--font-ui);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.gas-value {
		font-family: var(--font-numeric);
		font-variant-numeric: tabular-nums;
	}

	.value {
		white-space: nowrap;
		font-family: var(--font-numeric);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-variant-numeric: tabular-nums;
		color: var(--color-fg-base);
	}

	/* Reserved whether or not the tick is drawn, so choosing a row does not
	   shuffle the figures beside it. */
	.tick {
		display: flex;
		flex: 0 0 auto;
		width: var(--space-lg);
		color: var(--color-fg-base);
	}

	@media (prefers-reduced-motion: reduce) {
		.chevron {
			transition: none;
		}
	}
</style>
