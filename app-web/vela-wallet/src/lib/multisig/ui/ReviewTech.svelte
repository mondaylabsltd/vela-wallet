<script lang="ts">
	/**
	 * The one review's last layers (§3.6), as the signing sheet ends its own:
	 * the AI agent's opinion in its card (bound to this proposal on this
	 * chain), the three hashes a hardware wallet shows — each copied exactly —
	 * and Technical details: the SafeTx's ten fields as signed and its EIP-712
	 * typed data, highlighted and copied exactly (`RawData`). For every
	 * member, a Vela account's too: anyone can match these against another
	 * device before the passkey.
	 */
	import AgentAdviceCard from '$lib/signing/ui/AgentAdviceCard.svelte';
	import TechDetails from '$lib/signing/ui/TechDetails.svelte';
	import type { AgentAdviceCardModel } from '$lib/agent/advisor';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import type { MultisigMessages } from '../messages';
	import type { MultisigReviewOut } from '../core/kernel';
	import type { TechModel } from '$lib/signing/model';
	import { hashRows } from '../core/review-blocks';

	interface Props {
		m: MultisigMessages;
		review: MultisigReviewOut;
		tech: TechModel;
		advice: AgentAdviceCardModel | null;
		/**
		 * S4 (r7): it already ran — the hashes are a record, not something to
		 * compare before signing: no "if any of them differs, don't sign".
		 */
		executed?: boolean;
	}
	let { m, review, tech, advice, executed = false }: Props = $props();

	let open = $state(false);
	let copied = $state<string | null>(null);
	const rows = $derived(hashRows(review, m));

	async function copy(value: string) {
		try {
			await navigator.clipboard.writeText(value);
			copied = value;
			setTimeout(() => {
				if (copied === value) copied = null;
			}, 1500);
		} catch {
			copied = null;
		}
	}
</script>

{#if advice}
	<AgentAdviceCard card={advice} />
{/if}

<section class="hashes" aria-label={m.review.hashesTitle} data-testid="review-hashes">
	<h3>{m.review.hashesTitle}</h3>
	{#if !executed}<p class="note">{m.external.hashesNote}</p>{/if}
	<dl>
		{#each rows as row (row.label)}
			<div class="row">
				<dt>{row.label}</dt>
				<dd>
					<code data-testid="review-hash">{row.value}</code>
					<button
						type="button"
						class="copy"
						aria-label={`${m.external.copy} ${row.label}`}
						onclick={() => copy(row.value)}
					>
						<Icon
							icon={copied === row.value ? UTILITY_ICONS.check : UTILITY_ICONS.copy}
							size="sm"
						/>
					</button>
				</dd>
			</div>
		{/each}
	</dl>
</section>

<div data-testid="review-tech">
	<TechDetails {tech} {open} ontoggle={() => (open = !open)} />
</div>

<style>
	.hashes {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
		padding: var(--space-md);
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
	}
	h3 {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}
	.note {
		margin: 0;
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}
	dl {
		margin: 0;
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}
	.row {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}
	dt {
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}
	dd {
		margin: 0;
		display: flex;
		align-items: flex-start;
		gap: var(--space-sm);
	}
	code {
		flex: 1;
		min-width: 0;
		font-family: var(--font-mono);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-base);
		overflow-wrap: anywhere;
	}
	.copy {
		flex: none;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-width: var(--size-targetMin);
		min-height: var(--size-targetMin);
		padding: 0;
		border: none;
		border-radius: var(--radius-md);
		background: transparent;
		color: var(--color-fg-muted);
		cursor: pointer;
	}
	.copy:focus-visible {
		outline: var(--space-xs) solid var(--color-fg-base);
		outline-offset: var(--space-xs);
	}
</style>
