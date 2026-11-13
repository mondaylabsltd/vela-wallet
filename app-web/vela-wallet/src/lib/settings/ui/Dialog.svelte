<script module lang="ts">
	/**
	 * The open dialogs, newest last. Escape and the Tab trap belong to the one
	 * on top: a viewer opened over a dialog closes first, alone (the bottom
	 * sheet's rule, BottomSheet.svelte).
	 */
	const openDialogs: symbol[] = [];
</script>

<script lang="ts">
	/**
	 * The desktop's centred modal (DST4b / DSR1) — what the phone draws as a
	 * bottom sheet. Same content, different container: the SPEC's rule is that
	 * every phone 弹框 becomes an in-panel section or a centred dialog on the
	 * desktop, and these two are the dialogs.
	 *
	 * vis-2: it behaves as the sheet does — focus moves in on open, Tab stays
	 * inside, and focus goes back to what opened it on close.
	 */
	import type { Snippet } from 'svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { dialogPop, scrimFade } from '$lib/ui/dialog-motion';

	interface Props {
		title: string;
		subtitle?: string;
		closeLabel: string;
		onclose?: () => void;
		children: Snippet;
		/** The action row, outside the scroll: never scrolled out of sight. */
		footer?: Snippet;
	}

	let { title, subtitle, closeLabel, onclose, children, footer }: Props = $props();

	const self = Symbol('dialog');
	let panel = $state<HTMLElement>();

	const onTop = () => openDialogs[openDialogs.length - 1] === self;

	function focusables(): HTMLElement[] {
		if (!panel) return [];
		return Array.from(
			panel.querySelectorAll<HTMLElement>(
				'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])'
			)
		).filter((el) => el.offsetParent !== null || el === document.activeElement);
	}

	function onKeydown(event: KeyboardEvent): void {
		if (!onTop()) return;
		if (event.key === 'Escape') {
			// A composition's Escape cancels the composition, not the dialog.
			if (event.isComposing || event.keyCode === 229) return;
			if (onclose === undefined) return;
			event.preventDefault();
			onclose();
			return;
		}
		if (event.key !== 'Tab' || !panel) return;
		const items = focusables();
		if (items.length === 0) {
			event.preventDefault();
			panel.focus({ preventScroll: true });
			return;
		}
		const first = items[0];
		const last = items[items.length - 1];
		const active = document.activeElement;
		const inside = active instanceof Node && panel.contains(active);
		if (event.shiftKey && (active === first || active === panel || !inside)) {
			event.preventDefault();
			last.focus();
		} else if (!event.shiftKey && (active === last || !inside)) {
			event.preventDefault();
			first.focus();
		}
	}

	$effect(() => {
		const node = panel;
		if (!node) return;
		openDialogs.push(self);
		const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
		// Content that focused something of its own (a field) keeps it.
		if (!node.contains(document.activeElement)) node.focus({ preventScroll: true });
		return () => {
			const at = openDialogs.indexOf(self);
			if (at !== -1) openDialogs.splice(at, 1);
			const active = document.activeElement;
			const lost = active === null || active === document.body || node.contains(active);
			if (lost && opener?.isConnected) opener.focus({ preventScroll: true });
		};
	});
</script>

<svelte:window onkeydown={onKeydown} />

<!-- vis-19: it leaves as it came, rather than in one frame. -->
<div class="scrim" role="presentation" onclick={onclose} transition:scrimFade|global></div>
<div
	class="dialog"
	transition:dialogPop|global={{ centred: true }}
	role="dialog"
	aria-modal="true"
	aria-label={title}
	tabindex="-1"
	data-focus-inner
	bind:this={panel}
>
	<!-- A div, not a <header>: the dialog's title row is no second banner. -->
	<div class="head">
		<div class="titles">
			<h2>{title}</h2>
			{#if subtitle !== undefined}<p>{subtitle}</p>{/if}
		</div>
		<button type="button" class="close" aria-label={closeLabel} onclick={onclose}>
			<Icon icon={UTILITY_ICONS.x} size="md" />
		</button>
	</div>
	<div class="content">{@render children()}</div>
	{#if footer}<div class="foot">{@render footer()}</div>{/if}
</div>

<style>
	.scrim {
		position: absolute;
		inset: 0;
		background: var(--color-fixed-backdrop);
		z-index: 10;
	}

	.dialog {
		position: absolute;
		top: 50%;
		left: 50%;
		z-index: 11;
		width: var(--layout-settingsDialogW);
		max-height: 80%;
		transform: translate(-50%, -50%);
		display: flex;
		flex-direction: column;
		padding: var(--space-3xl);
		border-radius: var(--radius-xl);
		background: var(--color-bg-raised);
		border: var(--border-hairline) solid var(--color-border-base);
		box-shadow: var(--shadow-lg);
	}

	.head {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: var(--space-lg);
		margin-bottom: var(--space-xl);
	}

	h2 {
		margin: 0;
		font-size: calc(var(--text-2xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	.titles p {
		margin: var(--space-md) 0 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.close {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--space-4xl);
		height: var(--space-4xl);
		border: none;
		border-radius: var(--radius-full);
		background: var(--color-bg-sunken);
		color: var(--color-fg-muted);
		cursor: pointer;
		flex-shrink: 0;
	}

	/* vis-13: `min-height: 0` lets the content shrink inside the capped
	   dialog and scroll (a flex item will not go below its content height
	   otherwise, and the bottom of a long proposal was cut off). The two
	   `local` covers ride with the content and hide the two `scroll` shades
	   at either end, so a shade shows only where there is more to scroll to. */
	/* A scroller clips at its box, and a full-width last control (the new
	   multisig's Next) stood flush with it: its bottom edge, and any focus
	   ring, were cut (vis-21). A little room inside on each side, handed back
	   outside, so the content lines up exactly where it did. */
	.content {
		min-height: 0;
		margin-inline: calc(var(--space-sm) * -1);
		padding: 0 var(--space-sm) var(--space-sm);
		overflow-y: auto;
		overscroll-behavior: contain;
		background:
			linear-gradient(var(--color-bg-raised) 30%, transparent) top / 100% var(--space-3xl) no-repeat
				local,
			linear-gradient(transparent, var(--color-bg-raised) 70%) bottom / 100% var(--space-3xl)
				no-repeat local,
			radial-gradient(
					farthest-side at 50% 0,
					color-mix(in srgb, var(--color-fixed-shadowInk) 22%, transparent),
					transparent
				)
				top / 100% var(--space-lg) no-repeat scroll,
			radial-gradient(
					farthest-side at 50% 100%,
					color-mix(in srgb, var(--color-fixed-shadowInk) 22%, transparent),
					transparent
				)
				bottom / 100% var(--space-lg) no-repeat scroll;
	}

	.foot {
		flex: none;
	}

	.dialog:focus {
		outline: none;
	}
</style>
