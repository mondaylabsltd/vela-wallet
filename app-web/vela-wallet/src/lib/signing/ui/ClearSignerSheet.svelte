<script lang="ts">
	/**
	 * The Clear Signer's sheet (spec 071): what the wallet shows while the
	 * request is on the signer page, and the one sentence it ends with.
	 *
	 * It lies over whatever the signature started from — the signing sheet, or
	 * the send screen, which has no sheet of its own — because that screen
	 * stays exactly as it was: nothing was signed until the page answers, and a
	 * refusal leaves the request open to be signed another way.
	 *
	 * "Open the page again" is also how a blocked popup gets through: the
	 * swipe that started the signature may have spent its user activation by
	 * the time the operation was assembled, and this tap is a fresh one.
	 */
	import Button from '$lib/ui/Button.svelte';
	import type { ClearSignerModel } from '../model';

	interface Props {
		model: ClearSignerModel;
		onreopen?: () => void;
		/** Cancel while waiting, close after. */
		ondismiss?: () => void;
	}

	let { model, onreopen, ondismiss }: Props = $props();

	/**
	 * Escape is this sheet's: Cancel while waiting, close after. Taken in the
	 * capture phase, before the signing sheet underneath hears it — there,
	 * Escape is the REJECTION of the whole request, which is not what somebody
	 * dismissing this sheet meant.
	 */
	function onkeydown(event: KeyboardEvent): void {
		if (event.key !== 'Escape') return;
		event.stopPropagation();
		ondismiss?.();
	}
</script>

<svelte:document onkeydowncapture={onkeydown} />

<div class="scrim" role="presentation"></div>
<div
	class="sheet"
	role={model.waiting ? 'dialog' : 'alertdialog'}
	aria-modal="true"
	aria-label={model.title}
>
	<div class="text" role="status" aria-live="polite">
		{#if model.waiting}<span class="pulse" aria-hidden="true"></span>{/if}
		<p class="title">{model.title}</p>
		{#if model.hint !== undefined}
			<p class="hint">{model.hint}</p>
		{/if}
	</div>
	<div class="actions">
		{#if model.reopen !== undefined}
			<Button variant="secondary" shape="rounded" onclick={onreopen}>{model.reopen}</Button>
		{/if}
		<Button variant="secondary" shape="rounded" onclick={ondismiss}>{model.dismiss}</Button>
	</div>
</div>

<style>
	/* Above the signing sheet (20/21), which stays where it is underneath. */
	.scrim {
		position: fixed;
		inset: 0;
		z-index: 30;
		background: var(--color-fixed-backdrop);
	}

	.sheet {
		position: fixed;
		z-index: 31;
		inset-inline: 0;
		bottom: 0;
		display: flex;
		flex-direction: column;
		gap: var(--space-2xl);
		padding: var(--space-3xl) var(--layout-screenPaddingX);
		background: var(--color-bg-raised);
		border-start-start-radius: var(--radius-2xl);
		border-start-end-radius: var(--radius-2xl);
		animation: rise var(--motion-sheet-in) ease-out;
	}

	/* No bottom sheets on the desktop: the prompt card, as the signing sheet. */
	@media (min-width: 1280px) {
		.sheet {
			inset: 0;
			margin: auto;
			width: calc(100% - 2 * var(--space-3xl));
			max-width: var(--layout-promptCard);
			height: fit-content;
			border: var(--border-hairline) solid var(--color-border-base);
			border-radius: var(--radius-xl);
			box-shadow: var(--shadow-lg);
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

	.text {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}

	.title {
		margin: 0;
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		line-height: var(--leading-normal);
		color: var(--color-fg-base);
	}

	.hint {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
	}

	/* Waiting is on another page: a quiet beat, not a spinner that claims work here. */
	.pulse {
		width: var(--space-md);
		height: var(--space-md);
		border-radius: var(--radius-full);
		background: var(--color-info-base);
		animation: pulse calc(var(--motion-duration-slow) * 3) ease-in-out infinite;
	}

	@keyframes pulse {
		50% {
			opacity: var(--opacity-dim);
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.sheet,
		.pulse {
			animation: none;
		}
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		justify-content: flex-end;
		gap: var(--space-md);
	}
</style>
