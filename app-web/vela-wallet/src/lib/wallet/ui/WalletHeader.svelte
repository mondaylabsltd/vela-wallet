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
			<!-- The full name on hover: the ellipsis below is the only place a long
			     name is ever cut, and the switcher this opens prints it whole. -->
			<span class="name" title={header.name}>{header.name}</span>
			<span class="chevron"><Icon icon={UTILITY_ICONS['chevron-down']} size="sm" /></span>
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
		/* `min-width: 0` was never enough. This row is a start-aligned item of a
		   column flex BUTTON, and there it takes its max-content width: a long
		   name ran straight out of the sidebar and over the next column, the
		   ellipsis on `.name` never fired, and the chevron — the only sign this
		   is a switcher — went with it ("xiaoxiao · Key 2" already touched the
		   edge; founder, 2026-09-20). Held to the button, the name gives way. */
		max-width: 100%;
	}

	.chevron {
		display: flex;
		flex-shrink: 0;
	}

	.name {
		font-size: calc(var(--text-xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		/* Two lines, then the ellipsis. One line cut "xiaoxiao · Key 1" and
		   "xiaoxiao · Key 2" to the same "xiaoxiao · K…" — the sidebar leaves a
		   name about 137px, and the END of a name is what tells two accounts
		   apart. `anywhere` because a name can be one unbroken word. */
		display: -webkit-box;
		-webkit-box-orient: vertical;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		overflow: hidden;
		overflow-wrap: anywhere;
		line-height: var(--leading-tight);
	}

	.address {
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}
</style>
