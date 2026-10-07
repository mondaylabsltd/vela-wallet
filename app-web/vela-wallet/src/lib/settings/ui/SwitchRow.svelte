<script lang="ts">
	/**
	 * A settings row that is a switch: a title, the line that says what it
	 * means, and the product's on/off track (`$lib/ui/Switch.svelte`'s look) at
	 * the end. The whole row is the target, as every settings row is.
	 *
	 * Used by About's "Share anonymous usage statistics", beside the privacy
	 * policy it is described in.
	 */
	import type { SwitchRowModel } from '../model';

	interface Props {
		row: SwitchRowModel;
		/** Absent in the gallery, where the row is a picture. */
		onchange?: (on: boolean) => void;
	}

	let { row, onchange }: Props = $props();
</script>

<button
	type="button"
	role="switch"
	class="row"
	class:on={row.on}
	aria-checked={row.on}
	data-testid={row.id}
	onclick={() => onchange?.(!row.on)}
>
	<span class="text">
		<span class="title">{row.title}</span>
		<span class="subtitle">{row.subtitle}</span>
	</span>
	<span class="track" aria-hidden="true"><span class="thumb"></span></span>
</button>

<style>
	.row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-lg);
		width: 100%;
		min-height: var(--size-control-lg);
		padding-block: var(--space-lg);
		padding-inline: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		color: var(--color-fg-base);
		text-align: start;
		cursor: pointer;
	}

	.text {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		min-width: 0;
	}

	.title {
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		overflow-wrap: break-word;
	}

	.subtitle {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		overflow-wrap: break-word;
	}

	.track {
		display: flex;
		flex: none;
		align-items: center;
		width: var(--icon-3xl);
		height: var(--icon-lg);
		padding: var(--space-xs);
		border-radius: var(--radius-full);
		background: var(--color-border-strong);
		transition: background var(--motion-duration-fast) ease;
	}

	.on .track {
		justify-content: flex-end;
		background: var(--color-fg-base);
	}

	.thumb {
		width: var(--icon-base);
		height: var(--icon-base);
		border-radius: var(--radius-full);
		background: var(--color-bg-base);
		transition: width var(--motion-duration-fast) ease;
	}

	.row:active .thumb {
		width: var(--icon-lg);
	}

	.row:focus-visible {
		outline: var(--border-emphasis) solid var(--color-border-strong);
		outline-offset: var(--space-xs);
	}
</style>
