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
	}

	let { model, onclose, onconfirm, onchip, oncustom, onfee }: Props = $props();
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
		<SigningBody {model} {onconfirm} {onchip} {oncustom} {onfee} />
	</div>
</div>

<style>
	.scrim {
		position: absolute;
		inset: 0;
		background: var(--color-fixed-backdrop);
	}

	.sheet {
		position: absolute;
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
