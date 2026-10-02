<script lang="ts">
	import type { ActivityRowModel } from '../model';
	import { UTILITY_ICONS } from '../icons';
	import Icon from './Icon.svelte';
	import RemoteLogo from './RemoteLogo.svelte';

	interface Props {
		row: ActivityRowModel;
		/**
		 * Where this row goes. Absent ⇒ the row is a READING and not a control:
		 * no button, no pointer, no press. A contact's 最近往来 is the case that
		 * matters — it has no transaction detail to open (the three other shells
		 * draw those rows as plain rows for the same reason), and a row that
		 * shrinks under a finger and then does nothing is a promise broken every
		 * time (spec 081, dead-controls #15).
		 */
		onclick?: () => void;
	}

	let { row, onclick }: Props = $props();

	const icon = $derived(
		row.kind === 'sent'
			? UTILITY_ICONS['arrow-up-right']
			: row.kind === 'received'
				? UTILITY_ICONS['arrow-down-left']
				: UTILITY_ICONS['link-2']
	);
</script>

{#snippet body()}
	<span class="lead" aria-hidden="true">
		<Icon {icon} size="md" />
		<span
			class="badge"
			class:with-logo={row.badgeLogoUrl !== undefined}
			style:background={row.badgeColor}
		>
			<RemoteLogo urls={row.badgeLogoUrl === undefined ? undefined : [row.badgeLogoUrl]} />
		</span>
	</span>
	<span class="text">
		<span class="title">{row.title}</span>
		<span class="subtitle">{row.subtitle}</span>
	</span>
	<!-- 087 F11: a dApp call that moved no coin of ours has no figure, and
	     draws no amount cell — never an empty one. Spec 093: a grant's
	     allowance stands where the figure would be ("Unlimited" in red), and
	     a swap's coin back sits under what left. -->
	{#if row.amount !== '' || row.unit !== ''}
		<span class="figures">
			<span
				class="amount"
				class:positive={row.positive}
				class:masked={row.masked}
				class:danger={row.danger === true}
			>
				<span class="value">{row.amount}</span>
				<span class="unit">{row.unit}</span>
			</span>
			{#if row.received !== undefined}
				<span class="amount back" class:masked={row.masked}>
					<span class="value">{row.received.amount}</span>
					<span class="unit">{row.received.unit}</span>
				</span>
			{/if}
		</span>
	{/if}
{/snippet}

{#if onclick !== undefined}
	<button type="button" class="row" {onclick}>{@render body()}</button>
{:else}
	<div class="row stated">{@render body()}</div>
{/if}

<style>
	.row {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		width: 100%;
		padding-block: var(--space-lg);
		padding-inline: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		text-align: start;
		cursor: pointer;
	}

	.row:active {
		transform: scale(var(--motion-press-row));
	}

	/* Nothing to open: no invitation, and above all no press. */
	.row.stated,
	.row.stated:active {
		cursor: default;
		transform: none;
	}

	.lead {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: center;
		width: calc(var(--space-2xl) * 2);
		height: calc(var(--space-2xl) * 2);
		border-radius: var(--radius-full);
		background: var(--color-bg-sunken);
		color: var(--color-fg-muted);
		flex-shrink: 0;
	}

	.badge {
		position: absolute;
		right: 0;
		bottom: 0;
		width: var(--icon-xs);
		height: var(--icon-xs);
		border-radius: var(--radius-full);
		border: var(--border-emphasis) solid var(--color-bg-base);
	}

	/* A badge that may carry a logo is a size a logo can be read at. */
	.badge.with-logo {
		width: var(--icon-base);
		height: var(--icon-base);
	}

	.text {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		flex: 1;
		min-width: 0;
	}

	.title {
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.subtitle {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.figures {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		gap: var(--space-xs);
		flex-shrink: 0;
	}

	.amount {
		display: inline-flex;
		align-items: baseline;
		gap: var(--space-sm);
		color: var(--color-fg-base);
	}

	.value {
		font-family: var(--font-numeric);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		font-variant-numeric: tabular-nums;
	}

	.unit {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.positive .value {
		color: var(--color-success-base);
	}

	.danger .value {
		color: var(--color-error-base);
	}

	/* What came back: smaller, under the figure, in the colour of money in. */
	.back .value {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-medium);
		color: var(--color-success-base);
	}

	.masked .value {
		letter-spacing: var(--space-xs);
	}
</style>
