<script lang="ts">
	import SigningHeader from './ui/SigningHeader.svelte';
	import SigningBody from './ui/SigningBody.svelte';
	import type { SigningModel } from './model';

	/**
	 * The phone signing sheet (spec 022) — a bottom sheet over the page that
	 * asked for the signature, so the site you are dealing with never leaves
	 * the screen.
	 *
	 * Dismissal is rejection. The scrim, the drag handle and Escape all do the
	 * same thing, and none of them is labelled "Reject", because a wallet with
	 * a reject button teaches people to reach for it without reading.
	 */
	interface Props {
		model: SigningModel;
		onclose?: () => void;
		onconfirm?: () => void;
		onchip?: (id: string) => void;
		oncustom?: (text: string) => void;
		onfee?: () => void;
		onfeepick?: (id: string) => void;
		onspeed?: () => void;
		onspeedpick?: (id: string) => void;
		onsignwith?: (id: string | null) => void;
	}

	let {
		model,
		onclose,
		onconfirm,
		onchip,
		oncustom,
		onfee,
		onfeepick,
		onspeed,
		onspeedpick,
		onsignwith
	}: Props = $props();
</script>

<svelte:window
	onkeydown={(event: KeyboardEvent) => {
		if (event.key === 'Escape') onclose?.();
	}}
/>

<div class="scrim" role="presentation" onclick={() => onclose?.()}></div>
<div class="sheet" role="dialog" aria-modal="true" aria-label={model.panelTitle}>
	<span class="handle" aria-hidden="true"></span>
	<div class="scroll">
		<SigningHeader dapp={model.dapp} network={model.network} />
		<SigningBody
			{model}
			{onconfirm}
			{onclose}
			{onchip}
			{oncustom}
			{onfee}
			{onfeepick}
			{onspeed}
			{onspeedpick}
			{onsignwith}
		/>
	</div>
</div>

<style>
	/*
		FIXED and above the page, not absolute within it: the host is mounted at
		the end of whichever route a request reaches, and an absolute sheet there
		is laid out against the PAGE — it spanned a desktop window edge to edge
		and let the sidebar's marks and the asset list show through it
		(founder-found 2026-09-18).
	*/
	.scrim {
		position: fixed;
		inset: 0;
		z-index: 20;
		background: var(--color-fixed-backdrop);
	}

	.sheet {
		position: fixed;
		z-index: 21;
		inset-inline: 0;
		bottom: 0;
		display: flex;
		flex-direction: column;
		max-height: 88%;
		background: var(--color-bg-raised);
		border-start-start-radius: var(--radius-2xl);
		border-start-end-radius: var(--radius-2xl);
		animation: rise var(--motion-sheet-in) ease-out;
	}

	/*
		Past the desktop breakpoint a bottom sheet is the wrong object (founder
		ruling 2026-09-05: no bottom sheets on desktop). The same atoms become a
		centred card, the width every other prompt card uses, with nothing to
		drag.
	*/
	@media (min-width: 1280px) {
		.sheet {
			inset: 0;
			margin: auto;
			width: calc(100% - 2 * var(--space-3xl));
			max-width: var(--layout-promptCard);
			height: fit-content;
			max-height: calc(100% - 2 * var(--space-3xl));
			padding-top: var(--space-2xl);
			border: var(--border-hairline) solid var(--color-border-base);
			border-radius: var(--radius-xl);
			box-shadow: var(--shadow-lg);
		}

		.handle {
			display: none;
		}
	}

	@keyframes rise {
		from {
			transform: translateY(var(--space-5xl));
			opacity: 0;
		}
		to {
			transform: translateY(0);
			opacity: 1;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.sheet {
			animation: none;
		}
	}

	.handle {
		align-self: center;
		width: var(--space-5xl);
		height: var(--space-sm);
		border-radius: var(--radius-full);
		background: var(--color-border-strong);
		margin-block: var(--space-lg);
	}

	.scroll {
		overflow-y: auto;
		padding-inline: var(--layout-screenPaddingX);
		padding-bottom: var(--space-3xl);
	}
</style>
