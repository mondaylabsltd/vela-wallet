<script lang="ts">
	interface Props {
		/** The single value-moving action on this screen — accent is licensed here. */
		label: string;
		caption: string;
		disabled?: boolean;
		onclick?: () => void;
	}

	let { label, caption, disabled = false, onclick }: Props = $props();
</script>

<div class="bar">
	<button type="button" class="cta" {disabled} {onclick}>{label}</button>
	<p class="caption">{caption}</p>
</div>

<style>
	.bar {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-md);
		padding: var(--space-lg) var(--layout-screenPaddingX) var(--space-2xl);
		background: var(--color-bg-base);
	}

	.cta {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 100%;
		height: var(--size-control-lg);
		border: none;
		border-radius: var(--radius-full);
		background: var(--color-fg-base);
		color: var(--color-fg-inverse);
		font-family: var(--font-ui);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		cursor: pointer;
	}

	.cta:active {
		transform: scale(var(--motion-press-button));
	}

	.cta:disabled {
		opacity: var(--opacity-disabled);
		cursor: default;
	}

	.caption {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
		text-align: center;
	}
</style>
