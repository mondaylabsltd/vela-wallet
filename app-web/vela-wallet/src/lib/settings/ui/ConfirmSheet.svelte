<script lang="ts">
	/**
	 * ST3 / ST3b / ST13b / ST16 — the confirm sheet.
	 *
	 * Four mocks, one component: a title, a body, an optional quieter second
	 * paragraph, an optional callout, and two stacked buttons. The tone picks
	 * the CTA's colour, so "清除缓存" is accent and "全部清除" is red without
	 * either screen owning a button of its own.
	 *
	 * ob-2: a loss nothing brings back (`warning` — the erase's unsynced
	 * wallets, by name) is drawn first, and with `ack` the confirm waits for
	 * the person to tick that they understand it.
	 */
	import type { ConfirmSheetModel } from '../model';
	import Button from '$lib/ui/Button.svelte';
	import Callout from './Callout.svelte';

	interface Props {
		sheet: ConfirmSheetModel;
		onconfirm?: () => void;
		oncancel?: () => void;
	}

	let { sheet, onconfirm, oncancel }: Props = $props();

	let acknowledged = $state(false);
	const blocked = $derived(sheet.ack !== undefined && !acknowledged);
</script>

<div class="confirm">
	<p class="body">{sheet.body}</p>
	{#if sheet.warning !== undefined}
		<Callout callout={{ tone: 'danger', text: sheet.warning }} />
	{/if}
	{#if sheet.note !== undefined}
		<p class="note">{sheet.note}</p>
	{/if}
	{#if sheet.callout !== undefined}
		<Callout callout={sheet.callout} />
	{/if}
	{#if sheet.ack !== undefined}
		<label class="ack">
			<input type="checkbox" bind:checked={acknowledged} />
			<span>{sheet.ack}</span>
		</label>
	{/if}
	<div class="actions">
		<Button
			variant={sheet.tone === 'danger' ? 'danger' : 'primary'}
			shape="rounded"
			disabled={blocked}
			onclick={onconfirm}
		>
			{sheet.confirm}
		</Button>
		<Button variant="secondary" shape="rounded" onclick={oncancel}>{sheet.cancel}</Button>
	</div>
</div>

<style>
	.confirm {
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
		padding-block: var(--space-md) var(--space-xl);
	}

	.body {
		margin: 0;
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-base);
	}

	.note {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-subtle);
	}

	.ack {
		display: flex;
		align-items: flex-start;
		gap: var(--space-md);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-base);
		cursor: pointer;
	}

	.ack input {
		flex: none;
		width: var(--space-xl);
		height: var(--space-xl);
		margin: 0;
		/* A check is a selected state: neutral, never the accent. */
		accent-color: var(--color-fg-base);
	}

	.actions {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
	}
</style>
