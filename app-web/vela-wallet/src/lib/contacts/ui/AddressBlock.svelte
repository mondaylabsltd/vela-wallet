<script lang="ts">
	import Icon from '#lib/wallet/ui/Icon.svelte';
	import { UTILITY_ICONS } from '#lib/wallet/icons.ts';
	import type { AddressBlockModel } from '../model';

	interface Props {
		address: AddressBlockModel;
		/** 'mobile' keeps the fixture's exact two-line wrap; 'desktop' is one line. */
		layout?: 'mobile' | 'desktop';
		oncopy?: () => void;
	}

	let { address, layout = 'mobile', oncopy }: Props = $props();
</script>

<section class="address {layout}">
	<p class="label">{address.label}</p>
	<div class="value">
		<p class="mono">
			{#if layout === 'mobile'}
				{#each address.lines as line, i (i)}
					<span class="line">{line}</span>
				{/each}
			{:else}
				{address.full}
			{/if}
		</p>
		<button type="button" aria-label={address.copyLabel} onclick={oncopy}>
			<Icon icon={UTILITY_ICONS.copy} size="lg" />
		</button>
	</div>
</section>

<style>
	.address {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}

	.label {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.desktop .label {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.value {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-lg);
	}

	.mono {
		margin: 0;
		font-family: var(--font-mono);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
		overflow-wrap: anywhere;
	}

	.desktop .mono {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
	}

	.line {
		display: block;
		line-height: var(--leading-normal);
	}

	button {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-control-sm);
		height: var(--size-control-sm);
		flex-shrink: 0;
		border: none;
		border-radius: var(--radius-md);
		background: none;
		color: var(--color-fg-muted);
		cursor: pointer;
	}

	button:hover {
		background: var(--color-bg-raised);
		color: var(--color-fg-base);
	}
</style>
