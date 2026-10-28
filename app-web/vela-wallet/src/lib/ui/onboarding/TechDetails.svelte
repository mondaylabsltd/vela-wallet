<script lang="ts">
	/**
	 * 技术详情 disclosure (spec 014, FR-004 / E2x anatomy). Collapsed by
	 * default on every entry to a state (the host re-keys per state); the
	 * expanded code block shows the error code (error color), a context line
	 * and an optional endpoint line, mono on bg-sunken.
	 */
	interface Props {
		/** Resolved disclosure label (onboarding.create.technicalDetails). */
		label: string;
		code: string;
		context: string;
		endpoint?: string;
		initialExpanded?: boolean;
		/** Local visual toggle also reports to the host sink (toggle_details). */
		onToggle?: (expanded: boolean) => void;
	}

	let { label, code, context, endpoint, initialExpanded = false, onToggle }: Props = $props();

	// Initial value by design: collapsed is the default on every ENTRY to a
	// state; the panel re-keys this atom per state, so later toggling is
	// purely local (FR-011).
	// svelte-ignore state_referenced_locally
	let expanded = $state(initialExpanded);

	function toggle() {
		expanded = !expanded;
		onToggle?.(expanded);
	}
</script>

<div class="details">
	<button class="toggle" type="button" aria-expanded={expanded} onclick={toggle}>
		<span>{label}</span>
		<svg class="chevron" class:open={expanded} viewBox="0 0 24 24" aria-hidden="true">
			<path d="m6 9 6 6 6-6" />
		</svg>
	</button>
	{#if expanded}
		<div class="block">
			<p class="code">{code}</p>
			<p class="context">{context}</p>
			{#if endpoint !== undefined}
				<p class="endpoint">{endpoint}</p>
			{/if}
		</div>
	{/if}
</div>

<style>
	.details {
		border-block: var(--border-hairline) solid var(--color-border-base);
	}

	.toggle {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-xl);
		width: 100%;
		min-height: var(--size-hitTarget);
		padding: var(--space-lg) 0;
		border: none;
		background: none;
		color: var(--color-fg-muted);
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		cursor: pointer;
	}

	.chevron {
		width: var(--icon-md);
		height: var(--icon-md);
		fill: none;
		stroke: currentColor;
		stroke-width: var(--icon-stroke-base);
		stroke-linecap: round;
		stroke-linejoin: round;
		transition: transform var(--motion-duration-fast) ease;
	}

	.chevron.open {
		transform: rotate(180deg);
	}

	.block {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
		margin-bottom: var(--space-xl);
		padding: var(--space-xl);
		background: var(--color-bg-sunken);
		border-radius: var(--radius-lg);
		font-family: var(--font-mono);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
	}

	.block p {
		margin: 0;
		overflow-wrap: anywhere;
	}

	.code {
		color: var(--color-error-base);
		font-weight: var(--weight-medium);
	}

	.context {
		color: var(--color-fg-muted);
	}

	.endpoint {
		color: var(--color-fg-subtle);
	}
</style>
