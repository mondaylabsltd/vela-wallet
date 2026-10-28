<script lang="ts">
	import { networkSpace } from '$lib/network-space/network-space.svelte';
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
	<span class="avatar">
		<Identicon svg={header.identiconSvg} size="header" address={header.addressFull} />
		{#if header.rule}
			<!-- A multisig: a members badge on its artwork, the way to its
			     settings; its rule is the badge's name ("2 of 3 members approve"). -->
			<!-- eslint-disable svelte/no-navigation-without-resolve -- the page resolved it -->
			<a
				class="members"
				href={header.rule.href}
				title={header.rule.title}
				aria-label={header.rule.title}><Icon icon={UTILITY_ICONS['users-round']} size="xs" /></a
			>
			<!-- eslint-enable svelte/no-navigation-without-resolve -->
		{/if}
	</span>
	<div class="text">
		<!-- The name has the whole column: nothing beside it squeezes it. -->
		<button type="button" class="name-row" aria-haspopup="dialog" {onclick}>
			<!-- The full name on hover: the clamp below is the only place a long
			     name is ever cut, and the switcher this opens prints it whole. -->
			<span class="name" title={header.name}>{header.name}</span>
			<span class="chevron"><Icon icon={UTILITY_ICONS['chevron-down']} size="sm" /></span>
		</button>
		<span class="facts">
			<!-- vis-16: reachable by keyboard, as the name above it is; it opens
			     the same switcher. -->
			<button type="button" class="address" data-tap aria-haspopup="dialog" {onclick}
				>{header.addressDisplay}</button
			>
			<!-- The space in front (spec 105), beside the account it shows: tapping it
			     offers the other space. Live headers only — the gallery's is a picture. -->
			{#if onclick && networkSpace.words}
				<button
					type="button"
					class="chip space"
					data-tap
					class:testnet={networkSpace.testnet}
					data-testid="network-space"
					onclick={() => networkSpace.openSwitcher()}
				>
					{networkSpace.testnet ? networkSpace.words.testnet : networkSpace.words.mainnet}
				</button>
			{/if}
		</span>
	</div>
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
		flex: 1;
		min-width: 0;
	}

	/* Above the grown targets of the line under it (`data-tap`, vis-12): a
	   press on the name is always the name's. */
	.name-row {
		position: relative;
		z-index: 1;
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		max-width: 100%;
		padding: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		text-align: start;
		color: var(--color-fg-base);
		cursor: pointer;
		min-width: 0;
	}

	.chevron {
		display: flex;
		flex-shrink: 0;
	}

	/* vis-23: the name opens the switcher; under a pointer it says so, as a
	   link does, and the chevron joins it. */
	@media (hover: hover) and (pointer: fine) {
		.name-row:hover .name {
			text-decoration: underline;
			text-decoration-thickness: var(--border-hairline);
			text-underline-offset: var(--space-xs);
		}

		.name-row:hover .chevron {
			color: var(--color-fg-base);
		}
	}

	.name {
		font-size: calc(var(--text-xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		/* Two lines, then the ellipsis: the END of a name is what tells two
		   accounts apart ("xiaoxiao · Key 1" / "… Key 2"). A word breaks only
		   when it is longer than the line by itself. */
		display: -webkit-box;
		-webkit-box-orient: vertical;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		overflow: hidden;
		overflow-wrap: break-word;
		line-height: var(--leading-tight);
	}

	/* The address and what qualifies it — the rule, the space — on one line,
	   wrapping under the address when the column is narrow. */
	.facts {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-xs) var(--space-sm);
		min-width: 0;
	}

	.address {
		padding: 0;
		border: none;
		background: none;
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
		cursor: pointer;
	}

	.avatar {
		position: relative;
		display: inline-flex;
		flex: none;
	}

	/* A multisig's mark: a small soft disc tucked on the artwork's lower
	   edge, set off by a ring of the surface it sits on rather than an
	   outline — it says "members" without covering the art. */
	.members {
		position: absolute;
		inset-inline-end: calc(var(--space-sm) * -1);
		inset-block-end: calc(var(--space-sm) * -1);
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--icon-md);
		height: var(--icon-md);
		border-radius: var(--radius-full);
		background: var(--color-bg-sunken);
		box-shadow: 0 0 0 var(--border-emphasis) var(--color-bg-base);
		color: var(--color-fg-muted);
	}

	.members :global(svg) {
		width: calc(var(--icon-xs) * 0.85);
		height: calc(var(--icon-xs) * 0.85);
		stroke-width: var(--icon-stroke-bold);
	}

	.chip {
		display: inline-flex;
		align-items: center;
		gap: var(--space-xs);
		padding: 0 var(--space-sm);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-full);
		background: var(--color-bg-raised);
		font-family: var(--font-ui);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
		text-decoration: none;
		cursor: pointer;
	}

	.space.testnet {
		border-color: var(--color-warning-border);
		background: var(--color-warning-soft);
		color: var(--color-fg-base);
	}
</style>
