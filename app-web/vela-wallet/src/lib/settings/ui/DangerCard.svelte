<script lang="ts">
	/**
	 * ST1b / DST1's 清理数据 card — the one place in settings that is drawn as
	 * a bordered box rather than a hairline row, because it is the only action
	 * on the screen that cannot be undone.
	 */
	import { UTILITY_ICONS } from '#lib/wallet/icons.ts';
	import Icon from '#lib/wallet/ui/Icon.svelte';

	interface Props {
		title: string;
		subtitle: string;
		/** Desktop shows a text action; the phone shows a bin glyph. */
		action?: string;
		onselect?: () => void;
	}

	let { title, subtitle, action, onselect }: Props = $props();
</script>

<button type="button" class="danger" onclick={onselect}>
	<span class="text">
		<span class="title">{title}</span>
		<span class="subtitle">{subtitle}</span>
	</span>
	{#if action !== undefined}
		<span class="action">{action}</span>
	{:else}
		<span class="glyph"><Icon icon={UTILITY_ICONS['trash-2']} size="lg" /></span>
	{/if}
</button>

<style>
	.danger {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		width: 100%;
		padding: var(--space-xl);
		border: var(--border-hairline) solid
			color-mix(in srgb, var(--color-error-base) 35%, transparent);
		border-radius: var(--radius-lg);
		background: var(--color-error-soft);
		font-family: var(--font-ui);
		text-align: start;
		cursor: pointer;
	}

	.text {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		flex: 1;
		min-width: 0;
	}

	.title {
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-error-base);
	}

	.subtitle {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.action,
	.glyph {
		display: flex;
		color: var(--color-error-base);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		flex-shrink: 0;
	}
</style>
