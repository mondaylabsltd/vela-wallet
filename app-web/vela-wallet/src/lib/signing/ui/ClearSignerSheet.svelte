<script lang="ts">
	/**
	 * The Clear Signer's sheet (spec 071, extended by 075): where the signer is,
	 * the pairing with another device, the wait on the page, and the one
	 * sentence it ends with.
	 *
	 * It lies over whatever the request started from — the signing sheet, the
	 * send screen, the create flow's key list — because that screen stays
	 * exactly as it was: nothing was signed or minted until the page answers,
	 * and a refusal leaves the request open to be answered another way.
	 *
	 * "Open the page again" is also how a blocked popup gets through: the
	 * swipe that started the signature may have spent its user activation by
	 * the time the operation was assembled, and this tap is a fresh one.
	 *
	 * The six-digit code is drawn as prominently as the page draws it, and its
	 * confirm is the ONLY way a cross-device request is sent: a person who does
	 * not compare it has no protection against a stand-in page.
	 */
	import Button from '$lib/ui/Button.svelte';
	import QRCard from '$lib/flows/ui/QRCard.svelte';
	import type { ClearSignerModel } from '../model';

	interface Props {
		model: ClearSignerModel;
		onreopen?: () => void;
		/** Cancel while waiting, close after. */
		ondismiss?: () => void;
		/** Spec 075: the two places a Clear Signer can be. */
		onwhere?: (where: 'this_device' | 'other_device') => void;
		/** Spec 075: both screens show the same six digits. */
		onconfirmcode?: () => void;
	}

	let { model, onreopen, ondismiss, onwhere, onconfirmcode }: Props = $props();

	/**
	 * Copy the pairing link. A browser that refuses the clipboard changes
	 * nothing: the link is on screen, in full, to be typed or read out.
	 */
	async function copyLink(link: string): Promise<void> {
		try {
			await navigator.clipboard.writeText(link);
		} catch {
			/* the link is on screen either way */
		}
	}

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
		{#if model.pair !== undefined}
			<p class="hint">{model.pair.hint}</p>
		{/if}
	</div>

	{#if model.pair !== undefined}
		<div class="pair">
			<!-- The link as a code to scan, exactly as the receive screen draws an
			     address: it is data, not decoration. -->
			<QRCard label={model.pair.hint} code={model.pair.qr} />
			<p class="link">{model.pair.link}</p>
			<div class="actions">
				<Button variant="secondary" shape="rounded" onclick={() => void copyLink(model.pair!.link)}>
					{model.pair.copy}
				</Button>
			</div>
			{#if model.code === undefined}
				<p class="hint" role="status">{model.pair.waiting}</p>
			{/if}
		</div>
	{/if}

	{#if model.code !== undefined}
		<!-- The one place a stand-in page is caught. Nothing has been sent yet. -->
		<div class="code">
			<p class="digits">{model.code.text}</p>
			<Button variant="primary" shape="rounded" onclick={onconfirmcode}>
				{model.code.confirm}
			</Button>
		</div>
	{/if}

	<div class="actions">
		{#if model.where !== undefined}
			<Button variant="secondary" shape="rounded" onclick={() => onwhere?.('this_device')}>
				{model.where.thisDevice}
			</Button>
			<Button variant="secondary" shape="rounded" onclick={() => onwhere?.('other_device')}>
				{model.where.otherDevice}
			</Button>
		{/if}
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

	/* Spec 075: the pairing block — the code to scan, the link under it. */
	.pair {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
		align-items: center;
	}

	.link {
		margin: 0;
		max-width: 100%;
		overflow-wrap: anywhere;
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	/* The six digits: as loud here as on the other screen, or nobody compares them. */
	.code {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
		align-items: center;
	}

	.digits {
		margin: 0;
		text-align: center;
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
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
