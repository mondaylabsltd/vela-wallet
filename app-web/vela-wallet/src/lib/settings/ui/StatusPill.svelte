<script lang="ts">
	/**
	 * The one badge every settings screen uses (spec 023).
	 *
	 * Latency (`45ms`), reachability (`在线 · 45ms`, `离线`), provider state
	 * (`已连接`, `未设置`) and compatibility (`兼容`, `不兼容`) are all the same
	 * object in the mocks: a soft-tinted capsule, optionally led by a dot. They
	 * differ only in tone, so they are one component and not four.
	 */
	import type { StatusPillModel } from '../model';

	interface Props {
		pill: StatusPillModel;
	}

	let { pill }: Props = $props();
</script>

<span class="pill {pill.tone}">
	{#if pill.dot}<span class="dot" aria-hidden="true"></span>{/if}
	<span class="label">{pill.label}</span>
</span>

<style>
	.pill {
		display: inline-flex;
		align-items: center;
		gap: var(--space-sm);
		padding-inline: var(--space-md);
		padding-block: var(--space-xs);
		border-radius: var(--radius-full);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-medium);
		white-space: nowrap;
	}

	.dot {
		width: var(--space-md);
		height: var(--space-md);
		border-radius: var(--radius-full);
		background: currentcolor;
		flex-shrink: 0;
	}

	.ok {
		background: var(--color-success-soft);
		color: var(--color-success-base);
	}

	.warn {
		background: var(--color-warning-soft);
		color: var(--color-warning-base);
	}

	.error {
		background: var(--color-error-soft);
		color: var(--color-error-base);
	}

	/* In progress ("Connecting"): the ink of a fact, not the accent — that
	   is only for the action that moves value (vis-6). */
	.accent {
		background: var(--color-bg-sunken);
		color: var(--color-fg-base);
	}

	/* Unset, not failed — the mock greys these rather than colouring them. */
	.neutral {
		background: var(--color-bg-raised);
		color: var(--color-fg-subtle);
	}

	.label {
		font-variant-numeric: tabular-nums;
	}
</style>
