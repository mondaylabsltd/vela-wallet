<script lang="ts">
	/**
	 * Copyable address strip (spec 014, A11 / FR-003). The tail truncates
	 * VISUALLY (ellipsis) but copy writes the full untruncated address.
	 * Clipboard is the only I/O this feature performs (contract §4).
	 */
	import { COPIED_FEEDBACK_MS } from './geometry';

	interface Props {
		address: string;
		/** Resolved a11y label (onboarding.common.copyAddress). */
		copyLabel: string;
		/** Resolved confirmation (onboarding.common.copied). */
		copiedLabel: string;
		/** Reports the press to the host sink (copy_address). */
		onCopy?: () => void;
	}

	let { address, copyLabel, copiedLabel, onCopy }: Props = $props();

	let copied = $state(false);
	let timer: ReturnType<typeof setTimeout> | undefined;

	$effect(() => () => clearTimeout(timer));

	async function copy() {
		onCopy?.();
		try {
			await navigator.clipboard.writeText(address);
		} catch {
			// Clipboard unavailable (permissions/insecure context) — feedback
			// still shows; there is nothing else to do in a pure-UI feature.
		}
		copied = true;
		clearTimeout(timer);
		timer = setTimeout(() => (copied = false), COPIED_FEEDBACK_MS);
	}
</script>

<div class="strip">
	<span class="address">{address}</span>
	<button
		class="copy"
		class:done={copied}
		type="button"
		aria-label={copied ? copiedLabel : copyLabel}
		onclick={copy}
	>
		{#if copied}
			<svg class="glyph" viewBox="0 0 24 24" aria-hidden="true">
				<path d="M20 6 9 17l-5-5" />
			</svg>
		{:else}
			<svg class="glyph" viewBox="0 0 24 24" aria-hidden="true">
				<rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
				<path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
			</svg>
		{/if}
	</button>
	<span class="live" aria-live="polite">{copied ? copiedLabel : ''}</span>
</div>

<style>
	.strip {
		position: relative;
		display: flex;
		align-items: center;
		gap: var(--space-md);
		width: 100%;
		padding: var(--space-sm) var(--space-sm) var(--space-sm) var(--space-xl);
		background: var(--color-bg-sunken);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-lg);
	}

	.address {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		white-space: nowrap;
		text-overflow: ellipsis;
		font-family: var(--font-mono);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-variant-numeric: tabular-nums;
		color: var(--color-fg-base);
		line-height: var(--size-control-md);
	}

	.copy {
		display: grid;
		place-items: center;
		flex: none;
		width: var(--size-control-md);
		height: var(--size-control-md);
		padding: 0;
		border: none;
		background: none;
		border-radius: var(--radius-md);
		color: var(--color-fg-muted);
		cursor: pointer;
		transition: opacity var(--motion-duration-fast) ease;
	}

	.copy:hover {
		opacity: var(--opacity-hover);
		color: var(--color-fg-base);
	}

	.copy.done {
		color: var(--color-success-base);
	}

	.glyph {
		width: var(--icon-md);
		height: var(--icon-md);
		fill: none;
		stroke: currentColor;
		stroke-width: var(--icon-stroke-base);
		stroke-linecap: round;
		stroke-linejoin: round;
	}

	.live {
		position: absolute;
		width: var(--space-xs);
		height: var(--space-xs);
		overflow: hidden;
		clip-path: inset(50%);
		white-space: nowrap;
	}
</style>
