<script lang="ts">
	/**
	 * SD4a / SD4b / SD4c / DSD4L — the receipt, in whichever state it is in.
	 *
	 * The SPEC sheet calls these three "三态" and it means it: submitting,
	 * submitted, confirmed. One screen that changes, not three that replace
	 * each other — which is why the disc, the title and the button all keep
	 * their positions and only their contents move.
	 *
	 * "Close · keep running" is load-bearing copy. The transaction does not
	 * depend on this screen staying open, and a person who thinks it does will
	 * sit here watching a spinner.
	 */
	import Button from '$lib/ui/Button.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import Breakdown from '../ui/Breakdown.svelte';
	import StatusHero from '../ui/StatusHero.svelte';
	import type { SendReceiptModel } from '../model';

	interface Props {
		model: SendReceiptModel;
		onexplorer?: () => void;
		oncta?: () => void;
	}

	let { model, onexplorer, oncta }: Props = $props();

	let copied = $state(false);
	let timer: ReturnType<typeof setTimeout> | undefined;

	function copy() {
		copied = true;
		clearTimeout(timer);
		timer = setTimeout(() => (copied = false), 150);
	}

	// Spec 038 #D3: while the relay has the op, the screen counts. One second
	// is the right grain — a person reads "12s", not a spinner. The sentences
	// arrive in the model; only the number is this screen's.
	let now = $state(Date.now());
	$effect(() => {
		if (!model.eta) return;
		const timer = setInterval(() => (now = Date.now()), 1000);
		return () => clearInterval(timer);
	});
	const elapsedS = $derived(
		model.eta ? Math.max(0, Math.floor((now - model.eta.submittedAtMs) / 1000)) : 0
	);
	const etaLines = $derived.by(() => {
		if (!model.eta) return [] as string[];
		const { typicalS, typicalLine, elapsedTemplate, slowLine } = model.eta;
		return [
			typicalLine,
			elapsedS >= typicalS * 2 ? slowLine : elapsedTemplate.replace('{{elapsed}}', String(elapsedS))
		];
	});
</script>

<div class="receipt">
	<StatusHero stage={model.stage} title={model.title} captions={[...model.captions, ...etaLines]} />

	{#if model.breakdown !== undefined}
		<div class="parts"><Breakdown rows={model.breakdown} title={model.breakdownTitle} /></div>
	{/if}

	<div class="foot">
		{#if model.hash !== undefined}
			<p class="hash">
				<span class="hash-label">{model.hash.label}</span>
				<span class="hash-value">{model.hash.value}</span>
				<button type="button" aria-label={model.hash.copyLabel} class:copied onclick={copy}>
					<Icon icon={copied ? UTILITY_ICONS.check : UTILITY_ICONS.copy} size="sm" />
				</button>
			</p>
		{/if}

		{#if model.viewOnExplorer !== undefined}
			<Button variant="secondary" onclick={onexplorer}>{model.viewOnExplorer}</Button>
		{/if}

		<Button
			variant={model.ctaAccent ? 'primary' : 'secondary'}
			shape={model.ctaAccent ? 'rounded' : 'pill'}
			onclick={oncta}
		>
			{model.cta}
		</Button>
	</div>
</div>

<style>
	.receipt {
		display: flex;
		flex-direction: column;
		flex: 1;
		min-height: 100%;
	}

	.parts {
		padding-top: var(--space-lg);
	}

	/* The buttons live at the bottom of the screen while the status sits near
	   the top: the gap between them is where the waiting happens, and filling
	   it would make the screen look busier than the moment is. */
	.foot {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
		margin-top: auto;
		padding-block: var(--space-3xl) var(--space-xl);
	}

	.hash {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-sm);
		margin: 0;
		padding-bottom: var(--space-md);
	}

	.hash-label {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.hash-value {
		font-family: var(--font-mono);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	.hash button {
		display: flex;
		align-items: center;
		border: none;
		background: none;
		color: var(--color-fg-subtle);
		cursor: pointer;
	}

	.hash .copied {
		color: var(--color-success-base);
	}
</style>
