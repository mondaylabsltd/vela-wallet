<script lang="ts">
	/**
	 * r4C-7 / r4D-2: what an outside owner signs, on the connect page, before
	 * the wallet asks — never blind. Everything here is the core's reading of
	 * the SafeTx itself (`reviewTx`), drawn with the same pieces Vela's own
	 * proposal sheet uses: the one-line title and amounts (`display.ts`), a
	 * call to another contract read by the signing sheet's machines
	 * (`CallReading` + `CallReview`, cs-12), a batch's member changes said
	 * first. Which multisig (its name only if the core derives its address
	 * from the genesis that carries it), the network, the number, and the
	 * hashes a hardware wallet shows — each one copyable, to match.
	 *
	 * r5reg-23: the same review for Vela's request on a paired page and for a
	 * link's transaction: its number as Vela writes it ("#7"), and its name
	 * when the registry's genesis carries it, said to be its record's.
	 */
	import { onDestroy } from 'svelte';
	import { fillTemplate } from '$lib/i18n/fill';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import type { SigningMessages } from '$lib/signing/messages';
	import type { MultisigMessages } from '../messages';
	import { CallReading, readableCalls } from '../core/call-review.svelte';
	import {
		actionRecipient,
		actionTitle,
		chainName,
		formatUnits,
		memberChanges,
		shortenAddress,
		tokenInfo
	} from '../core/display';
	import {
		reviewTx,
		transfersOf,
		validName,
		type Genesis,
		type SafeTx,
		type SafeTxHashes
	} from '../core/kernel';
	import CallReview from '../ui/CallReview.svelte';
	import { eip55 } from './eip55';

	interface Props {
		m: MultisigMessages;
		signing: SigningMessages;
		chainId: number;
		safe: string;
		tx: SafeTx;
		hashes: SafeTxHashes;
		/** The genesis the core derived the multisig's address from, if known: its name. */
		genesis?: Genesis | null;
		/** The chain's next number, when read: "next" or "after #n". */
		chainNonce?: bigint | null;
	}

	let {
		m,
		signing,
		chainId,
		safe,
		tx,
		hashes,
		genesis = null,
		chainNonce = null
	}: Props = $props();
	const w = $derived(m.external);

	const review = $derived.by(() => {
		try {
			return reviewTx(chainId, safe, tx);
		} catch {
			return null;
		}
	});
	const action = $derived(review?.action ?? null);
	const nonce = $derived(BigInt(tx.nonce));
	const numbered = (n: bigint) => fillTemplate(m.action.numberValue, { nonce: n.toString() });
	const name = $derived.by(() => {
		const n = genesis?.name;
		try {
			return n && validName(n) ? n : null;
		} catch {
			return null;
		}
	});

	let title = $state('');
	let token = $state<string | null>(null);
	let lines = $state<{ label: string; to: string }[]>([]);
	$effect(() => {
		const a = action;
		const c = chainId;
		const n = nonce;
		void actionTitle(m, c, a, n).then((t) => (title = t));
		token = null;
		lines = [];
		if (a?.kind === 'transfer') {
			void tokenInfo(c, a.token)
				.then((info) => {
					token = a.token
						? fillTemplate(w.tokenContract, {
								symbol: info.symbol,
								address: shortenAddress(a.token)
							})
						: fillTemplate(w.tokenNative, { symbol: info.symbol, network: chainName(c) });
				})
				.catch(() => (token = a.token ?? null));
		} else if (a?.kind === 'batch' && readableCalls(a) === null) {
			void Promise.all(
				transfersOf(a).map(async (t) => {
					const info = await tokenInfo(c, t.token).catch(() => ({ symbol: '', decimals: 18 }));
					return { label: `${formatUnits(t.amount, info.decimals)} ${info.symbol}`, to: t.to };
				})
			).then((l) => (lines = l));
		}
	});

	let reading = $state<CallReading | null>(null);
	$effect(() => {
		const calls = readableCalls(action);
		if (!calls) return;
		const next = new CallReading(chainId, safe, calls);
		reading = next;
		void next.start();
		return () => {
			next.dispose();
			if (reading === next) reading = null;
		};
	});
	onDestroy(() => reading?.dispose());

	const changes = $derived(memberChanges(m, action));
	const recipient = $derived(actionRecipient(action));

	let copied = $state<string | null>(null);
	async function copy(value: string) {
		try {
			await navigator.clipboard.writeText(value);
			copied = value;
			setTimeout(() => (copied = copied === value ? null : copied), 1600);
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
				{#if name}
					<!-- r5reg-23: its name, as the members gave it — a claim; the address is who it is. -->
					<span class="name" data-testid="review-name" translate="no">{name}</span>
					<span class="sub" data-testid="review-name-source">{w.nameFromRecord}</span>
				{/if}
				<span class="mono" data-testid="review-safe">{eip55(safe)}</span>
			</dd>
		</div>
		<div>
			<dt>{m.action.network}</dt>
			<dd data-testid="review-network">{chainName(chainId)}</dd>
		</div>
		<div>
			<dt>{w.doesLabel}</dt>
			<dd>
				<span class="title" data-testid="review-title">{title || '…'}</span>
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
				{numbered(nonce)}
				{#if chainNonce !== null && nonce > chainNonce}
					<span class="sub">{fillTemplate(w.queuedTx, { nonce: numbered(chainNonce) })}</span>
				{/if}
			</dd>
		</div>
	</dl>

	{#if review && action === null}
		<p class="warn" role="alert">
			<Icon icon={UTILITY_ICONS['triangle-alert']} size="sm" />
			{fillTemplate(w.refusedKind, { reason: review.refusal ?? '' })}
		</p>
	{/if}
	{#if reading}
		<CallReview {reading} m={signing} alerts={changes} />
	{:else if changes.length > 0}
		{#each changes as change, i (i)}
			<p class="warn" role="alert">{change}</p>
		{/each}
	{/if}
	{#if lines.length > 0}
		<ul class="lines">
			{#each lines as line, i (i)}
				<li><span>{line.label}</span><span class="mono">{shortenAddress(line.to)}</span></li>
			{/each}
		</ul>
	{/if}
	{#if tx.data !== '0x' && action?.kind !== 'transfer'}
		<details class="data">
			<summary>{m.proposal.data}</summary>
			<code>{tx.to} {tx.data}</code>
		</details>
	{/if}

	<div class="hashes" data-testid="review-hashes">
		<p class="hashes-title">{w.hashesTitle}</p>
		<p class="fine">{w.hashesNote}</p>
		{#each [{ label: w.safeTxHash, value: hashes.safeTxHash, id: 'safe-tx' }, { label: w.domainHash, value: hashes.domainHash, id: 'domain' }, { label: w.messageHash, value: hashes.messageHash, id: 'message' }] as h (h.id)}
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
