<script lang="ts">
	/**
	 * What the core found about who can act for a multisig (§3.5), said
	 * plainly: `refused` — a genesis Vela will not deploy or sign for, and
	 * why; `risk` — a deployed one that something besides its members can act
	 * for, each module, guard, handler or singleton named. Never folded away.
	 * `readOnlyOn`: the networks where it is read-only for that (the re-check
	 * refused its genesis there), said first.
	 */
	import { fillTemplate } from '$lib/i18n/fill';
	import { chainName } from '$lib/services/networks';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import type { Finding } from '../core/refusal';
	import type { MultisigMessages } from '../messages';
	import { findingLines } from './findings';

	interface Props {
		m: MultisigMessages;
		kind: 'refused' | 'risk';
		findings: Finding[];
		readOnlyOn?: number[];
	}

	let { m, kind, findings, readOnlyOn = [] }: Props = $props();

	const lines = $derived([
		...readOnlyOn.map((id) => fillTemplate(m.authority.refusedOn, { network: chainName(id) })),
		...findingLines(m, findings)
	]);
</script>

<div class="notice" role="alert" data-testid="authority-{kind}">
	<p class="title">
		<span class="glyph"><Icon icon={UTILITY_ICONS['triangle-alert']} size="md" /></span>
		{kind === 'refused' ? m.authority.refusedTitle : m.authority.riskTitle}
	</p>
	<p>{kind === 'refused' ? m.authority.refusedBody : m.authority.riskBody}</p>
	<ul>
		{#each lines as line (line)}
			<li class="mono-break">{line}</li>
		{/each}
	</ul>
</div>

<style>
	.notice {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
		padding: var(--space-lg);
		border-radius: var(--radius-lg);
		border: var(--border-hairline) solid
			color-mix(in srgb, var(--color-error-base) 30%, transparent);
		background: var(--color-error-soft);
		color: var(--color-error-base);
	}

	p {
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
	}

	.title {
		display: flex;
		align-items: flex-start;
		gap: var(--space-sm);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
	}

	.glyph {
		display: flex;
		flex-shrink: 0;
	}

	ul {
		margin: 0;
		padding-inline-start: var(--space-lg);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
	}

	/* An address in a line may not overflow a phone's width. */
	.mono-break {
		overflow-wrap: anywhere;
	}
</style>
