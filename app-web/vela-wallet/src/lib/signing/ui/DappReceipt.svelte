<script lang="ts">
	/**
	 * Where a dApp transaction lands (spec 077 FR-002).
	 *
	 * `StatusHero` is the send receipt's own centrepiece, used here unchanged:
	 * the disc keeps one size across submitting / submitted / confirmed, and
	 * the ring that fills while it waits is the same ring that closes and turns
	 * green on confirmation. A second drawing of the same moment would be a
	 * second thing to keep honest.
	 *
	 * **Nothing here answers the request.** The dApp was answered before this
	 * was drawn; this watches the chain, which is the person's business and not
	 * the request's. So "Done" closes a surface, never a conversation.
	 */
	import StatusHero from '$lib/flows/ui/StatusHero.svelte';
	import Button from '$lib/ui/Button.svelte';
	import { copyText } from '$lib/services/clipboard';
	import type { DappReceiptModel } from '../dapp-receipt';

	interface Props {
		model: DappReceiptModel;
		/** 0–1 while submitted; `undefined` makes the ring circle instead. */
		progress?: number;
		ondone?: () => void;
	}

	let { model, progress, ondone }: Props = $props();

	let copied = $state(false);
	let timer: ReturnType<typeof setTimeout> | undefined;

	function copyHash(): void {
		if (!model.hash) return;
		void copyText(model.hash.value);
		copied = true;
		clearTimeout(timer);
		timer = setTimeout(() => (copied = false), 1500);
	}

	/** `0x1234…abcd` — the whole thing is on the clipboard, not on the screen. */
	const shortHash = $derived(
		model.hash ? `${model.hash.value.slice(0, 10)}…${model.hash.value.slice(-8)}` : ''
	);
</script>

<section class="receipt" data-testid="dapp-receipt">
	<StatusHero stage={model.stage} title={model.title} captions={model.captions} {progress} />

	{#if model.hash}
		<button type="button" class="hash" onclick={copyHash} aria-live="polite">
			<span class="label">{model.hash.label}</span>
			<span class="value">{copied ? '✓' : shortHash}</span>
		</button>
	{/if}

	{#if model.explorer}
		<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- the chain's own explorer, a URL from the networks table, never an app route -->
		<a class="explorer" href={model.explorer.url} target="_blank" rel="noopener noreferrer">
			{model.explorer.label}
		</a>
	{/if}

	<div class="foot">
		<Button variant="primary" onclick={() => ondone?.()}>{model.cta}</Button>
	</div>
</section>

<style>
	.receipt {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-md);
		padding: var(--space-xl);
		min-height: 100%;
		justify-content: center;
	}

	.hash {
		display: flex;
		gap: var(--space-sm);
		align-items: baseline;
		background: none;
		border: 0;
		padding: 0;
		cursor: pointer;
		font: inherit;
		color: inherit;
	}

	.label {
		color: var(--color-fg-muted);
		font-size: var(--text-base);
	}

	.value {
		font-family: var(--font-mono);
		font-size: var(--text-base);
	}

	.explorer {
		color: var(--color-accent-base);
		font-size: var(--text-base);
		text-decoration: none;
	}

	.explorer:hover {
		text-decoration: underline;
	}

	.foot {
		margin-top: var(--space-md);
		width: 100%;
		max-width: var(--layout-flowColumn);
	}
</style>
