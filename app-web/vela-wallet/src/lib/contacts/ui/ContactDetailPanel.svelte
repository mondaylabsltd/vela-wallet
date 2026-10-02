<script lang="ts">
	import ActivityRow from '$lib/wallet/ui/ActivityRow.svelte';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import Identicon from '$lib/wallet/ui/Identicon.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import type { ContactDetailModel } from '../model';
	import AddressBlock from './AddressBlock.svelte';

	/**
	 * DC2 — the one new third-column content this feature ships. The column
	 * shell (width, open/close timing, Esc, ✕) is the reused spec-015
	 * ThirdPanel; this is only its body.
	 */
	interface Props {
		detail: ContactDetailModel;
		/**
		 * Edit (the pencil beside the name) and Delete (after the content).
		 * Absent in the gallery, where they are drawn.
		 */
		onedit?: () => void;
		ondelete?: () => void;
		/** The three pills, the address copy, the chips' `+`, 查看全部往来 (spec 028 US5). */
		onaction?: (id: 'send' | 'receive' | 'qr') => void;
		oncopy?: () => void;
		onaddgroup?: () => void;
		onactivityall?: () => void;
	}

	let { detail, onedit, ondelete, onaction, oncopy, onaddgroup, onactivityall }: Props = $props();

	/** The worded name action replaces the pencil: one Edit beside the name. */
	const nameAction = $derived(onedit !== undefined ? detail.nameAction : undefined);
</script>

<div class="panel">
	<div class="identity">
		<Identicon
			svg={detail.contact.identiconSvg}
			size="detail"
			label={detail.contact.name}
			address={detail.contact.addressFull}
		/>
		<div class="who">
			<!-- Edit sits beside what it edits (issue 334): a click opens this
			     panel, and the only Edit used to be at the panel's foot or in the
			     row's right-click menu. The phones' pencil is in the same place in
			     spirit — the header of the contact's own screen. -->
			<div class="name-row">
				<p class="name">{detail.contact.name}</p>
				{#if nameAction === undefined}
					<button
						type="button"
						class="edit"
						aria-label={detail.editLabel}
						title={detail.editLabel}
						onclick={onedit}
					>
						<Icon icon={UTILITY_ICONS.pencil} size="md" />
					</button>
				{/if}
			</div>
			{#if nameAction !== undefined}
				<!-- Nobody has named this contact: the way to do it sits where the
				     missing name is noticed, and says so in words (issue 191). -->
				<button type="button" class="name-action" onclick={onedit}>
					<Icon icon={UTILITY_ICONS.pencil} size="xs" />
					<span>{nameAction}</span>
				</button>
			{/if}
			<div class="chips">
				{#each detail.chips as chip (chip)}
					<span class="chip">{chip}</span>
				{/each}
				{#if onaddgroup !== undefined}
					<button type="button" class="chip add" onclick={onaddgroup}>
						<Icon icon={UTILITY_ICONS.plus} size="xs" />
						<span>{detail.addChipLabel}</span>
					</button>
				{/if}
			</div>
		</div>
	</div>

	<div class="actions">
		<button type="button" onclick={() => onaction?.('send')}>
			<Icon icon={UTILITY_ICONS['arrow-up-right']} size="base" />
			<span>{detail.actions.send}</span>
		</button>
		<button type="button" onclick={() => onaction?.('receive')}>
			<Icon icon={UTILITY_ICONS['arrow-down-left']} size="base" />
			<span>{detail.actions.receive}</span>
		</button>
		<button type="button" onclick={() => onaction?.('qr')}>
			<Icon icon={UTILITY_ICONS['qr-code']} size="base" />
			<span>{detail.actions.qr}</span>
		</button>
	</div>

	<hr />

	<AddressBlock address={detail.address} layout="desktop" {oncopy} />

	<hr />

	<section class="activity">
		<p class="section-label">{detail.activityTitle}</p>
		<ul>
			{#each detail.rows as row, i (i)}
				<li><ActivityRow {row} /></li>
			{/each}
		</ul>
		{#if detail.rows.length === 0 && detail.emptyActivity !== undefined}
			<p class="empty">{detail.emptyActivity}</p>
		{:else}
			<button type="button" class="link" onclick={onactivityall}>{detail.activityLink}</button>
		{/if}
	</section>

	<!-- Delete follows the content, one hairline below it (issue 310). It was
	     pinned to the panel's foot, a screen-tall gap away from the contact it
	     removes. -->
	<hr />
	<footer>
		<button type="button" class="delete" onclick={ondelete}>{detail.deleteLabel}</button>
	</footer>
</div>

<style>
	.panel {
		display: flex;
		flex-direction: column;
	}

	.identity {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
	}

	.who {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
		min-width: 0;
	}

	.name-row {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		min-width: 0;
	}

	.name {
		min-width: 0;
		margin: 0;
		overflow-wrap: anywhere;
		font-size: calc(var(--text-2xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	/* A plain icon button (DESIGN-LANGUAGE §7): no box, a control-md target, drawn
	   as the phones' header pencil is. A long name wraps; the pencil stays. */
	.edit {
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		width: var(--size-control-md);
		height: var(--size-control-md);
		padding: 0;
		border: none;
		border-radius: var(--radius-full);
		background: none;
		color: var(--color-fg-base);
		cursor: pointer;
	}

	.edit:hover {
		background: var(--color-bg-sunken);
	}

	.name-action {
		align-self: flex-start;
		display: inline-flex;
		align-items: center;
		gap: var(--space-sm);
		height: var(--icon-xl);
		padding-inline: var(--space-lg);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-full);
		background: var(--color-bg-raised);
		color: var(--color-fg-base);
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		cursor: pointer;
	}

	.name-action:hover {
		opacity: var(--opacity-hover);
	}

	.chips {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-sm);
	}

	.chip {
		display: inline-flex;
		align-items: center;
		height: var(--icon-xl);
		padding-inline: var(--space-lg);
		border-radius: var(--radius-full);
		background: var(--color-bg-raised);
		color: var(--color-fg-muted);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
	}

	.chip.add {
		gap: var(--space-xs);
		border: var(--border-hairline) dashed var(--color-border-base);
		background: none;
		font-family: var(--font-ui);
		cursor: pointer;
	}

	.empty {
		margin: var(--space-md) 0 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.actions {
		display: flex;
		gap: var(--space-md);
		padding-block: var(--space-2xl) 0;
	}

	.actions button {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-md);
		height: var(--size-control-md);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
		color: var(--color-fg-base);
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		cursor: pointer;
	}

	.actions button:hover {
		opacity: var(--opacity-hover);
	}

	hr {
		width: 100%;
		border: none;
		border-top: var(--border-hairline) solid var(--color-border-base);
		margin-block: var(--space-2xl);
	}

	.section-label {
		margin: 0 0 var(--space-md);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	.link {
		margin-top: var(--space-2xl);
		padding: 0;
		border: none;
		background: none;
		color: var(--color-fg-muted);
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		cursor: pointer;
	}

	.link:hover {
		color: var(--color-fg-base);
	}

	footer {
		display: flex;
	}

	/* Secondary and set apart by the hairline, but next to the content. */
	.delete {
		display: inline-flex;
		align-items: center;
		min-height: var(--size-control-md);
		padding: 0;
		border: none;
		background: none;
		color: var(--color-error-base);
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		cursor: pointer;
	}

	.delete:hover {
		opacity: var(--opacity-hover);
	}
</style>
