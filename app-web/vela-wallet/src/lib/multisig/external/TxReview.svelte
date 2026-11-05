<script lang="ts">
	/**
	 * r4C-7 / r4D-2: what an outside owner signs, on the connect page, before
	 * the wallet asks — never blind, and the one reading every signer sees
	 * (the Bybit rule): the core page machine read it (`TxShown`: its title
	 * in facts, its hashes, its name, the one review of §3.6), and this draws
	 * it as a member own proposal sheet does (`ReviewBody`: the member change
	 * and its rule, r7-11; a call read by the signing sheet machines; the
	 * state-diff guard, who each leg reaches, what moves; the hashes and the
	 * typed data). Nothing here reads the SafeTx again.
	 *
	 * Which multisig (its name only if the core derives its address from the
	 * genesis that carries it, said to be its record), the network, its
	 * number as Vela writes it ("#7"), and the hashes a hardware wallet
	 * shows — each one copyable, to match. The same review for Vela request
	 * on a paired page and for a link transaction (r5reg-23).
	 */
	import AddressText from '$lib/ui/AddressText.svelte';
	import { fillTemplate } from '$lib/i18n/fill';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import type { SigningMessages } from '$lib/signing/messages';
	import type { MultisigMessages } from '../messages';
	import { readingTitle } from '../core/reading-blocks';
	import { refusedLine } from '../core/approvals-words';
	import { nativeSymbol } from '$lib/services/networks';
	import { chainName, memberWho, shortenAddress } from '../core/display';
	import { published } from './published-names.svelte';
	import { listOf } from '$lib/i18n/list';
	import type { MultisigReviewOut, SafeTx, SafeTxHashes } from '../core/kernel';
	import type { OutsideTxShown } from '../generated/OutsideTxShown';
	import { eip55 } from './eip55';
	import ReviewBody from './ReviewBody.svelte';
	import { amountText, numbered, titleText, tokenLine } from './words';

	interface Props {
		m: MultisigMessages;
		signing: SigningMessages;
		/** The core reading: its title in facts, its hashes, its name, the one review. */
		shown: OutsideTxShown;
		/** The SafeTx itself, for the calls the signing sheet machines read. */
		tx: SafeTx;
	}

	let { m, signing, shown, tx }: Props = $props();
	const w = $derived(m.external);

	const review = $derived((shown.review ?? null) as MultisigReviewOut | null);
	const hashes = $derived(shown.hashes as SafeTxHashes);
	const chainId = $derived(shown.chainId);
	// r11-6: a member is named as everywhere — its published name (the
	// passkey index's, the same on every device) and its short form.
	const title = $derived(
		titleText(m, chainId, shown.title, shown.nonce, (a) => memberWho(a, published.label(a)))
	);
	const recipient = $derived(
		shown.title.kind === 'send' || shown.title.kind === 'call' ? shown.title.to : null
	);
	const token = $derived(
		shown.title.kind === 'send' ? tokenLine(m, chainId, shown.title.token) : null
	);
	/** A batch of transfers alone: each leg, as the core read its token. */
	const lines = $derived(
		shown.title.kind === 'batch'
			? shown.title.transfers.map((t) => ({
					label: amountText(chainId, t.amount, t.token),
					to: t.to
				}))
			: []
	);
	const chainNonce = $derived(shown.chainNonce === null ? null : BigInt(shown.chainNonce));
	const waitsFor = $derived(shown.waitsFor ?? []);
	const nonce = $derived(BigInt(shown.nonce));
	/** Its number is used: a record of what it did, nothing checked before signing. */
	const used = $derived(chainNonce !== null && nonce < chainNonce);

	/** r8-1: the core's reading names a call or a batch; else the multisig's line. */
	const shownTitle = $derived(
		readingTitle(review, {
			r: m.review.reading,
			callNumber: m.review.callNumber,
			nativeSymbol: nativeSymbol(chainId)
		}) || title
	);
	/** The core read a call or a batch (the reading draws its legs). */
	const reading = $derived((review?.reading?.calls.length ?? 0) > 0);
	/**
	 * r8-24: "What it does" always says it — a member change too ("Add
	 * Bob"), never an empty line; the list below gives its rule.
	 */
	const headed = true;

	/** The hashes a hardware wallet shows, to match (the review draws its own once gathered). */
	const hashList = $derived([
		{ label: w.safeTxHash, value: hashes.safeTxHash, id: 'safe-tx' },
		{ label: w.domainHash, value: hashes.domainHash, id: 'domain' },
		{ label: w.messageHash, value: hashes.messageHash, id: 'message' }
	]);
	let copied = $state<string | null>(null);
	async function copy(value: string) {
		try {
			await navigator.clipboard.writeText(value);
			copied = value;
			setTimeout(() => {
				if (copied === value) copied = null;
			}, 1600);
		} catch {
			// it stays on screen, selectable
		}
	}
</script>

