<script lang="ts">
	import Icon from '#lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS } from '#lib/wallet/icons.ts';
	import AmountHero from './AmountHero.svelte';
	import type { AmountLine } from '../model';

	interface Props {
		pay: AmountLine;
		receive: AmountLine;
	}

	let { pay, receive }: Props = $props();
</script>

<div class="pair">
	<AmountHero line={pay} compact />
	<span class="arrow" aria-hidden="true">
		<Icon icon={UTILITY_ICONS['arrow-down']} size="sm" />
	</span>
	<AmountHero line={receive} compact />
</div>

<style>
	.pair {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
	}

	.arrow {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: var(--space-4xl);
		height: var(--space-4xl);
		border-radius: var(--radius-full);
		background: var(--color-bg-sunken);
		color: var(--color-fg-muted);
	}
</style>
