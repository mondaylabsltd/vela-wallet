<script lang="ts">
	/**
	 * "Using a custom multisig service (127.0.0.1:8788)": said wherever the
	 * multisig service is used for something the person sees, while it is not
	 * the default — so nobody is on a developer's service without knowing.
	 * Neutral: a fact, not a warning. Nothing at all with the default.
	 */
	import { fillTemplate } from '$lib/i18n/fill';
	import type { MultisigMessages } from '../messages';
	import { genesisService, serviceHost } from '../core/service-state.svelte';

	let { m }: { m: MultisigMessages } = $props();

	const host = $derived(serviceHost(genesisService.custom));
</script>

{#if host !== null}
	<p class="custom-service" data-testid="custom-genesis-service">
		{fillTemplate(m.service.customNote, { host })}
	</p>
{/if}

<style>
	.custom-service {
		margin: 0 0 var(--space-md);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
		overflow-wrap: anywhere;
	}
</style>
