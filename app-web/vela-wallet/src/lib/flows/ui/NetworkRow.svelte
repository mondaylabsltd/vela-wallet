<script lang="ts">
	/**
	 * R1's network row (spec 021 component 9): the chain, the address on it,
	 * and the two things a person does with an address — copy it, or show it.
	 *
	 * Both actions sit on the row rather than behind it. The point of R1 is
	 * that ONE address serves every network, so the fastest path is to copy it
	 * from whichever line you happened to look at, without opening anything.
	 */
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import RemoteLogo from '$lib/wallet/ui/RemoteLogo.svelte';
	import type { NetworkRowModel } from '../model';

	interface Props {
		row: NetworkRowModel;
		/** Swaps the copy glyph for a tick; the caller times it back (150ms). */
		copied?: boolean;
		oncopy?: () => void;
		onqr?: () => void;
	}

	let { row, copied = false, oncopy, onqr }: Props = $props();
</script>

<div class="row">
	<!-- The chain's own logo over the lettered badge, the same way the asset
	     rows wear theirs (spec 028 Phase 9, T492); the letters stand until it
	     loads and stay when the endpoint has none. -->
	<span class="badge" style:background={row.badgeColor} aria-hidden="true">
		{row.code}
		<RemoteLogo urls={row.logoUrl === undefined ? undefined : [row.logoUrl]} />
	</span>
	<span class="text">
		<span class="name">{row.name}</span>
		<span class="address">{row.addressDisplay}</span>
	</span>
	<button type="button" aria-label={row.copyLabel} class:copied onclick={oncopy}>
		<Icon icon={copied ? UTILITY_ICONS.check : UTILITY_ICONS.copy} size="md" />
	</button>
	<button type="button" aria-label={row.qrLabel} onclick={onqr}>
		<Icon icon={UTILITY_ICONS['qr-code']} size="md" />
	</button>
</div>

<style>
	.row {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		padding-block: var(--space-lg);
	}

	.badge {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-chainBadge);
		height: var(--size-chainBadge);
		border-radius: var(--radius-full);
		flex-shrink: 0;
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		/* The chain colours are brand fills, dark enough for white in both
		   appearances — so the mode-invariant white, not fg.inverse. */
		color: var(--color-onAccent);
	}

	.text {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		flex: 1;
		min-width: 0;
	}

	.name {
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.address {
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	button {
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-control-sm);
		height: var(--size-control-sm);
		flex-shrink: 0;
		border: none;
		border-radius: var(--radius-full);
		background: none;
		color: var(--color-fg-muted);
		cursor: pointer;
		transition: color var(--motion-duration-fast) ease;
	}

	button:hover {
		background: var(--color-bg-raised);
		color: var(--color-fg-base);
	}

	.copied {
		color: var(--color-success-base);
	}
</style>
