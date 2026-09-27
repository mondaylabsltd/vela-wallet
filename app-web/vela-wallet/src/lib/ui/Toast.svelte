<script lang="ts">
	/**
	 * A toast: one outcome in a line, over whatever page is up, with at most one
	 * thing to do about it and a way to put it away (078 — the bug report's
	 * ending when its sheet was closed mid-send).
	 *
	 * The app had no toast that could carry an action — the contacts page's
	 * "Copied" is a line of text with nothing to press — so this is the first,
	 * drawn in the language the rest of the app speaks: the raised surface and
	 * hairline of a dialog, the shadow a card over the page wears, the tone's
	 * glyph in the tone's colour and the words in the body colour (amber or
	 * green text on a surface fails contrast long before a glyph does). The
	 * action is a text link in the link colour — never the accent, which is for
	 * the one action that moves value.
	 *
	 * It never takes focus: it is said (the host's live region), not
	 * interrupting. The pointer or focus inside it holds the host's timer, and
	 * Escape inside it puts it away.
	 */
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';

	interface Props {
		tone: 'success' | 'warning';
		title: string;
		/** A page outside the app (a new tab, no opener); pressing it also puts the toast away. */
		action?: { label: string; href: string };
		closeLabel: string;
		onclose: () => void;
		/** The pointer or keyboard focus is in the toast — the host holds its timer. */
		onhold?: (held: boolean) => void;
	}

	let { tone, title, action, closeLabel, onclose, onhold }: Props = $props();

	let node = $state<HTMLElement>();

	// Listened for natively on the group: holding the timer and Escape are
	// about the toast as a whole, not a control of their own.
	$effect(() => {
		const el = node;
		if (el === undefined) return;
		let hovered = false;
		let focused = false;
		const hold = () => onhold?.(hovered || focused);
		const enter = () => ((hovered = true), hold());
		const leave = () => ((hovered = false), hold());
		const focusIn = () => ((focused = true), hold());
		const focusOut = (event: FocusEvent) => {
			focused = el.contains(event.relatedTarget as Node | null);
			hold();
		};
		const key = (event: KeyboardEvent) => {
			if (event.key !== 'Escape' || event.isComposing) return;
			event.stopPropagation();
			onclose();
		};
		el.addEventListener('pointerenter', enter);
		el.addEventListener('pointerleave', leave);
		el.addEventListener('focusin', focusIn);
		el.addEventListener('focusout', focusOut);
		el.addEventListener('keydown', key);
		return () => {
			el.removeEventListener('pointerenter', enter);
			el.removeEventListener('pointerleave', leave);
			el.removeEventListener('focusin', focusIn);
			el.removeEventListener('focusout', focusOut);
			el.removeEventListener('keydown', key);
		};
	});
</script>

<div class="toast {tone}" role="group" aria-label={title} bind:this={node}>
	<span class="glyph">
		<Icon icon={tone === 'success' ? UTILITY_ICONS.check : UTILITY_ICONS['triangle-alert']} />
	</span>
	<div class="body">
		<p class="title">{title}</p>
		{#if action !== undefined}
			<!-- eslint-disable svelte/no-navigation-without-resolve -- an external page, never an app route -->
			<a
				class="action"
				href={action.href}
				target="_blank"
				rel="noopener noreferrer"
				onclick={onclose}>{action.label}</a
			>
			<!-- eslint-enable svelte/no-navigation-without-resolve -->
		{/if}
	</div>
	<button type="button" class="close" aria-label={closeLabel} onclick={onclose}>
		<Icon icon={UTILITY_ICONS.x} />
	</button>
</div>

<style>
	.toast {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		width: 100%;
		max-width: var(--layout-promptCard);
		padding: var(--space-sm) var(--space-sm) var(--space-sm) var(--space-lg);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-xl);
		background: var(--color-bg-raised);
		box-shadow: var(--shadow-lg);
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-base);
		pointer-events: auto;
		animation: toast-in var(--motion-sheet-in) ease-out;
	}

	/* 20 at the standard size, growing with the words. */
	.glyph {
		display: flex;
		flex: none;
	}

	.glyph :global(svg) {
		width: 1.25em;
		height: 1.25em;
	}

	.success .glyph {
		color: var(--color-success-base);
	}

	.warning .glyph {
		color: var(--color-warning-base);
	}

	/* The title and its action share a line while both fit; otherwise the
	   action takes the next line, under the title. */
	.body {
		display: flex;
		flex: 1;
		flex-wrap: wrap;
		align-items: baseline;
		column-gap: var(--space-lg);
		row-gap: var(--space-xs);
		min-width: 0;
		padding-block: var(--space-xs);
	}

	.title {
		margin: 0;
		font-weight: var(--weight-semibold);
		overflow-wrap: break-word;
	}

	/* The link colour a step toward the body colour: lighter on the dark
	   raised surface (4.44:1 → 5.2:1), darker on the light one (4.69:1 →
	   5.5:1) — one rule, AA in both themes. */
	.action {
		font-weight: var(--weight-semibold);
		color: color-mix(in srgb, var(--color-info-base) 88%, var(--color-fg-base));
		text-decoration: none;
		white-space: nowrap;
	}

	.action:hover {
		text-decoration: underline;
	}

	.close {
		display: flex;
		flex: none;
		align-items: center;
		justify-content: center;
		align-self: flex-start;
		width: var(--size-control-sm);
		height: var(--size-control-sm);
		padding: 0;
		border: none;
		border-radius: var(--radius-full);
		background: none;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		cursor: pointer;
	}

	.close :global(svg) {
		width: 1.125em;
		height: 1.125em;
	}

	.close:hover {
		background: var(--color-bg-sunken);
		color: var(--color-fg-base);
	}

	.close:active {
		transform: scale(var(--motion-press-button));
	}

	@keyframes toast-in {
		from {
			opacity: 0;
			transform: translateY(var(--space-lg));
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.toast {
			animation: toast-fade var(--motion-duration-fast) ease-out;
		}

		@keyframes toast-fade {
			from {
				opacity: 0;
			}
		}
	}
</style>
