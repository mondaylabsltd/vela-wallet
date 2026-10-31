<script lang="ts">
	/**
	 * r8-1: a multisig transaction's calls as the core read them
	 * (`MultisigReview.reading`) — every call, every leg of every batch —
	 * drawn by the signing sheet's own renderer, with the lines the core's
	 * concerns say about them (unlimited, nested, unreadable, decimals
	 * unknown). The same reading every platform draws: nothing is decoded here.
	 */
	import type { PartyBadge } from '$lib/signing/model';
	import BlockList from '$lib/signing/ui/BlockList.svelte';
	import type { MultisigMessages } from '../messages';
	import type { MultisigReviewOut } from '../core/kernel';
	import { concernBlocks, readingBlocks, type ReadingContext } from '../core/reading-blocks';

	interface Props {
		review: MultisigReviewOut | null;
		m: MultisigMessages;
		/** The network's coin, as a call's coin is named. */
		nativeSymbol: string;
		/** The multisig's own alarms, said first and in red (a batch call that changes its members). */
		alerts?: string[];
		/**
		 * r5reg-15: what the one review knows of an address this call names
		 * ("First time sending here"), by lowercased address — worn by this
		 * call's own row, as the review does not name that address again.
		 */
		badges?: Record<string, PartyBadge>;
		/**
		 * r4a-18: the name the review knows for an address this call names
		 * (one's own account, a member), by lowercased address — the row
		 * reads by it, its address beneath as always.
		 */
		names?: Record<string, string>;
	}

	let { review, m, nativeSymbol, alerts = [], badges = {}, names = {} }: Props = $props();

	const ctx = $derived<ReadingContext>({
		r: m.review.reading,
		callNumber: m.review.callNumber,
		nativeSymbol
	});
	const blocks = $derived([
		...alerts.map((text) => ({ kind: 'warning' as const, tone: 'danger' as const, text })),
		...concernBlocks(review, ctx),
		...readingBlocks(review, ctx).map((block) => {
			if (block.kind !== 'party' || !block.address) return block;
			const key = block.address.toLowerCase();
			const badge = block.badge ?? badges[key];
			return { ...block, name: names[key] ?? block.name, ...(badge ? { badge } : {}) };
		})
	]);
</script>

{#if blocks.length > 0}
	<div class="review" data-testid="multisig-call-review">
		<BlockList {blocks} />
	</div>
{/if}

<style>
	.review {
		display: flex;
		flex-direction: column;
		/* The signing sheet's own spacing between its blocks. */
		gap: var(--space-xl);
		margin-block-end: var(--space-lg);
	}
</style>
