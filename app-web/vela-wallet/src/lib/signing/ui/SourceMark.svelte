<script lang="ts">
	/**
	 * The faint marker on a row whose label a description wrote (cs-14). A tap
	 * says whose words they are ("Label from a fetched description (…)"); the
	 * row itself never changes.
	 */
	interface Props {
		text: string;
	}

	let { text }: Props = $props();
	let open = $state(false);
</script>

<button
	type="button"
	class="mark"
	aria-label={text}
	aria-expanded={open}
	title={text}
	onclick={() => (open = !open)}>△</button
>{#if open}<span class="said" role="note">{text}</span>{/if}

<style>
	.mark {
		margin-inline-start: var(--space-xs);
		padding: 0;
		border: none;
		background: none;
		font: inherit;
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
		cursor: pointer;
		vertical-align: super;
	}

	.said {
		display: block;
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}
</style>
