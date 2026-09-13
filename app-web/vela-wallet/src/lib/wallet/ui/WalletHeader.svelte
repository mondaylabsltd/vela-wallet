<script lang="ts">
	import type { WalletHeaderModel } from '../model';
	import { UTILITY_ICONS } from '../icons';
	import Icon from './Icon.svelte';
	import Identicon from './Identicon.svelte';

	interface Props {
		header: WalletHeaderModel;
		/**
		 * The name-and-chevron button: the account switcher (founder call,
		 * 2026-09-05). Every signed-in account, and the two ways to add one.
		 * Absent in the gallery, where the header is a picture.
		 */
		onclick?: () => void;
	}

	let { header, onclick }: Props = $props();
</script>

<div class="header">
	<!-- Nested buttons are invalid, so the artwork and the name are siblings.
	     The artwork answers a different question ("is this the account I
	     think it is?") and answers it through the resident viewer, wherever
	     it is drawn; a header with no address yet stays a picture. -->
	<Identicon svg={header.identiconSvg} size="header" address={header.addressFull} />
	<button type="button" class="text" aria-haspopup="dialog" {onclick}>
		<span class="name-row">
			<span class="name">{header.name}</span>
			<Icon icon={UTILITY_ICONS['chevron-down']} size="sm" />
		</span>
		<span class="address">{header.addressDisplay}</span>
	</button>
</div>

<style>
	.header {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		min-width: 0;
	}

	.text {
		display: flex;
		flex-direction: column;
		align-items: start;
		gap: var(--space-xs);
		padding: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		text-align: start;
		cursor: pointer;
		min-width: 0;
	}

	.name-row {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		color: var(--color-fg-base);
		min-width: 0;
	}

	.name {
		font-size: calc(var(--text-xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.address {
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}
</style>
