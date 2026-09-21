<script lang="ts">
	/**
	 * ST11 / DST5 — one card per provider: name, connection badge, key field
	 * with its own in-field action, and a support line. `未设置` is a neutral
	 * badge, not an error: not having an Alchemy key is a normal state.
	 */
	import type { RpcProvidersModel } from '../model';
	import StatusPill from './StatusPill.svelte';
	import UrlField from './UrlField.svelte';

	interface Props {
		panel: RpcProvidersModel;
		onaction?: (id: string) => void;
		/** Live wiring (spec 024). Absent = the gallery's pure picture. */
		onfield?: (id: string, value: string) => void;
		onfieldblur?: (id: string) => void;
	}

	let { panel, onaction, onfield, onfieldblur }: Props = $props();
</script>

<p class="description">{panel.description}</p>

<div class="providers">
	{#each panel.providers as provider (provider.id)}
		<section class="provider">
			<header>
				<h3>{provider.name}</h3>
				<StatusPill pill={provider.badge} />
			</header>
			<UrlField
				field={provider.field}
				action={provider.action}
				onaction={() => onaction?.(provider.id)}
				oninput={(value) => onfield?.(provider.id, value)}
				onblur={() => onfieldblur?.(provider.id)}
			/>
			{#if provider.support !== undefined}
				<p class="support">{provider.support}</p>
			{/if}
			{#if provider.link !== undefined && provider.linkUrl !== undefined}
				<a class="link" href={provider.linkUrl} target="_blank" rel="noreferrer noopener">
					{provider.link}
				</a>
			{/if}
		</section>
	{/each}
</div>

<style>
	.description {
		margin: 0 0 var(--space-3xl);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
	}

	.providers {
		display: flex;
		flex-direction: column;
		gap: var(--space-4xl);
	}

	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-md);
		margin-bottom: var(--space-lg);
	}

	h3 {
		margin: 0;
		font-size: calc(var(--text-xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	.support {
		margin: var(--space-md) 0 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.link {
		display: inline-block;
		margin-block-start: var(--space-md);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-info-base);
	}
</style>
