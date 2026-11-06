<script lang="ts">
	/**
	 * r14-11a: a line of text whose short address forms are never broken —
	 * the one way the web draws a short address inside wrapping text.
	 *
	 * "0x5fb7e1…47B271" may break after its "…" (Unicode line breaking allows
	 * it, and a CJK line takes it). Each short form here is one `.short-address`
	 * run: the words around it still wrap, and the address moves to the next
	 * line whole. Text with none is drawn exactly as before — one text.
	 *
	 * r16-21: a text that IS a full address (or hash) is drawn as its two
	 * halves (`addressHalves`, 22 + 20), each unbreakable — a line breaks
	 * between them or not at all, never one digit on its own.
	 */
	import { addressHalves, addressRuns } from './address-runs';

	let { text }: { text: string } = $props();

	const halves = $derived(addressHalves(text));
	const runs = $derived(addressRuns(text));
</script>

{#if halves}<span class="address-half">{halves[0]}</span><wbr /><span class="address-half"
		>{halves[1]}</span
	>{:else if runs.some((run) => run.address)}{#each runs as run, i (i)}{#if run.address}<span
				class="short-address">{run.text}</span
			>{:else}{run.text}{/if}{/each}{:else}{text}{/if}

<style>
	.short-address,
	.address-half {
		white-space: nowrap;
	}
</style>
