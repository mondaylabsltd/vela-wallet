<script lang="ts">
	import BrandMark from '#lib/ui/BrandMark.svelte';

	interface Props {
		title: string;
		caption: string;
		cta: string;
		onbrowse?: () => void;
	}

	let { title, caption, cta, onbrowse }: Props = $props();
</script>

<div class="empty">
	<BrandMark size={56} />
	<h2>{title}</h2>
	<p>{caption}</p>
	<button type="button" onclick={onbrowse}>{cta}</button>
</div>

<style>
	.empty {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-lg);
		padding-block: var(--space-5xl);
		text-align: center;
	}

	h2 {
		margin: 0;
		font-size: calc(var(--text-2xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	p {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	button {
		margin-top: var(--space-lg);
		height: var(--size-control-lg);
		padding-inline: var(--space-4xl);
		border: var(--border-hairline) solid var(--color-border-strong);
		border-radius: var(--radius-full);
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
		cursor: pointer;
	}

	button:active {
		transform: scale(var(--motion-press-button));
	}
</style>
