<script lang="ts">
	/**
	 * An on/off switch with its label — the receive code's "include network"
	 * (spec 090), the product's one switch.
	 *
	 * The whole row is the target. A press stretches the thumb toward where it
	 * is going, the shape-deform every pressed control here makes. Monochrome
	 * on purpose: the accent belongs to the one action that moves money, so
	 * "on" is the ink track.
	 */
	interface Props {
		label: string;
		checked: boolean;
		/** Absent in the gallery, where the switch is a picture. */
		onchange?: (checked: boolean) => void;
	}

	let { label, checked, onchange }: Props = $props();
</script>

<button
	type="button"
	role="switch"
	class="switch"
	class:on={checked}
	aria-checked={checked}
	onclick={() => onchange?.(!checked)}
>
	<span class="label">{label}</span>
	<span class="track" aria-hidden="true"><span class="thumb"></span></span>
</button>

<style>
	.switch {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-lg);
		width: 100%;
		padding: var(--space-sm) 0;
		border: none;
		background: none;
		color: var(--color-fg-base);
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		text-align: start;
		cursor: pointer;
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

	.switch:active .thumb {
		width: var(--icon-lg);
	}

	.switch:focus-visible {
		outline: var(--border-emphasis) solid var(--color-border-strong);
		outline-offset: var(--space-xs);
	}
</style>
