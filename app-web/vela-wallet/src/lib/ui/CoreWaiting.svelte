<script lang="ts">
	/**
	 * A signed-in screen before the core has ruled on whose wallet this is
	 * (perf-3). It is in the prerendered HTML, so a cold phone paints the
	 * screen's shape with the stylesheet instead of a blank page for the
	 * seconds the core's wasm is still arriving.
	 *
	 * Shapes only — no name, no figure, no row that could be read as a fact:
	 * this is a page waiting, not a fixture wallet, and when the core says
	 * there is no wallet here it sends the person to Welcome from this.
	 */
	import SkeletonRow from '$lib/wallet/ui/SkeletonRow.svelte';

	interface Props {
		/** The wallet home's hero figure under the header row. */
		hero?: boolean;
		/** List rows under it. */
		rows?: number;
	}

	let { hero = false, rows = 3 }: Props = $props();
</script>

<div class="waiting" aria-busy="true">
	<div class="column" aria-hidden="true">
		<SkeletonRow />
		{#if hero}
			<div class="hero"><SkeletonRow kind="block" /></div>
		{/if}
		{#each { length: rows }, i (i)}
			<SkeletonRow />
		{/each}
	</div>
</div>

<style>
	.waiting {
		min-height: 100dvh;
		background: var(--color-bg-base);
	}

	.column {
		display: flex;
		flex-direction: column;
		max-width: var(--layout-promptCard);
		margin-inline: auto;
		padding: var(--space-lg) var(--layout-screenPaddingX);
	}

	.hero {
		padding-block: var(--space-2xl);
	}
</style>
