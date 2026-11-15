<script lang="ts">
	/**
	 * SR5 — the passkey index is unreachable.
	 *
	 * A full screen rather than a sheet: this one blocks both creating and
	 * signing in, so there is nothing behind it to go back to. The endpoint is
	 * editable right here, because "the service is down" and "you pointed it at
	 * the wrong host" look identical from the inside, and only one of them is
	 * something the person can fix.
	 */
	import type { IndexDownModel } from '../model';
	import BrandMark from '#lib/ui/BrandMark.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Callout from './Callout.svelte';
	import UrlField from './UrlField.svelte';

	interface Props {
		panel: IndexDownModel;
		onretry?: () => void;
		onedit?: () => void;
	}

	let { panel, onretry, onedit }: Props = $props();

	const MARK_SIZE = 40;
</script>

<div class="screen">
	<span class="mark"><BrandMark size={MARK_SIZE} /></span>
	<h1>{panel.title}</h1>
	<p class="subtitle">{panel.subtitle}</p>

	<Callout callout={panel.callout} />
	<UrlField field={panel.field} />

	<div class="actions">
		<Button variant="primary" shape="rounded" onclick={onretry}>{panel.primary}</Button>
		<Button variant="secondary" shape="rounded" onclick={onedit}>{panel.secondary}</Button>
	</div>

	<p class="footer">{panel.footer}</p>
</div>

<style>
	.screen {
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
		height: 100%;
		padding: var(--space-5xl) var(--layout-screenPaddingX);
		background: var(--color-bg-base);
		overflow-y: auto;
	}

	.mark {
		display: flex;
		align-items: center;
		justify-content: center;
		align-self: center;
		width: var(--size-emptyStateCircle);
		height: var(--size-emptyStateCircle);
		border-radius: var(--radius-full);
		background: var(--color-bg-raised);
	}

	h1 {
		margin: 0;
		text-align: center;
		font-size: calc(var(--text-3xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	.subtitle {
		margin: 0;
		text-align: center;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-subtle);
	}

	.actions {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
		margin-block-start: var(--space-xl);
	}

	.footer {
		margin: 0;
		text-align: center;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-subtle);
	}
</style>
