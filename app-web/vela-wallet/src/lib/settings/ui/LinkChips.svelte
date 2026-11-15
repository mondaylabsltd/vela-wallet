<script lang="ts">
	/**
	 * SR2's "where to get a working RPC" row — four outbound chips. Small,
	 * quiet and outbound-marked, so they read as a list of places rather than
	 * as four competing buttons under the one CTA that matters.
	 */
	import { UTILITY_ICONS } from '#lib/wallet/icons.ts';
	import Icon from '#lib/wallet/ui/Icon.svelte';

	interface Props {
		label?: string;
		links: { label: string; href: string }[];
	}

	let { label, links }: Props = $props();
</script>

{#if label !== undefined}<p class="label">{label}</p>{/if}
<ul class="chips">
	{#each links as link (link.href)}
		<li>
			<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- fixture-supplied external RPC-provider URLs, not app routes -->
			<a href={link.href} target="_blank" rel="noreferrer noopener">
				{link.label}
				<Icon icon={UTILITY_ICONS['external-link']} size="xs" />
			</a>
		</li>
	{/each}
</ul>

<style>
	.label {
		margin: 0 0 var(--space-md);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.chips {
		list-style: none;
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-md);
		margin: 0;
		padding: 0;
	}

	a {
		display: inline-flex;
		align-items: center;
		gap: var(--space-sm);
		padding-inline: var(--space-lg);
		padding-block: var(--space-md);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-md);
		background: var(--color-bg-raised);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
		text-decoration: none;
	}
</style>