<section class="review" data-testid="connect-review" aria-label={w.reviewTitle}>
	<h2>{w.reviewTitle}</h2>
	<dl class="facts">
		<div>
			<dt>{w.multisigLabel}</dt>
			<dd>
				{#if shown.name}
					<!-- r5reg-23: its name, as the members gave it — a claim; the address is who it is. -->
					<span class="name" data-testid="review-name" translate="no">{shown.name}</span>
					<span class="sub" data-testid="review-name-source">{w.nameFromRecord}</span>
				{/if}
				<span class="mono" data-testid="review-safe">{eip55(shown.safe)}</span>
			</dd>
		</div>
		<div>
			<dt>{m.action.network}</dt>
			<dd data-testid="review-network">{chainName(chainId)}</dd>
		</div>
		<div>
			<dt>{w.doesLabel}</dt>
			<dd>
				{#if headed}
					<span class="title" data-testid="review-title">{shownTitle}</span>
				{/if}
				{#if recipient}
					<span class="sub"
						>{m.action.to}
						<span class="mono" data-testid="review-recipient">{recipient}</span></span
					>
				{/if}
				{#if token}<span class="sub" data-testid="review-token">{token}</span>{/if}
			</dd>
		</div>
		<div>
			<dt>{m.action.number}</dt>
			<dd data-testid="review-nonce">
				<!-- r5reg-23: numbered as Vela numbers it, "#7". -->
				{numbered(m, shown.nonce)}
				{#if waitsFor.length > 0}
					<!-- r11 S4: the wallet's own sentence and numbers (the core's `waitsFor`). -->
					<span class="sub" data-testid="review-queued"
						>{waitsFor.length > 1
							? fillTemplate(m.proposal.queuedNoteMany, {
									nonces: listOf(waitsFor.map((n) => numbered(m, n)))
								})
							: fillTemplate(m.proposal.queuedNote, { nonce: waitsFor[0] })}</span
					>
				{/if}
			</dd>
		</div>
	</dl>

	{#if shown.title.kind === 'unsupported'}
		<p class="warn" role="alert">
			<Icon icon={UTILITY_ICONS['triangle-alert']} size="sm" />
			<!-- r10-5: the core's reason in the person's words — never its English `refusal`. -->
			{refusedLine(m, review?.refused?.key) ?? m.review.refusedWhy.other}
		</p>
	{/if}
	{#if lines.length > 0 && !reading}
		<ul class="lines">
			{#each lines as line, i (i)}
				<li>
					<span>{line.label}</span><span class="mono"
						><AddressText text={shortenAddress(line.to)} /></span
					>
				</li>
			{/each}
		</ul>
	{/if}

	{#if review}
		<ReviewBody {m} {signing} {chainId} safe={shown.safe} {review} {used} />
	{:else}
		<!-- The review not gathered (yet): what it carries and its hashes, computed by the core, to match. -->
		{#if tx.data !== '0x' && shown.title.kind !== 'send'}
			<details class="data">
				<summary>{m.proposal.data}</summary>
				<code>{tx.to} {tx.data}</code>
			</details>
		{/if}
		<div class="hashes" data-testid="review-hashes">
			<p class="hashes-title">{w.hashesTitle}</p>
			<p class="fine">{w.hashesNote}</p>
			{#each hashList as h (h.id)}
				<div class="hash">
					<span class="hash-label">{h.label}</span>
					<code class="mono" data-testid="hash-{h.id}">{h.value}</code>
					<button
						type="button"
						class="copy"
						aria-label="{w.copy} {h.label}"
						onclick={() => copy(h.value)}
					>
						<Icon icon={copied === h.value ? UTILITY_ICONS.check : UTILITY_ICONS.copy} size="sm" />
						{copied === h.value ? w.copied : w.copy}
					</button>
				</div>
			{/each}
		</div>
	{/if}
</section>

<style>
	.review {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
	}

	h2 {
		margin: 0;
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
	}

	.facts {
		margin: 0;
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}

	.facts div {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	dt {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	dd {
		margin: 0;
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
	}

	.name,
	.title {
		font-weight: var(--weight-semibold);
	}

	.sub {
		color: var(--color-fg-muted);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
	}

	.mono,
	code {
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		overflow-wrap: anywhere;
	}

	.warn {
		margin: 0;
		display: flex;
		gap: var(--space-sm);
		align-items: flex-start;
		color: var(--color-error-base);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
	}

	.lines {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	.lines li {
		display: flex;
		justify-content: space-between;
		gap: var(--space-md);
	}

	.data summary {
		cursor: pointer;
		color: var(--color-fg-muted);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
	}

	.data code {
		display: block;
		margin-block-start: var(--space-sm);
	}

	.hashes {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
		padding: var(--space-md);
		border-radius: var(--radius-md);
		background: var(--color-bg-sunken);
	}

	.hashes-title {
		margin: 0;
		font-weight: var(--weight-semibold);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
	}

	.fine {
		margin: 0;
		color: var(--color-fg-muted);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
	}

	.hash {
		display: grid;
		grid-template-columns: 1fr auto;
		gap: var(--space-xs) var(--space-sm);
		align-items: center;
	}

	.hash-label {
		grid-column: 1 / -1;
		color: var(--color-fg-muted);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
	}

	.copy {
		display: inline-flex;
		align-items: center;
		gap: var(--space-xs);
		min-height: var(--size-control-sm);
		padding: 0 var(--space-sm);
		border: var(--border-hairline) solid var(--color-border-strong);
		border-radius: var(--radius-md);
		background: var(--color-bg-base);
		color: inherit;
		font: inherit;
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		cursor: pointer;
	}
</style>
