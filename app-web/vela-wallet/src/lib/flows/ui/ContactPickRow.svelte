<script lang="ts">
	/**
	 * SD2e's contact row (spec 021 component 12).
	 *
	 * Close cousin of spec 018's `ContactRow`, and deliberately not it: that
	 * row manages a contact (swipe to reveal edit and delete, a favourite star,
	 * a send count). This one PICKS one, so it carries a chevron and nothing
	 * else — every affordance it doesn't have is one that can't fire by
	 * accident while someone is halfway through a transfer.
	 *
	 * The one thing it shares: the artwork beside the button opens the
	 * identicon viewer on this address (founder call, 2026-09-05) — the moment
	 * before a transfer is exactly when "is this who I think it is?" is asked.
	 */
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import Identicon from '$lib/wallet/ui/Identicon.svelte';
	import type { ContactPickModel } from '../model';

	interface Props {
		contact: ContactPickModel['contacts'][number];
		onselect?: () => void;
	}

	let { contact, onselect }: Props = $props();
</script>

<div class="row">
	<Identicon svg={contact.identiconSvg} size="row" address={contact.addressFull} />
	<button type="button" class="main" onclick={onselect}>
		<span class="text">
			<span class="top">
				<span class="name">{contact.name}</span>
				{#if contact.group !== undefined}<span class="tag">{contact.group}</span>{/if}
			</span>
			<span class="address">{contact.addressDisplay}</span>
		</span>
		<Icon icon={UTILITY_ICONS['chevron-right']} size="sm" />
	</button>
</div>

<style>
	.row {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		width: 100%;
	}

	.main {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		flex: 1;
		min-width: 0;
		padding-block: var(--space-lg);
		padding-inline: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		color: var(--color-fg-subtle);
		text-align: start;
		cursor: pointer;
	}

	.main:active {
		transform: scale(var(--motion-press-row));
	}

	.text {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		flex: 1;
		min-width: 0;
	}

	.top {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		min-width: 0;
	}

	.name {
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.tag {
		flex-shrink: 0;
		padding: 0 var(--space-sm);
		border-radius: var(--radius-sm);
		background: var(--color-bg-raised);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.address {
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
</style>
