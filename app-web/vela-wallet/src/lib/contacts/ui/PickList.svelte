<script lang="ts">
	/**
	 * A tick-list with a save (spec 028 US5): 添加成员 lists the book with the
	 * group's members ticked; 移入分组 lists the groups with the contact's
	 * ticked. The container is the caller's — a bottom sheet on the phone, a
	 * centred dialog on the desktop — and what a tick MEANS is the core's:
	 * this component only reports the ids left ticked when 保存 is pressed.
	 */
	import { SvelteSet } from 'svelte/reactivity';
	import Button from '$lib/ui/Button.svelte';
	import SearchField from '$lib/flows/ui/SearchField.svelte';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import Identicon from '$lib/wallet/ui/Identicon.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import type { PickListModel } from '../live';

	interface Props {
		model: PickListModel;
		onsave: (ids: string[]) => void;
	}

	let { model, onsave }: Props = $props();

	// Seeded once per open (the caller keys the component); edits stay local
	// until saved, so a dismissed sheet changes nothing.
	// svelte-ignore state_referenced_locally
	const checked = new SvelteSet(model.rows.filter((row) => row.checked).map((row) => row.id));
	let query = $state('');

	const shown = $derived.by(() => {
		const q = query.trim().toLowerCase();
		if (q === '') return model.rows;
		return model.rows.filter((row) => `${row.name} ${row.detail ?? ''}`.toLowerCase().includes(q));
	});

	function toggle(id: string) {
		if (checked.has(id)) checked.delete(id);
		else checked.add(id);
	}
</script>

<div class="pick">
	{#if model.rows.length > 6}
		<SearchField placeholder={model.searchPlaceholder} value={query} oninput={(v) => (query = v)} />
	{/if}

	{#if model.rows.length === 0}
		<p class="empty">{model.empty}</p>
	{:else}
		<ul role="listbox" aria-multiselectable="true" aria-label={model.title}>
			{#each shown as row (row.id)}
				<li>
					<button
						type="button"
						role="option"
						aria-selected={checked.has(row.id)}
						class="row"
						onclick={() => toggle(row.id)}
					>
						{#if row.identiconSvg !== undefined}
							<Identicon svg={row.identiconSvg} size="row" label={row.name} />
						{:else}
							<span class="tile" aria-hidden="true">
								<Icon icon={UTILITY_ICONS['users-round']} size="md" />
							</span>
						{/if}
						<span class="text">
							<span class="name">{row.name}</span>
							{#if row.detail !== undefined}
								<span class="detail">{row.detail}</span>
							{/if}
						</span>
						<span class="box" class:on={checked.has(row.id)} aria-hidden="true">
							{#if checked.has(row.id)}
								<Icon icon={UTILITY_ICONS.check} size="sm" />
							{/if}
						</span>
					</button>
				</li>
			{/each}
		</ul>
	{/if}

	<div class="actions">
		<Button variant="primary" shape="rounded" onclick={() => onsave([...checked])}>
			{model.save}
		</Button>
	</div>
</div>

<style>
	.pick {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
		padding-block-end: var(--space-xl);
	}

	ul {
		list-style: none;
		margin: 0;
		padding: 0;
		max-height: 50dvh;
		overflow-y: auto;
	}

	.row {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		width: 100%;
		padding: var(--space-md) var(--space-sm);
		border: none;
		border-bottom: var(--border-hairline) solid var(--color-border-base);
		background: none;
		color: var(--color-fg-base);
		font-family: var(--font-ui);
		text-align: start;
		cursor: pointer;
	}

	.row:hover {
		background: var(--color-bg-raised);
	}

	.tile {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: var(--icon-2xl);
		height: var(--icon-2xl);
		border-radius: var(--radius-full);
		background: var(--color-bg-raised);
		color: var(--color-fg-muted);
	}

	.text {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	.name {
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.detail {
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.box {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: var(--icon-lg);
		height: var(--icon-lg);
		border: var(--border-hairline) solid var(--color-border-strong, var(--color-border-base));
		border-radius: var(--radius-sm);
		color: var(--color-onAccent);
	}

	.box.on {
		border-color: var(--color-accent-base);
		background: var(--color-accent-base);
	}

	.empty {
		margin: var(--space-2xl) 0;
		text-align: center;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.actions {
		margin-block-start: var(--space-md);
	}
</style>
