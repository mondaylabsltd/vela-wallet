<script lang="ts">
	/**
	 * r4a-1 / r7-11: what a transaction changes about who controls a
	 * multisig, drawn the one way every signer reads it: who it adds,
	 * removes or replaces (each address whole, in its EIP-55 case), the
	 * rule it meets and the one it leaves ("Approvals: 2 of 3 → 2 of 4"),
	 * the network, the caution, and that the Safe would refuse it there
	 * when it does not fit. The reading is the core one (`MemberChanges`,
	 * the review `memberChanges`); this only puts its words on it.
	 */
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import Identicon from '$lib/wallet/ui/Identicon.svelte';
	import { identiconSvgForClient } from '$lib/wallet/identicon';
	import type { MultisigMessages } from '../messages';
	import { changeLines, memberWho } from '../core/display';
	import type { MemberChanges } from '../core/kernel';
	import { eip55 } from '../external/eip55';

	interface Props {
		m: MultisigMessages;
		/** The core reading: each change, the rule before and after, whether it fits. */
		changes: MemberChanges;
		/** The network it runs on, by name. */
		network: string;
		/** How a member is named: the name known for them and the short address. */
		who?: (address: string) => string;
		/** Its number is used: what it did, without the caution or the rule. */
		done?: boolean;
	}

	let { m, changes, network, who = (a: string) => memberWho(a), done = false }: Props = $props();

	const lines = $derived(changeLines(m, changes, network, who));
</script>

<section class="change" data-testid="member-change">
	{#if !done}
		<p class="change-caution" role="alert">
			<Icon icon={UTILITY_ICONS['triangle-alert']} size="sm" />
			<span>{lines.caution}</span>
		</p>
	{/if}
	<ul class="change-list">
		{#each changes.changes as c, i (i)}
			<li>
				<span class="change-what">{lines.changes[i]}</span>
				{#each [c.removed, c.added].filter((a) => a !== null) as address, j (j)}
					<span class="change-who">
						<Identicon
							svg={identiconSvgForClient(address ?? '')}
							size="compact"
							address={address ?? ''}
						/>
						<code class="change-address">{eip55(address ?? '')}</code>
					</span>
				{/each}
			</li>
		{/each}
	</ul>
	{#if !done}
		<p class="change-rule" data-testid="member-change-rule">{lines.rule}</p>
		{#if lines.doesNotFit}
			<p class="warn" role="alert" data-testid="member-change-misfit">{lines.doesNotFit}</p>
		{/if}
	{/if}
</section>

<style>
	/* r4a-1: a member change, read before anything else. */
	.change {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
		margin: var(--space-md) 0 var(--space-lg);
		padding: var(--space-md);
		border-radius: var(--radius-lg);
		background: var(--color-error-soft);
	}

	.change-caution {
		display: flex;
		gap: var(--space-sm);
		align-items: flex-start;
		margin: 0;
		font-weight: var(--weight-semibold);
		color: var(--color-error-base);
	}

	.change-list {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.change-list li {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	.change-what {
		font-weight: var(--weight-semibold);
		color: var(--color-fg-base);
	}

	.change-who {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		min-width: 0;
	}

	.change-address {
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		overflow-wrap: anywhere;
	}

	.change-rule {
		margin: 0;
		font-variant-numeric: tabular-nums;
		color: var(--color-fg-base);
	}

	.warn {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-error-base);
	}
</style>
