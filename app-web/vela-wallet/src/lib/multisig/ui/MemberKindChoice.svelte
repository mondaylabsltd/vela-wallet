<script lang="ts">
	/**
	 * r7: a member's kind — "Vela account" or "Other wallet" — where a member
	 * is added (Create, the Safe{Wallet} import, Settings → Add member). Shown
	 * as found when evidence says what it is (a Vela account the index knows,
	 * or a smart contract account — code and no index record — which approves
	 * from its own app, said honestly); else the two choices. "Other wallet"
	 * is drawn as a hint, not picked, when no Vela account is found (kept only
	 * if the person picks it); nothing is picked when the wallet can't tell.
	 * A label that steers what the member is offered, never authority
	 * (`member-kind.ts`).
	 */
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import type { MultisigMessages } from '../messages';
	import type { KindChoice, MemberKind } from '../core/member-kind';

	interface Props {
		m: MultisigMessages;
		/** What it shows (`kindChoice`). */
		choice: KindChoice;
		/** Whose kind it is: names the group for assistive tech. */
		name?: string;
		onpick: (kind: MemberKind) => void;
	}

	let { m, choice, name, onpick }: Props = $props();

	const mm = $derived(m.members);
	const options = $derived([
		['passkey', mm.kindVela],
		['wallet', mm.kindWallet]
	] as const);
</script>

<div class="kind" data-testid="member-kind" data-kind={choice.kind ?? 'unknown'}>
	{#if choice.locked}
		<p class="known">
			{#if choice.kind === 'passkey'}<Icon icon={UTILITY_ICONS.check} size="sm" />{/if}<span
				>{choice.kind === 'contract' ? mm.kindContract : mm.kindVela}</span
			>
		</p>
		<p class="note" data-testid="member-kind-{choice.note}">
			{choice.note === 'contract' ? mm.kindContractNote : mm.kindKnown}
		</p>
	{:else}
		<div class="kinds" role="radiogroup" aria-label={name ? `${mm.kind} · ${name}` : mm.kind}>
			{#each options as [value, label] (value)}
				<button
					type="button"
					role="radio"
					aria-checked={choice.kind === value}
					class:on={choice.kind === value}
					class:hint={choice.suggested === value}
					data-suggested={choice.suggested === value ? '' : undefined}
					onclick={() => onpick(value)}>{label}</button
				>
			{/each}
		</div>
		{#if choice.note === 'likely'}
			<p class="note" data-testid="member-kind-likely">{mm.kindLikely}</p>
		{:else if choice.note === 'ask'}
			<p class="note" data-testid="member-kind-ask">{mm.kindAsk}</p>
		{/if}
	{/if}
</div>

<style>
	.kind {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	.known {
		display: flex;
		align-items: center;
		gap: var(--space-xs);
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.note {
		margin: 0;
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.kinds {
		display: flex;
		gap: var(--space-xs);
	}

	.kinds button {
		flex: 1;
		min-height: var(--size-control-md);
		padding-inline: var(--space-sm);
		border: var(--border-hairline) solid var(--color-border-strong);
		border-radius: var(--radius-full);
		background: var(--color-bg-base);
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		cursor: pointer;
	}

	/* The wallet's hint, not a pick: a dashed edge, the label in full ink. */
	.kinds button.hint {
		border-style: dashed;
		border-color: var(--color-fg-muted);
		color: var(--color-fg-base);
	}

	.kinds button.on {
		border-style: solid;
		border-color: var(--color-fg-base);
		color: var(--color-fg-base);
		font-weight: var(--weight-semibold);
	}
</style>
