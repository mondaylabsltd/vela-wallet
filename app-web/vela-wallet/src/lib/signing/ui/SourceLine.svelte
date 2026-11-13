<script lang="ts">
	/**
	 * One small muted line saying where something came from (cs-14): the
	 * reading's provenance under the intent ("✓ Read by Vela from the
	 * transaction", "△ Described by … — a description, not a guarantee"), the
	 * contract on the card's face, a token named only by its own contract.
	 */
	interface Props {
		text: string;
		tone?: 'neutral' | 'caution' | 'danger';
	}

	let { text, tone = 'neutral' }: Props = $props();
</script>

<p class="source" data-tone={tone} data-testid="signing-source">{text}</p>

<style>
	.source {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		/* r4D-11: information, not a mark — fg.muted holds AA on every sheet
		   (fg.subtle read 3.38:1 under the multisig review's check). */
		color: var(--color-fg-muted);
		overflow-wrap: anywhere;
	}

	[data-tone='caution'] {
		color: var(--color-warning-base);
	}

	[data-tone='danger'] {
		color: var(--color-error-base);
	}
</style>
