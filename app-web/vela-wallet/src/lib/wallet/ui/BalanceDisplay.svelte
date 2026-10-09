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
</script>

<div class="balance">
	<p class="label">{balance.label} · {balance.currency}</p>

	{#if balance.state === 'loading'}
		<SkeletonRow kind="block" />
	{:else if balance.state === 'hidden'}
		<p class="amount hidden-row">
			<span class="mask">{balance.integer}</span>
			<button type="button" class="toggle" aria-label={balance.a11yShow} onclick={ontoggle}>
				<Icon icon={UTILITY_ICONS['eye-off']} size="lg" />
			</button>
		</p>
	{:else if ontoggle !== undefined}
		<!-- Live (spec 025): the figure itself is the tap-to-hide target — the
		     H5 design's gesture, with the hidden state's eye-off as its inverse.
		     Absent a handler (the gallery), the amount stays a plain figure. -->
		<button
			type="button"
			class="amount amount-toggle"
			aria-label={balance.a11yHide}
			onclick={ontoggle}
		>
			<span class="integer">{balance.integer}</span><span class="decimals"
				>{balance.decimalMark ?? '.'}{balance.decimals}</span
			>
		</button>
	{:else}
		<p class="amount">
			<span class="integer">{balance.integer}</span><span class="decimals"
				>{balance.decimalMark ?? '.'}{balance.decimals}</span
			>
		</p>
	{/if}

	{#if balance.state === 'zero-live' && balance.liveText !== undefined}
		<p class="live">
			<span class="live-dot" aria-hidden="true"></span>
			{balance.liveText}
		</p>
	{/if}

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

	.decimals {
		font-size: calc(var(--text-3xl) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
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
		font-size: calc(var(--text-base) * var(--text-scale, 1));
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
