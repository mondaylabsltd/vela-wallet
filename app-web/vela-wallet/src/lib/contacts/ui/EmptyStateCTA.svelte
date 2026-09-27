<script lang="ts">
	import EmptyState from '$lib/wallet/ui/EmptyState.svelte';
	import { UTILITY_ICONS, type UtilityIconId } from '$lib/wallet/icons';
	import type { EmptyCtaModel } from '../model';

	/**
	 * Spec-015 EmptyState + the CTA slot this feature adds (vocabulary #14):
	 * stacked full-width on mobile, an inline pair on desktop. The primary is
	 * the accent action; the secondary is outline.
	 */
	interface Props {
		empty: EmptyCtaModel;
		layout?: 'mobile' | 'desktop';
		icon?: UtilityIconId;
		onprimary?: () => void;
		onsecondary?: () => void;
	}

	let { empty, layout = 'mobile', icon = 'users-round', onprimary, onsecondary }: Props = $props();
</script>

<div class="wrap {layout}">
	<EmptyState icon={UTILITY_ICONS[icon]} title={empty.title} caption={empty.caption} />
	<div class="ctas">
		<button type="button" class="primary" onclick={onprimary}>{empty.primary}</button>
		<button type="button" class="secondary" onclick={onsecondary}>{empty.secondary}</button>
	</div>
</div>

<style>
	.wrap {
		display: flex;
		flex-direction: column;
		align-items: center;
	}

	/* Balanced: wrapped at the full width, the centred caption left a stub
	   under a long line (「……现有通讯 / 录。」). */
	.wrap :global(.caption) {
		max-width: calc(var(--layout-maxContentWidth) / 2);
		text-wrap: balance;
	}

	.ctas {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
		width: 100%;
		padding-top: var(--space-2xl);
	}

	.desktop .ctas {
		flex-direction: row;
		justify-content: center;
		width: auto;
	}

	button {
		display: flex;
		align-items: center;
		justify-content: center;
		height: var(--size-control-lg);
		border-radius: var(--radius-full);
		font-family: var(--font-ui);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		cursor: pointer;
	}

	.desktop button {
		height: var(--size-control-md);
		padding-inline: var(--space-4xl);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
	}

	.primary {
		border: none;
		background: var(--color-accent-base);
		color: var(--color-onAccent);
	}

	.secondary {
		border: var(--border-hairline) solid var(--color-border-strong);
		background: none;
		color: var(--color-fg-base);
	}

	button:active {
		transform: scale(var(--motion-press-button));
	}
</style>
