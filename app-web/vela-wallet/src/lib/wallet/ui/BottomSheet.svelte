<script lang="ts">
	import type { Snippet } from 'svelte';
	import { UTILITY_ICONS } from '../icons';
	import Icon from './Icon.svelte';

	interface Props {
		title: string;
		/** Optional trailing icon-button slot in the title row (mock H8: search). */
		trailingIcon?: 'search';
		/** Spec 018 C5/C6: action sheets show no visible title, only the a11y name. */
		hideTitle?: boolean;
		/**
		 * Spec 021: a close button in the title row. The grabber already
		 * dismisses by drag, but every sheet in the wallet-2 mocks draws an
		 * explicit ×, and a sheet reached mid-transfer needs a way out that
		 * does not depend on knowing a gesture.
		 */
		closeLabel?: string;
		/**
		 * How tall the sheet is allowed to grow. `half` is spec 015's 60%;
		 * `tall` (spec 021) is for sheets whose content IS the screen — the
		 * receive QR, a transaction's facts, the contact list.
		 */
		height?: 'half' | 'tall';
		onclose?: () => void;
		children: Snippet;
	}

	let {
		title,
		trailingIcon,
		hideTitle = false,
		closeLabel,
		height = 'half',
		onclose,
		children
	}: Props = $props();
</script>

<div class="scrim" role="presentation" onclick={onclose}></div>
<div class="sheet {height}" role="dialog" aria-modal="true" aria-label={title}>
	<span class="handle" aria-hidden="true"></span>
	{#if !hideTitle}
		<header>
			<h2>{title}</h2>
			{#if trailingIcon === 'search'}
				<span class="trailing"><Icon icon={UTILITY_ICONS.search} size="lg" /></span>
			{/if}
			{#if closeLabel !== undefined}
				<button type="button" class="close" aria-label={closeLabel} onclick={onclose}>
					<Icon icon={UTILITY_ICONS.x} size="lg" />
				</button>
			{/if}
		</header>
	{/if}
	<div class="content">
		{@render children()}
	</div>
</div>

<style>
	.scrim {
		position: absolute;
		inset: 0;
		background: var(--color-fixed-backdrop);
	}

	.sheet {
		position: absolute;
		inset-inline: 0;
		bottom: 0;
		display: flex;
		flex-direction: column;
		background: var(--color-bg-base);
		border-start-start-radius: var(--radius-2xl);
		border-start-end-radius: var(--radius-2xl);
		padding-inline: var(--layout-screenPaddingX);
		padding-bottom: var(--space-3xl);
		animation: rise var(--motion-sheet-in) ease-out;
	}

	.half {
		max-height: 60%;
	}

	.tall {
		max-height: 88%;
	}

	@keyframes rise {
		from {
			transform: translateY(var(--space-5xl));
			opacity: 0;
		}

		to {
			transform: translateY(0);
			opacity: 1;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.sheet {
			animation: none;
		}
	}

	.handle {
		align-self: center;
		width: var(--space-5xl);
		height: var(--space-sm);
		border-radius: var(--radius-full);
		background: var(--color-border-strong);
		margin-block: var(--space-lg);
	}

	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding-block: var(--space-md);
	}

	h2 {
		margin: 0;
		font-size: calc(var(--text-2xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	.trailing {
		color: var(--color-fg-muted);
		display: flex;
	}

	.close {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-control-sm);
		height: var(--size-control-sm);
		margin-inline-end: calc(var(--space-md) * -1);
		border: none;
		border-radius: var(--radius-full);
		background: none;
		color: var(--color-fg-muted);
		cursor: pointer;
	}

	.close:hover {
		background: var(--color-bg-raised);
		color: var(--color-fg-base);
	}

	/* The scroll box spans the whole sheet, its content still on the gutter
	   column: whatever reaches a little past the column — a tap area around
	   a corner badge, a focus ring — lands in the gutter, still inside this
	   box, instead of being cut off or scrolling the sheet sideways. The
	   sideways axis clips; a sheet never scrolls horizontally. */
	.content {
		margin-inline: calc(-1 * var(--layout-screenPaddingX));
		padding-inline: var(--layout-screenPaddingX);
		overflow-x: clip;
		overflow-y: auto;
	}
</style>
