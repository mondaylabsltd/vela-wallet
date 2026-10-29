<script lang="ts">
	/**
	 * r4D-17: an executed transaction's story, on its sheet — opened from the
	 * home or from its Activity row: Created → Approved (each member, when;
	 * counted when it ran) → Executed (who, the transaction, when). The steps
	 * are the core of `timeline.ts`; this draws them.
	 */
	import { fillTemplate } from '$lib/i18n/fill';
	import { formatDateTime } from '$lib/services/locale-format';
	import { explorerTxURL } from '$lib/services/networks';
	import type { TimelineStep } from '../core/timeline';
	import type { MultisigMessages } from '../messages';

	interface Props {
		m: MultisigMessages;
		chainId: number;
		steps: TimelineStep[];
		/** A member's name as this device knows it (else the short address). */
		label: (address: string) => string;
	}

	let { m, chainId, steps, label }: Props = $props();

	function line(step: TimelineStep): string {
		switch (step.kind) {
			case 'created':
				return fillTemplate(m.timeline.created, { name: label(step.who) });
			case 'approved':
				return fillTemplate(
					step.byExecuting ? m.timeline.approvedByExecuting : m.timeline.approved,
					{
						name: label(step.who)
					}
				);
			case 'executed':
				return step.who
					? fillTemplate(m.timeline.executedBy, { name: label(step.who) })
					: m.timeline.executed;
		}
	}

	const when = (ms: number | null) => (ms === null ? '' : formatDateTime(ms));
	const explorer = (hash: string) => {
		try {
			return explorerTxURL(chainId, hash);
		} catch {
			return '';
		}
	};
</script>

<section class="timeline" aria-label={m.timeline.heading} data-testid="proposal-timeline">
	<h3>{m.timeline.heading}</h3>
	<ol>
		{#each steps as step, i (i)}
			<li class={step.kind} data-step={step.kind}>
				<span class="dot" aria-hidden="true"></span>
				<div class="body">
					<p class="what">
						{line(step)}
						{#if step.kind === 'approved' && step.counted !== null}
							<span class="mark" class:off={!step.counted}
								>· {step.counted ? m.timeline.counted : m.timeline.notCounted}</span
							>
						{/if}
					</p>
					{#if step.atMs !== null || step.kind === 'executed'}
						<p class="when">
							{when(step.atMs)}
							{#if step.kind === 'executed' && explorer(step.txHash)}
								{#if step.atMs !== null}·{/if}
								<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- an external block explorer -->
								<a href={explorer(step.txHash)} target="_blank" rel="noopener noreferrer"
									>{m.proposal.explorer}</a
								>
							{/if}
						</p>
					{/if}
				</div>
			</li>
		{/each}
	</ol>
</section>

<style>
	.timeline {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	h3 {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	ol {
		margin: 0;
		padding: 0;
		list-style: none;
		display: flex;
		flex-direction: column;
	}

	li {
		position: relative;
		display: flex;
		gap: var(--space-md);
		padding-block-end: var(--space-md);
	}

	/* The line between one step and the next. */
	li:not(:last-child)::before {
		content: '';
		position: absolute;
		inset-block: var(--space-md) 0;
		inset-inline-start: calc(var(--space-xs) - var(--border-hairline) / 2);
		border-inline-start: var(--border-hairline) solid var(--color-border-base);
	}

	.dot {
		flex-shrink: 0;
		inline-size: var(--space-sm);
		block-size: var(--space-sm);
		margin-block-start: var(--space-xs);
		border-radius: var(--radius-full);
		background: var(--color-fg-muted);
	}

	.executed .dot {
		background: var(--color-success-base);
	}

	.body {
		display: flex;
		flex-direction: column;
		min-inline-size: 0;
	}

	p {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		overflow-wrap: anywhere;
	}

	.what {
		color: var(--color-fg-base);
	}

	.when,
	.mark.off {
		color: var(--color-fg-muted);
	}

	.when a {
		color: inherit;
	}
</style>
