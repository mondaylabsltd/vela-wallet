<script lang="ts">
	/**
	 * A multisig transaction's call to another contract, drawn by the signing
	 * sheet's own renderer from the signing sheet's own blocks (cs-12): what
	 * it does, what it grants, every warning the machines raised — or, when
	 * nobody can read it, the blind warning and the coin it moves.
	 */
	import { currency } from '$lib/settings/core/currency.svelte';
	import type { SigningMessages } from '$lib/signing/messages';
	import BlockList from '$lib/signing/ui/BlockList.svelte';
	import { callReviewBlocks, type CallReading } from '../core/call-review.svelte';

	interface Props {
		reading: CallReading;
		m: SigningMessages;
		/** The multisig's own alarms, said first and in red (a batch call that changes its members). */
		alerts?: string[];
	}

	let { reading, m, alerts = [] }: Props = $props();

	const blocks = $derived([
		...alerts.map((text) => ({ kind: 'warning' as const, tone: 'danger' as const, text })),
		...callReviewBlocks(reading, m, currency.view)
	]);
</script>

<div class="review" data-testid="multisig-call-review">
	<BlockList {blocks} />
</div>

<style>
	.review {
		display: flex;
		flex-direction: column;
		/* The signing sheet's own spacing between its blocks. */
		gap: var(--space-xl);
		margin-block-end: var(--space-lg);
	}
</style>
