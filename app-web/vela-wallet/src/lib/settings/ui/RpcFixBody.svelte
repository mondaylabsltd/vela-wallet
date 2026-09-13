<script lang="ts">
	/**
	 * SR2 / SR2b / DSR1 — fix one network's RPC.
	 *
	 * The failing and restored states are one component: the badge, the
	 * callout tone, the field outline and the CTA label all come from the
	 * model, and the "where to get one" chips disappear once the answer is no
	 * longer needed. Making them two components would let the two drift, and
	 * the second one is a screen somebody sees for three seconds.
	 */
	import type { RpcFixModel } from '../model';
	import Button from '$lib/ui/Button.svelte';
	import Callout from './Callout.svelte';
	import ChainMark from './ChainMark.svelte';
	import LinkChips from './LinkChips.svelte';
	import StatusPill from './StatusPill.svelte';
	import UrlField from './UrlField.svelte';

	interface Props {
		panel: RpcFixModel;
		onprimary?: () => void;
		/** The URL being typed, and the leave that saves it (spec 028 Phase 8). Absent in the gallery. */
		onfield?: (value: string) => void;
		onfieldblur?: () => void;
	}

	let { panel, onprimary, onfield, onfieldblur }: Props = $props();
</script>

<div class="fix">
	<div class="identity">
		<ChainMark mark={panel.mark} />
		<span class="text">
			<span class="name">{panel.name}</span>
			<span class="meta">{panel.meta}</span>
		</span>
		<StatusPill pill={panel.badge} />
	</div>

	<Callout callout={panel.callout} />
	<UrlField field={panel.field} oninput={onfield} onblur={onfieldblur} />

	<Button variant="primary" shape="rounded" onclick={onprimary}>{panel.primary}</Button>

	{#if panel.providers !== undefined}
		<LinkChips label={panel.providersLabel} links={panel.providers} />
	{/if}
	{#if panel.report !== undefined}
		<a
			class="report"
			href="https://github.com/mondaylabsltd/vela-wallet/issues/new"
			target="_blank"
			rel="noreferrer noopener">{panel.report}</a
		>
	{/if}
</div>

<style>
	.fix {
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
		padding-block: var(--space-md) var(--space-xl);
	}

	.identity {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
	}

	.text {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		flex: 1;
		min-width: 0;
	}

	.name {
		font-size: calc(var(--text-xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	.meta {
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.report {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-info-base);
	}
</style>
