<script lang="ts">
	/**
	 * The one review (§3.6) of a SafeTx as every signer reads it — the core
	 * reading (`MultisigReviewOut`), drawn with the pieces a member own
	 * proposal sheet uses: who it adds, removes or replaces and the rule
	 * after (`MemberChangeRule`, r7-11), a call read by the signing sheet
	 * machines (`CallReview`, wearing what the review knows of each party),
	 * the state-diff guard, who each leg reaches and what moves
	 * (`reviewBlocks`), the hashes and the typed data (`ReviewTech`). Shared
	 * by the connect page review and the sheet that asks an outside wallet:
	 * one reading, one drawing — nothing here reads the SafeTx again.
	 */
	import { untrack } from 'svelte';
	import BlockList from '$lib/signing/ui/BlockList.svelte';
	import type { SigningMessages } from '$lib/signing/messages';
	import type { SimEnded } from '$lib/signing/verdict';
	import { currency } from '$lib/settings/core/currency.svelte';
	import { resolvedFormatKeys } from '$lib/services/locale-format';
	import { nativeSymbol } from '$lib/services/networks';
	import type { MultisigMessages } from '../messages';
	import { CallReading, callReviewBlocks, reviewedCalls } from '../core/call-review.svelte';
	import { simEndedOf } from '../core/proposal-review.svelte';
	import {
		partyAddresses,
		recipientBadges,
		recipientNames,
		reviewBlocks,
		reviewTech,
		type ReviewContext
	} from '../core/review-blocks';
	import { chainName } from '../core/display';
	import type { MultisigReviewOut, SafeTx } from '../core/kernel';
	import CallReview from '../ui/CallReview.svelte';
	import MemberChangeRule from '../ui/MemberChangeRule.svelte';
	import ReviewTech from '../ui/ReviewTech.svelte';

	interface Props {
		m: MultisigMessages;
		signing: SigningMessages;
		chainId: number;
		/** The multisig: who the calls are read from. */
		safe: string;
		/** The SafeTx itself, for the calls the signing sheet machines read. */
		tx: SafeTx;
		/** The core one review of it. */
		review: MultisigReviewOut;
		/** Its number is used: a record of what it did, nothing checked before signing. */
		used?: boolean;
		/** How a member is named in a change (the name known for them and the short address). */
		who?: (address: string) => string;
		/** The call reading, once started: its title heads the review (bound). */
		reading?: CallReading | null;
	}

	let {
		m,
		signing,
		chainId,
		safe,
		tx,
		review,
		used = false,
		who,
		reading = $bindable(null)
	}: Props = $props();

	const network = $derived(chainName(chainId));
	const changes = $derived(review.memberChanges);

	/*
	 * A call (or a transfer) read by the signing sheet machines, as a member
	 * sheet reads it — keyed by the call itself, so a review read again does
	 * not read it again.
	 */
	const callKey = $derived.by(() => {
		const calls = reviewedCalls(review.review.action, tx);
		if (!calls) return '';
		const each = calls.map((c) => [c.to, c.value, c.data].join(':')).join(';');
		return [chainId, safe, each].join(':');
	});
	$effect(() => {
		if (!callKey) return;
		const { action, from, chain, signed } = untrack(() => ({
			action: review.review.action,
			from: safe,
			chain: chainId,
			signed: tx
		}));
		const calls = reviewedCalls(action, signed);
		if (!calls) return;
		const next = new CallReading(chain, from, calls);
		reading = next;
		void next.start();
		return () => {
			next.dispose();
			if (reading === next) reading = null;
		};
	});

	/** The simulation as the signing sheet draws it, judged by the token rules for the Safe. */
	let sim = $state.raw<SimEnded | null>(null);
	$effect(() => {
		const r = review;
		const chain = chainId;
		const from = safe;
		let live = true;
		void simEndedOf(r, chain, from).then((s) => {
			if (live) sim = s;
		});
		return () => {
			live = false;
		};
	});
	const ctx = $derived<ReviewContext>({
		m,
		signing,
		network,
		nativeSymbol: nativeSymbol(chainId),
		preset: resolvedFormatKeys().number,
		// No names but the review own: an address is who it is.
		nameOf: () => null
	});
	/** r5reg-15: the addresses the call reading names already: said once. */
	const drawn = $derived(
		reading ? partyAddresses(callReviewBlocks(reading, signing, currency.view)) : []
	);
	const checks = $derived(used ? [] : reviewBlocks(review, sim, ctx, drawn));
	const badges = $derived(recipientBadges(review, ctx));
	const names = $derived(recipientNames(review, ctx));
	const tech = $derived(reviewTech(review, ctx));
</script>

{#if changes}
	<!-- r4a-1 / r7-11: who it changes, and the rule before and after — said first. -->
	<MemberChangeRule {m} {changes} {network} {who} done={used} />
{/if}
{#if reading}
	<CallReview {reading} m={signing} {badges} {names} />
{/if}
{#if checks.length > 0}
	<!-- §3.6: the state-diff guard, who each leg reaches, what moves — the signing sheet blocks. -->
	<div class="checks" data-testid="review-checks">
		<BlockList blocks={checks} />
	</div>
{/if}
<ReviewTech {m} {review} {tech} advice={null} executed={used} />
