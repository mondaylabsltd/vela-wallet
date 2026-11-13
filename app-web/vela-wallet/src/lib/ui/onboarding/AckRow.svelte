<script lang="ts">
	/**
	 * Acknowledgment checkbox row (spec 014, contract §5). Unchecked =
	 * hairline square (border-strong); checked = inverse-ink fill + inverse ✓ (the accent is only for moving value).
	 * Row 3 passes inline links via `children` — links must stay
	 * individually activatable without toggling the checkbox (the caller's
	 * link handlers call preventDefault; spec-011 e2e lesson).
	 */
	import type { Snippet } from 'svelte';

	interface Props {
		checked: boolean;
		onToggle: () => void;
		/** Plain resolved row text (rows 1–2). */
		label?: string;
		/** Rich row content with inline links (row 3); wins over `label`. */
		children?: Snippet;
	}

	let { checked, onToggle, label, children }: Props = $props();
</script>

<label class="row">
	<input class="native" type="checkbox" {checked} onchange={() => onToggle()} />
	<span class="box" aria-hidden="true">
		<svg class="mark" viewBox="0 0 24 24">
			<path d="M20 6 9 17l-5-5" />
		</svg>
	</span>
	<span class="text">
		{#if children}{@render children()}{:else}{label}{/if}
	</span>
</label>

<style>
	.row {
		display: grid;
		grid-template-columns: auto 1fr;
		gap: var(--space-lg);
		align-items: start;
		cursor: pointer;
	}

	.native {
		position: absolute;
		width: var(--space-xs);
		height: var(--space-xs);
		margin: 0;
		opacity: 0;
		pointer-events: none;
	}

	.box {
		display: grid;
		place-items: center;
		width: var(--icon-lg);
		height: var(--icon-lg);
		margin-top: var(--space-xs);
		border: var(--border-emphasis) solid var(--color-border-strong);
		border-radius: var(--radius-sm);
		background: transparent;
		transition:
			background var(--motion-duration-fast) ease,
			border-color var(--motion-duration-fast) ease;
	}

	.mark {
		width: var(--icon-sm);
		height: var(--icon-sm);
		fill: none;
		stroke: var(--color-fg-inverse);
		stroke-width: var(--icon-stroke-heavy);
		stroke-linecap: round;
		stroke-linejoin: round;
		opacity: 0;
	}

	.native:checked ~ .box {
		background: var(--color-fg-base);
		border-color: var(--color-fg-base);
	}

	.native:checked ~ .box .mark {
		opacity: 1;
	}

	.native:focus-visible ~ .box {
		box-shadow:
			0 0 0 var(--space-xs) var(--color-fixed-focusRingInner),
			0 0 0 var(--space-sm) var(--color-fg-base);
	}

	.text {
		color: var(--color-fg-muted);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-relaxed);
	}
</style>
