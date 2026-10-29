<script lang="ts">
	/**
	 * What a token nobody vouches for wears wherever it is named (r4a-2): the
	 * core's word on it — "Unverified token", or "Not the real USDC — a
	 * different contract uses this name" in the danger tone — and its
	 * contract, the one thing that tells two tokens of one name apart: short,
	 * with a copy of the whole and a link to it on the explorer.
	 *
	 * `bare`: inside a control (a picker's row) — the words and the short
	 * contract only, no button in a button.
	 */
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import type { ProvenanceNote } from '$lib/services/token-provenance';

	interface Props {
		note: ProvenanceNote;
		/** "Token contract", before the address. */
		contractLabel?: string;
		copyLabel?: string;
		explorerLabel?: string;
		bare?: boolean;
	}

	let { note, contractLabel, copyLabel, explorerLabel, bare = false }: Props = $props();

	let copied = $state(false);
	async function copy() {
		try {
			await navigator.clipboard.writeText(note.contract);
			copied = true;
			setTimeout(() => (copied = false), 1600);
		} catch {
			// nothing to copy into: the address stays on screen
		}
	}
</script>

<span
	class="note"
	class:danger={note.tone === 'danger'}
	class:bare
	data-testid="token-provenance"
	data-tone={note.tone}
>
	<span class="words">
		<Icon icon={UTILITY_ICONS['triangle-alert']} size="xs" />
		<span>{note.text}</span>
	</span>
	<span class="contract">
		{#if contractLabel && !bare}<span class="label">{contractLabel}</span>{/if}
		<span class="address" title={note.contract}>{note.short}</span>
		{#if !bare}
			{#if copyLabel}
				<button
					type="button"
					class="act"
					aria-label={copyLabel}
					data-testid="token-provenance-copy"
					onclick={copy}
				>
					<Icon icon={copied ? UTILITY_ICONS.check : UTILITY_ICONS.copy} size="xs" />
				</button>
			{/if}
			{#if explorerLabel}
				<!-- eslint-disable svelte/no-navigation-without-resolve -- an external block explorer -->
				<a
					class="act"
					href={note.explorerUrl}
					target="_blank"
					rel="noopener noreferrer"
					aria-label={explorerLabel}
				>
					<Icon icon={UTILITY_ICONS['external-link']} size="xs" />
				</a>
				<!-- eslint-enable svelte/no-navigation-without-resolve -->
			{/if}
		{/if}
	</span>
</span>

<style>
	.note {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-xs) var(--space-sm);
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-warning-base);
	}

	.note.danger {
		color: var(--color-error-base);
		font-weight: var(--weight-semibold);
	}

	.words {
		display: inline-flex;
		align-items: center;
		gap: var(--space-xs);
	}

	.contract {
		display: inline-flex;
		align-items: center;
		gap: var(--space-xs);
		color: var(--color-fg-muted);
		font-weight: var(--weight-regular);
	}

	.address {
		font-family: var(--font-mono);
	}

	.act {
		display: inline-flex;
		padding: var(--space-xs);
		border: none;
		background: none;
		color: var(--color-fg-muted);
		cursor: pointer;
	}

	.act:hover {
		color: var(--color-fg-base);
	}
</style>
