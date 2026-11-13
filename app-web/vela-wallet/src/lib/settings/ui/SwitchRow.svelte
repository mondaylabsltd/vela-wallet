<script lang="ts">
	/**
	 * A settings row that is a switch: an optional glyph, a title, the line
	 * that says what it means, and the product's on/off track
	 * (`$lib/ui/Switch.svelte`'s look) at the end. The whole row is the target,
	 * as every settings row is.
	 *
	 * Used by "Share anonymous usage statistics" — in 高级 beside the AI agent
	 * and the connected sites on the phone, its own panel on the desktop.
	 */
	import type { SwitchRowModel } from '../model';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';

	interface Props {
		row: SwitchRowModel;
		/** Absent in the gallery, where the row is a picture. */
		onchange?: (on: boolean) => void;
		/** Hairline under the row, as between settings rows. */
		divider?: boolean;
	}

	let { row, onchange, divider = false }: Props = $props();
</script>

<!-- Keyboard focus is the app's one ring (app.css), as on every settings
     row: this row drew a second outline of its own, outside it. -->
<button
	type="button"
	role="switch"
	class="row"
	class:on={row.on}
	class:divider
	aria-checked={row.on}
	data-testid={row.id}
	onclick={() => onchange?.(!row.on)}
>
	{#if row.icon !== undefined}
		<span class="glyph"><Icon icon={UTILITY_ICONS[row.icon]} size="lg" /></span>
	{/if}
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

	.divider {
		border-bottom: var(--border-hairline) solid var(--color-border-base);
	}

	/* SettingsRow's glyph column, on the title's line (the subtitle is always
	   there, so the row is always more than one line). */
	.glyph {
		display: flex;
		align-items: center;
		justify-content: center;
		align-self: flex-start;
		width: var(--icon-2xl);
		height: 1lh;
		flex-shrink: 0;
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.text {
		display: flex;
		flex: 1;
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
</style>
