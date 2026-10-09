<script lang="ts">
	/**
	 * "Where you review and sign" (spec 102) — one account's venue: in Vela's
	 * own sheet, or on a trusted page.
	 *
	 * The two are NOT two kinds of key. The same keys sign in either place (D2);
	 * what differs is who shows the person what they are signing — Vela, which
	 * sits on many libraries, or a zero-dependency page whose build can be
	 * checked. So the list is drawn as exactly that choice: Vela's sheet, then
	 * the pages under one heading that says what a trusted page IS, each with
	 * the one line that backs the word "trusted" (its integrity check).
	 *
	 * A choice that cannot reach this account's keys (R1) stays on the list,
	 * disabled, with the core's reason under it — a row that vanished could not
	 * say why. The account's own signing domain closes the list: it is the fact
	 * every reason refers to.
	 *
	 * `readOnly` is the web: it opens no signing page, so the list is stated
	 * rather than offered — where it signs is marked, and every page row is
	 * drawn disabled with the core's reason (D-16).
	 */
	import type { SigningVenue } from '$lib/core/generated/SigningVenue';
	import type { VenueModel } from '../model';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import IntegrityLine from './IntegrityLine.svelte';

	interface Props {
		venue: VenueModel;
		/** A reachable row was chosen. Absent in the gallery and on the web. */
		onpick?: (venue: SigningVenue) => void;
		/**
		 * Close with the account's signing domain. Off where the same screen
		 * already says it (the wide account panel, whose keys block names a
		 * custom domain right above).
		 */
		showDomain?: boolean;
	}

	let { venue, onpick, showDomain = true }: Props = $props();

	const inVela = $derived(venue.rows.filter((row) => row.page === undefined));
	const pages = $derived(venue.rows.filter((row) => row.page !== undefined));
	/** The pages' shared heading: what a trusted page is, said once. */
	const pageGroup = $derived(pages[0]);
</script>

{#snippet choice(row: VenueModel['rows'][number])}
	{@const disabled = venue.readOnly === true || row.blocked !== undefined}
	<li>
		<button
			type="button"
			class="row"
			class:active={row.active}
			class:blocked={row.blocked !== undefined}
			role="radio"
			aria-checked={row.active}
			aria-disabled={disabled}
			disabled={row.blocked !== undefined}
			onclick={() => {
				if (!disabled && !row.active) onpick?.(row.venue);
			}}
		>
			<span class="glyph" aria-hidden="true"><Icon icon={UTILITY_ICONS[row.icon]} size="lg" /></span
			>
			<span class="body">
				{#if row.page === undefined}
					<span class="title">{row.title}</span>
					<span class="line">{row.body}</span>
				{:else}
					<span class="title"
						>{row.page.name}{#if row.page.hostShown}<span class="host">{row.page.host}</span
							>{/if}</span
					>
				{/if}
				{#if row.integrity !== undefined && row.blocked === undefined}
					<IntegrityLine line={row.integrity} />
				{/if}
				{#if row.blocked !== undefined}
					<span class="reason">{row.blocked}</span>
				{/if}
			</span>
			<span class="mark" aria-hidden="true">
				{#if row.active}<Icon icon={UTILITY_ICONS.check} size="md" />{/if}
			</span>
		</button>
	</li>
{/snippet}

<div class="venue" role="radiogroup" aria-label={venue.title}>
	<ul class="rows">
		{#each inVela as row (row.id)}
			{@render choice(row)}
		{/each}
	</ul>

	{#if pageGroup !== undefined}
		<div class="group">
			<span class="group-title">{pageGroup.title}</span>
			<span class="group-line">{pageGroup.body}</span>
		</div>
		<ul class="rows">
			{#each pages as row (row.id)}
				{@render choice(row)}
			{/each}
		</ul>
	{/if}

	{#if showDomain}<p class="domain">{venue.domainLine}</p>{/if}
</div>

<style>
	.venue {
		display: flex;
		flex-direction: column;
	}

	.rows {
		display: flex;
		flex-direction: column;
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.row {
		display: flex;
		align-items: flex-start;
		gap: var(--space-lg);
		width: 100%;
		padding-block: var(--space-xl);
		padding-inline: 0;
		border: none;
		border-bottom: var(--border-hairline) solid var(--color-border-base);
		background: none;
		font-family: var(--font-ui);
		color: var(--color-fg-base);
		text-align: start;
		cursor: pointer;
	}

	.row[aria-disabled='true'] {
		cursor: default;
	}

	.row.blocked .glyph,
	.row.blocked .title {
		opacity: var(--opacity-disabled);
	}

	.glyph {
		display: flex;
		flex-shrink: 0;
		color: var(--color-fg-muted);
	}

	.active .glyph {
		color: var(--color-fg-base);
	}

	.body {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: var(--space-sm);
		min-width: 0;
	}

	.title {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		column-gap: var(--space-md);
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-medium);
	}

	.active .title {
		font-weight: var(--weight-semibold);
	}

	.host {
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-regular);
		color: var(--color-fg-subtle);
		overflow-wrap: anywhere;
	}

	.line {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-subtle);
	}

	.reason {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
	}

	/* The chosen row: a check, as every picker in Settings marks one. */
	.mark {
		display: flex;
		flex-shrink: 0;
		width: var(--icon-md);
		color: var(--color-accent-base);
	}

	.group {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		padding-block: var(--space-3xl) var(--space-sm);
	}

	.group-title {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		letter-spacing: var(--letterSpacing-sectionLabel);
		color: var(--color-fg-subtle);
	}

	.group-line {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-subtle);
	}

	.domain {
		margin: 0;
		padding-block: var(--space-xl) 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}
</style>
