<script lang="ts">
	interface Props {
		/**
		 * 'row' mimics list-row geometry; 'block' is a free-standing bar;
		 * 'figure' stands in a list row's trailing figure while it is read.
		 */
		kind?: 'row' | 'block' | 'figure';
	}

	let { kind = 'row' }: Props = $props();
</script>

{#if kind === 'row'}
	<div class="row" aria-hidden="true">
		<span class="dot shimmer"></span>
		<span class="lines">
			<span class="line wide shimmer"></span>
			<span class="line narrow shimmer"></span>
		</span>
		<span class="line trail shimmer"></span>
	</div>
{:else if kind === 'figure'}
	<span class="figure shimmer" aria-hidden="true" data-testid="skeleton-figure"></span>
{:else}
	<span class="block shimmer" aria-hidden="true"></span>
{/if}

<style>
	.row {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		padding-block: var(--space-lg);
	}

	.dot {
		width: calc(var(--space-2xl) * 2);
		height: calc(var(--space-2xl) * 2);
		border-radius: var(--radius-full);
		flex-shrink: 0;
	}

	.lines {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
		flex: 1;
	}

	.line {
		height: var(--space-lg);
		border-radius: var(--radius-sm);
	}

	.wide {
		width: 40%;
	}

	.narrow {
		width: 60%;
	}

	.trail {
		width: 18%;
	}

	.block {
		display: block;
		width: 55%;
		height: calc(var(--text-4xl) * var(--text-scale, 1));
		border-radius: var(--radius-md);
	}

	.figure {
		display: inline-block;
		width: calc(var(--space-2xl) * 2.5);
		height: calc(var(--text-lg) * var(--text-scale, 1));
		border-radius: var(--radius-sm);
		vertical-align: middle;
	}

	.shimmer {
		background: var(--color-bg-sunken);
		animation: pulse calc(var(--motion-entrance-fadeUp) * 2) ease-in-out infinite alternate;
	}

	@keyframes pulse {
		from {
			opacity: 1;
		}

		to {
			opacity: var(--opacity-dim);
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.figure {
			display: inline-block;
			width: calc(var(--space-2xl) * 2.5);
			height: calc(var(--text-lg) * var(--text-scale, 1));
			border-radius: var(--radius-sm);
			vertical-align: middle;
		}

		.shimmer {
			animation: none;
		}
	}
</style>
