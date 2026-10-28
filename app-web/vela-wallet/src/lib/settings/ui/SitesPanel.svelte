<script lang="ts">
	/**
	 * dapp-11: Settings › Connected sites — every site that holds a grant, the
	 * account it was given, and its Disconnect. The same rows the device-storage
	 * page lists under its connections group (which stays as it is); here they
	 * are where a person looks for who can see their wallet, beside the AI
	 * agent row, not three levels down under storage.
	 */
	import type { SitesPanelModel } from '../model';
	import StorageGroup from './StorageGroup.svelte';

	interface Props {
		panel: SitesPanelModel;
		/** A site's Disconnect, by its origin — asked before it acts. */
		ondisconnect?: (origin: string) => void;
	}

	let { panel, ondisconnect }: Props = $props();
</script>

{#if panel.group.items.length === 0}
	<p class="empty">{panel.empty}</p>
{:else}
	<StorageGroup group={panel.group} onclear={ondisconnect} />
{/if}

<style>
	.empty {
		margin: 0;
		padding-block: var(--space-xl);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}
</style>
