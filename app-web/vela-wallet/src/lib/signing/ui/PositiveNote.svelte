<script lang="ts">
	import Icon from '#lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS } from '#lib/wallet/icons.ts';

	interface Props {
		text: string;
		/** The reassurance under the fee row reads quieter than a hero note. */
		quiet?: boolean;
	}

	let { text, quiet = false }: Props = $props();
</script>

<p class="ok" class:quiet>
	<Icon icon={UTILITY_ICONS.check} size="sm" />
	<span>{text}</span>
</p>

<style>
	.ok {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		margin: 0;
		padding: var(--space-lg) var(--space-xl);
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	.quiet {
		padding: 0;
		background: none;
		color: var(--color-success-base);
	}
</style>
