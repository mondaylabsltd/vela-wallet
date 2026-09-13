<script lang="ts">
	import AllowanceEditor from './AllowanceEditor.svelte';
	import AmountHero from './AmountHero.svelte';
	import BalanceChanges from './BalanceChanges.svelte';
	import CodeBlock from './CodeBlock.svelte';
	import DetailCard from './DetailCard.svelte';
	import IntentLabel from './IntentLabel.svelte';
	import IntentSentence from './IntentSentence.svelte';
	import KeyValueRows from './KeyValueRows.svelte';
	import NftHero from './NftHero.svelte';
	import PartyRow from './PartyRow.svelte';
	import PositiveNote from './PositiveNote.svelte';
	import SwapPair from './SwapPair.svelte';
	import WarningBanner from './WarningBanner.svelte';
	import type { Block } from '../model';

	/**
	 * The universal renderer: blocks in mock order, out. Nothing here knows
	 * what a swap or a permit is — which is what lets all 33 scenarios, and
	 * the ones nobody has drawn yet, come out of one code path.
	 */
	interface Props {
		blocks: Block[];
		onchip?: (id: string) => void;
		/** The custom cap's keystrokes, on their way back to the guard. */
		oncustom?: (text: string) => void;
	}

	let { blocks, onchip, oncustom }: Props = $props();
</script>

{#each blocks as block, i (i)}
	{#if block.kind === 'intent'}
		<IntentLabel text={block.text} tone={block.tone} />
	{:else if block.kind === 'amount'}
		<AmountHero line={block.line} card={block.card} note={block.note} />
	{:else if block.kind === 'swap'}
		<SwapPair pay={block.pay} receive={block.receive} />
	{:else if block.kind === 'nft'}
		<NftHero id={block.id} collection={block.collection} />
	{:else if block.kind === 'sentence'}
		<IntentSentence text={block.text} tone={block.tone} />
	{:else if block.kind === 'allowance'}
		<AllowanceEditor
			label={block.label}
			value={block.value}
			valueTone={block.valueTone}
			chips={block.chips}
			note={block.note}
			resultingTotal={block.resultingTotal}
			custom={block.custom}
			{onchip}
			{oncustom}
		/>
	{:else if block.kind === 'party'}
		<PartyRow label={block.label} name={block.name} address={block.address} badge={block.badge} />
	{:else if block.kind === 'rows'}
		<KeyValueRows rows={block.rows} />
	{:else if block.kind === 'warning'}
		<WarningBanner tone={block.tone} text={block.text} />
	{:else if block.kind === 'positive'}
		<PositiveNote text={block.text} />
	{:else if block.kind === 'code'}
		<CodeBlock lines={block.lines} note={block.note} />
	{:else if block.kind === 'card'}
		<DetailCard title={block.title} rows={block.rows} tone={block.tone} />
	{:else}
		<BalanceChanges
			title={block.title}
			rows={block.rows}
			note={block.note}
			noteTone={block.noteTone}
		/>
	{/if}
{/each}
