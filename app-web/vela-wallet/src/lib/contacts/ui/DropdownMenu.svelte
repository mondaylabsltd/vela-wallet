<script lang="ts">
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import type { MenuModel } from '../model';

	interface Props {
		menu: MenuModel;
		/** Anchoring inside a `position: relative` parent (header ⋯ button). */
		align?: 'start' | 'end';
		/** Viewport coordinates when raised by a contextmenu event. */
		at?: { x: number; y: number };
		/** Gallery component boards render the card in normal flow, undismissed. */
		inline?: boolean;
		onselect?: (label: string) => void;
		onclose?: () => void;
	}

	let { menu, align = 'end', at, inline = false, onselect, onclose }: Props = $props();

	/**
	 * A floating menu is placed at the pointer (or under a button) — and a
	 * pointer near the right or bottom edge of the window would put the card
	 * half off-screen. The group view's ⋯ sits at the far right of its column,
	 * so its menu opened rightwards into nothing and could not be read. The
	 * card is measured once it exists and slid back inside the viewport, with
	 * a margin, in whichever direction it overflowed.
	 */
	let card = $state<HTMLDivElement | undefined>();
	let placed = $state<{ x: number; y: number } | undefined>(undefined);
	const EDGE = 8;

	$effect(() => {
		if (at === undefined || inline || card === undefined) {
			placed = undefined;
			return;
		}
		const { width, height } = card.getBoundingClientRect();
		const x = Math.max(EDGE, Math.min(at.x, window.innerWidth - width - EDGE));
		const y = Math.max(EDGE, Math.min(at.y, window.innerHeight - height - EDGE));
		placed = { x, y };
	});

	const position = $derived(placed ?? at);

	function onkeydown(event: KeyboardEvent) {
		if (event.key === 'Escape') onclose?.();
	}
</script>

<svelte:window {onkeydown} />

{#if !inline}
	<!-- Outside-click dismissal: a transparent full-page layer under the card. -->
	<div class="dismiss" role="presentation" onclick={onclose} oncontextmenu={onclose}></div>
{/if}

<div
	bind:this={card}
	class="menu {align}"
	class:inline
	class:floating={at !== undefined && !inline}
	style:left={position === undefined || inline ? undefined : `${position.x}px`}
	style:top={position === undefined || inline ? undefined : `${position.y}px`}
	role="menu"
	aria-label={menu.label}
	tabindex="-1"
>
	{#each menu.items as item (item.label)}
		<button
			type="button"
			role="menuitem"
			class:destructive={item.destructive}
			class:divider={item.dividerAfter}
			onclick={() => {
				onselect?.(item.label);
				onclose?.();
			}}
		>
			<Icon icon={UTILITY_ICONS[item.icon]} size="base" />
			<span>{item.label}</span>
		</button>
	{/each}
</div>

<style>
	.dismiss {
		position: fixed;
		inset: 0;
	}

	.menu {
		position: absolute;
		top: 100%;
		margin-top: var(--space-md);
		z-index: 1;
		display: flex;
		flex-direction: column;
		min-width: var(--layout-contactsMenuW);
		padding-block: var(--space-md);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
		box-shadow: var(--shadow-lg);
		animation: menu-in var(--motion-crossfade) ease-out;
	}

	.menu.end {
		inset-inline-end: 0;
	}

	.menu.start {
		inset-inline-start: 0;
	}

	.menu.floating {
		position: fixed;
		top: auto;
		margin-top: 0;
		inset-inline: auto;
	}

	.menu.inline {
		position: static;
		margin-top: 0;
		width: fit-content;
	}

	@keyframes menu-in {
		from {
			opacity: 0;
		}

		to {
			opacity: 1;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.menu {
			animation: none;
		}
	}

	button {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		height: var(--size-control-md);
		padding-inline: var(--space-lg);
		border: none;
		background: none;
		color: var(--color-fg-base);
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		text-align: start;
		cursor: pointer;
		white-space: nowrap;
	}

	button:hover {
		background: var(--color-bg-sunken);
	}

	button.destructive {
		color: var(--color-error-base);
	}

	button.divider {
		border-bottom: var(--border-hairline) solid var(--color-border-base);
		margin-bottom: var(--space-md);
		padding-bottom: 0;
	}
</style>
