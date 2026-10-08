<script lang="ts">
	/**
	 * SD2e — choosing who gets the money.
	 *
	 * Scan sits at the top, above the saved people. Most sends go to someone
	 * already in the book, but the ones that don't are the ones where a person
	 * is holding a phone in one hand and an address in the other — so the
	 * escape hatch is the first thing, not the last.
	 *
	 * A row answers with its ADDRESS (issue 467), never its place: the book
	 * re-sorts by favourite, recency and name while names resolve, so the
	 * row at a place when the list was drawn can be somebody else's by the
	 * time the tap is handled — and this is who gets the money.
	 */
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import ContactPickRow from '../ui/ContactPickRow.svelte';
	import SearchField from '../ui/SearchField.svelte';
	import type { ContactPickModel } from '../model';

	interface Props {
		model: ContactPickModel;
		onscan?: () => void;
		ongroup?: (index: number) => void;
		/** The person tapped, by the address the row was drawn for. */
		onselect?: (address: string) => void;
	}

	let { model, onscan, ongroup, onselect }: Props = $props();

	let query = $state('');

	const shown = $derived(
		query.trim() === ''
			? model.contacts
			: model.contacts.filter((contact) =>
					`${contact.name} ${contact.addressDisplay}`
						.toLowerCase()
						.includes(query.trim().toLowerCase())
				)
	);
</script>

<div class="pick">
	<SearchField placeholder={model.searchPlaceholder} bind:value={query} />

	<button type="button" class="scan" onclick={onscan}>
		<Icon icon={UTILITY_ICONS['qr-code']} size="md" />
		<span class="scan-label">{model.scanRow}</span>
		<Icon icon={UTILITY_ICONS['chevron-right']} size="sm" />
	</button>

	{#if model.groups.length > 0 && query.trim() === ''}
		<p class="section">{model.groupsTitle}</p>
		<ul>
			{#each model.groups as group, i (group.name)}
				<li>
					<button type="button" class="group" onclick={() => ongroup?.(i)}>
						<span class="swatch" aria-hidden="true">
							<span class="disc" style:background={group.colors[0]}></span>
							<span class="disc" style:background={group.colors[1]}></span>
						</span>
						<span class="group-name">{group.name}</span>
						<span class="count">{group.count}</span>
						<Icon icon={UTILITY_ICONS['chevron-right']} size="sm" />
					</button>
				</li>
			{/each}
		</ul>
	{/if}

	<p class="section">{model.contactsTitle}</p>
	<ul>
		<!-- Keyed by address: a row keeps its person when the book re-sorts. -->
		{#each shown as contact (contact.addressFull)}
			<li><ContactPickRow {contact} onselect={() => onselect?.(contact.addressFull)} /></li>
		{/each}
	</ul>
</div>

<style>
	.pick {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}

	.scan {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		padding: var(--space-lg);
		border: none;
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
		font-family: var(--font-ui);
		color: var(--color-fg-subtle);
		cursor: pointer;
	}

	.scan-label {
		flex: 1;
		min-width: 0;
		text-align: start;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-medium);
		color: var(--color-fg-base);
	}

	.section {
		margin: 0;
		padding-top: var(--space-md);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	.group {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		width: 100%;
		padding-block: var(--space-lg);
		padding-inline: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		color: var(--color-fg-subtle);
		cursor: pointer;
	}

	/* Two overlapping discs stand for "several people" without drawing any of
	   them — a group has no single face to show. */
	.swatch {
		display: inline-flex;
		flex-shrink: 0;
	}

	.disc {
		width: var(--icon-2xl);
		height: var(--icon-2xl);
		border-radius: var(--radius-full);
	}

	.disc + .disc {
		margin-inline-start: calc(var(--space-lg) * -1);
	}

	.group-name {
		flex: 1;
		min-width: 0;
		text-align: start;
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.count {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}
</style>
