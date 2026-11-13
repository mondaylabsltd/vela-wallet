<script lang="ts">
	/**
	 * The confirmation every signature on a high-risk network asks (§3.5),
	 * before its passkey: what was found there — each module, guard, handler
	 * or singleton, by its address — and the person's explicit "sign". Asked
	 * again for the next signature; nothing is remembered. Closing it signs
	 * nothing.
	 */
	import Button from '$lib/ui/Button.svelte';
	import { fillTemplate } from '$lib/i18n/fill';
	import { chainName } from '$lib/services/networks';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import type { RiskRequest } from '../core/gate.svelte';
	import type { MultisigMessages } from '../messages';
	import { findingLines } from './findings';
	import Sheet from './Sheet.svelte';

	interface Props {
		m: MultisigMessages;
		request: RiskRequest;
		wide: boolean;
		closeLabel: string;
	}

	let { m, request, wide, closeLabel }: Props = $props();

	const lines = $derived(findingLines(m, request.findings));
</script>

<Sheet title={m.authority.signRiskTitle} {closeLabel} {wide} onclose={() => request.answer(false)}>
	<div class="risk" role="alert" data-testid="risk-confirm">
		<p class="title">
			<span class="glyph"><Icon icon={UTILITY_ICONS['triangle-alert']} size="md" /></span>
			{fillTemplate(m.authority.signRiskBody, { network: chainName(request.chainId) })}
		</p>
		<ul>
			{#each lines as line (line)}
				<li class="mono-break">{line}</li>
			{/each}
		</ul>
	</div>
	<div class="actions">
		<Button variant="danger" shape="rounded" onclick={() => request.answer(true)}
			>{m.authority.signRiskConfirm}</Button
		>
		<Button variant="secondary" shape="rounded" onclick={() => request.answer(false)}
			>{m.authority.signRiskCancel}</Button
		>
	</div>
</Sheet>

<style>
	.risk {
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

	.title {
		display: flex;
		align-items: flex-start;
		gap: var(--space-sm);
		margin: 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
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

	.actions {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
		margin-top: var(--space-lg);
	}
</style>
