<script lang="ts">
	import type { PartyBadge } from '../model';

	interface Props {
		label: string;
		name: string;
		address?: string;
		badge?: PartyBadge;
	}

	let { label, name, address, badge }: Props = $props();
</script>

<div class="party">
	<p class="label">{label}</p>
	<div class="line">
		<span class="who">
			<span class="name">{name}</span>
			{#if address}
				<span class="address">{address}</span>
			{/if}
		</span>
		{#if badge}
			<span class="badge" data-tone={badge.tone}>{badge.text}</span>
		{/if}
	</div>
</div>

<style>
	.party {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	.label {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.line {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: var(--space-lg);
	}

	.who {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		min-width: 0;
	}

	.name {
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	/*
	  The whole address, always (spec 089): forty hex digits have no break
	  opportunity, so at the side panel's 360 px the line ran past the sheet's
	  padding and its LAST characters — the ones a person checks against
	  address poisoning — were clipped. It wraps anywhere instead.
	*/
	.address {
		font-family: var(--font-mono);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		overflow-wrap: anywhere;
	}

	.badge {
		flex-shrink: 0;
		padding: var(--space-xs) var(--space-md);
		border-radius: var(--radius-sm);
		background: var(--color-bg-sunken);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		white-space: nowrap;
	}

	[data-tone='success'] {
		background: var(--color-success-soft);
		color: var(--color-success-base);
	}

	[data-tone='caution'] {
		background: var(--color-warning-soft);
		color: var(--color-warning-base);
	}

	[data-tone='danger'] {
		background: var(--color-error-soft);
		color: var(--color-error-base);
	}
</style>
