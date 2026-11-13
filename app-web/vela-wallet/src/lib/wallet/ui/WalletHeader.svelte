<script lang="ts">
	import { networkSpace } from '$lib/network-space/network-space.svelte';
	import { identiconViewer } from '../identicon-viewer.svelte';
	import { receiveIdentity } from '$lib/onboarding/core/receive-identity.svelte';
	import { session } from '$lib/session/core/session.svelte';
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

	/*
	 * home-19: the address copies; the name opens the switcher. A short
	 * address that opened a list of accounts was a surprise to everyone who
	 * tapped it to copy it — which is what every other wallet's does.
	 */
	let copied = $state(false);
	let copiedTimer: ReturnType<typeof setTimeout> | undefined;

	$effect(() => () => clearTimeout(copiedTimer));

	async function copyAddress(): Promise<void> {
		if (receiveIdentity.held(header.addressFull) !== null) return;
		try {
			await navigator.clipboard.writeText(header.addressFull);
		} catch {
			// A refused clipboard is never a silent tap: the viewer shows the
			// address whole, with its own Copy, for the person to take by hand.
			identiconViewer.open({ address: header.addressFull, identiconSvg: header.identiconSvg });
			return;
		}
		copied = true;
		clearTimeout(copiedTimer);
		copiedTimer = setTimeout(() => (copied = false), 1500);
	}
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
		<!-- The name has the whole column: nothing beside it squeezes it.
		     r14-1: a PLAIN button — no `aria-haspopup`. WebKit makes a button
		     that has one a pop-up button, and iOS exposes that with no button
		     trait: on the iPhone, VoiceOver and automation found "a group",
		     and the switcher behind it was out of reach. The name and the
		     chevron are both inside it: a press on either opens the switcher. -->
		<button type="button" class="name-row" {onclick}>
			<!-- The full name on hover: the clamp below is the only place a long
			     name is ever cut, and the switcher this opens prints it whole. -->
			<span class="name" title={header.name}>{header.name}</span>
			<span class="chevron"
				><Icon
					icon={UTILITY_ICONS['chevron-down']}
					size="sm"
				/>{#if header.waiting}<!-- r4D-7:
					     another account has something waiting: a dot the switcher's
					     button carries, said in words for whoever cannot see it. --><span
						class="waiting"
						data-testid="others-waiting"
						title={header.waiting}
					></span><span class="visually-hidden">{header.waiting}</span>{/if}</span
			>
		</button>
		<span class="facts">
			<!-- §37: an account the core holds the address of back (not yet
			     confirmed on-chain, or refuted) offers no copy of it. -->
			{#if onclick && header.addressFull !== '' && receiveIdentity.held(header.addressFull) === null}
				<!-- home-19: the address copies itself (the glyph says so) and
				     confirms it in words; the name above opens the switcher. -->
				<button
					type="button"
					class="address copy"
					data-tap
					data-testid="header-address"
					aria-label={identiconViewer.copyLabel
						? `${identiconViewer.copyLabel} ${header.addressDisplay}`
						: undefined}
					onclick={copyAddress}
				>
					<span>{header.addressDisplay}</span>
					<span class="glyph" aria-hidden="true"
						><Icon icon={UTILITY_ICONS[copied ? 'check' : 'copy']} size="xs" /></span
					>
					{#if copied && identiconViewer.copiedLabel}
						<span class="copied" aria-hidden="true">{identiconViewer.copiedLabel}</span>
					{/if}
				</button>
				<!-- Read, not seen: the same word, for whoever cannot see it land. -->
				<span class="visually-hidden" role="status"
					>{copied ? identiconViewer.copiedLabel : ''}</span
				>
			{:else}
				<span class="address">{header.addressDisplay}</span>
			{/if}
			{#if header.rule}
				<!-- r4D-7: a multisig's rule, beside its address on every layout —
				     the rule in force on its networks now; the way to its members. -->
				<!-- eslint-disable svelte/no-navigation-without-resolve -- the page resolved it -->
				{#if onclick}
					<a
						class="chip rule"
						data-tap
						data-testid="rule-chip"
						href={header.rule.href}
						title={header.rule.title}>{header.rule.label}</a
					>
				{:else}
					<span class="chip rule" title={header.rule.title}>{header.rule.label}</span>
				{/if}
				<!-- eslint-enable svelte/no-navigation-without-resolve -->
			{/if}
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
		{#if header.sync}
			<!-- ob-2: saved here, not yet findable elsewhere. Quiet and neutral:
			     the session keeps trying on its own; after a while of failing,
			     the person may ask it to try now. -->
			<span class="sync" role="status" class:stalled={header.sync.stalled}>
				<span>{header.sync.text}</span>
				{#if header.sync.retry && onclick}
					<button type="button" class="retry" data-tap onclick={() => session.syncPending()}
						>{header.sync.retry}</button
					>
				{/if}
			</span>
		{/if}
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
		/* r4D-12: never smaller than --size-targetMin (the line is 20). */
		min-height: var(--size-targetMin);
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
		position: relative;
		display: flex;
		flex-shrink: 0;
	}

	/* Something waits in another account: a small neutral dot on the chevron. */
	.waiting {
		position: absolute;
		inset-block-start: calc(var(--space-xs) * -1);
		inset-inline-end: calc(var(--space-xs) * -1);
		width: var(--space-sm);
		height: var(--space-sm);
		border-radius: var(--radius-full);
		background: var(--color-fg-base);
		box-shadow: 0 0 0 var(--border-hairline) var(--color-bg-base);
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
		color: var(--color-fg-muted);
	}

	.copy {
		display: inline-flex;
		align-items: center;
		gap: var(--space-xs);
		/* r4D-12: never smaller than --size-targetMin (the line is 14). */
		min-height: var(--size-targetMin);
		cursor: pointer;
	}

	.glyph {
		display: flex;
		color: var(--color-fg-muted);
	}

	/* The confirmation, in words, beside the address — neutral ink: copying
	   moves nothing. */
	.copied {
		font-family: var(--font-ui);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.sync {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: var(--space-xs) var(--space-sm);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
	}

	.sync.stalled {
		color: var(--color-fg-base);
	}

	.retry {
		padding: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: inherit;
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
		text-decoration: underline;
		text-decoration-thickness: var(--border-hairline);
		text-underline-offset: var(--space-xs);
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
	/* r4D-12: the link is the target floor (--size-targetMin) and leaves the
	   artwork's own button at least that much uncovered: its box sits
	   `--space-xs` further out than the disc, and the disc and its glyph are
	   drawn back where they always sat (icon-md, its centre unmoved). */
	.members {
		--slack: calc((var(--size-targetMin) - var(--icon-md)) / 2 + var(--space-xs));
		position: absolute;
		inset-inline-end: calc(var(--space-sm) * -1 - var(--slack));
		inset-block-end: calc(var(--space-sm) * -1 - var(--slack));
		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-targetMin);
		height: var(--size-targetMin);
		border-radius: var(--radius-full);
		color: var(--color-fg-muted);
	}

	.members::before {
		content: '';
		position: absolute;
		inset-inline-start: calc((var(--size-targetMin) - var(--icon-md)) / 2 - var(--space-xs));
		inset-block-start: calc((var(--size-targetMin) - var(--icon-md)) / 2 - var(--space-xs));
		width: var(--icon-md);
		height: var(--icon-md);
		border-radius: var(--radius-full);
		background: var(--color-bg-sunken);
		box-shadow: 0 0 0 var(--border-emphasis) var(--color-bg-base);
	}

	.members :global(svg) {
		position: relative;
		inset-inline-end: var(--space-xs);
		inset-block-end: var(--space-xs);
		width: calc(var(--icon-xs) * 0.85);
		height: calc(var(--icon-xs) * 0.85);
		stroke-width: var(--icon-stroke-bold);
	}

	.chip {
		display: inline-flex;
		align-items: center;
		gap: var(--space-xs);
		/* r4D-12: never smaller than --size-targetMin (the pill was 16). */
		min-height: var(--size-targetMin);
		padding: 0 var(--space-md);
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

	/* r4D-7: the rule, in the chip's own neutral ink. */
	.rule {
		color: var(--color-fg-base);
		white-space: nowrap;
	}

	.space.testnet {
		border-color: var(--color-warning-border);
		background: var(--color-warning-soft);
		color: var(--color-fg-base);
	}
</style>
