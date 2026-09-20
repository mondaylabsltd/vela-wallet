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
	import { copyText } from '$lib/services/clipboard';
	import { shortenAddress } from '$lib/wallet/identity';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import Breakdown from '../ui/Breakdown.svelte';
	import StatusHero from '../ui/StatusHero.svelte';
	import type { SendReceiptModel } from '../model';

	interface Props {
		model: SendReceiptModel;
		/**
		 * Issue 199. A phone screen is short and the button belongs under the
		 * thumb, so `screen` pins the foot to the bottom. The desktop's third
		 * column is as tall as the window: pinned there, the status and the
		 * button end up 900px apart with nothing between them, so `column`
		 * keeps them one group and sets the group a little above the middle.
		 */
		layout?: 'screen' | 'column';
		onexplorer?: () => void;
		oncta?: () => void;
	}

	let { model, layout = 'screen', onexplorer, oncta }: Props = $props();

	let copied = $state(false);
	let timer: ReturnType<typeof setTimeout> | undefined;

	// The whole hash goes to the clipboard; the row shows its two ends (spec
	// 038 #D4 — 66 characters on one line ran off both edges of the column,
	// and the button beside them copied nothing).
	function copy() {
		if (model.hash) void copyText(model.hash.value);
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
	// Inside the typical time the line counts DOWN — "~9s remaining" is a
	// promise with an end, "6s elapsed" is a stopwatch. "Almost there" waits
	// until the typical time has passed, which is when it is true.
	const etaLines = $derived.by(() => {
		if (!model.eta) return [] as string[];
		const { typicalS, typicalLine, remainingTemplate, elapsedTemplate, slowLine } = model.eta;
		const second =
			elapsedS < typicalS
				? remainingTemplate.replace('{{remaining}}', String(typicalS - elapsedS))
				: elapsedS < typicalS * 2
					? elapsedTemplate.replace('{{elapsed}}', String(elapsedS))
					: slowLine;
		return [typicalLine, second];
	});

	// The ring round the disc. It eases toward full and never gets there:
	// about 70% at the typical time, 86% at twice it, and a ceiling of 92%
	// after that, so a transaction that takes three minutes is still visibly
	// moving and one that takes ten seconds does not sit at 100% waiting.
	// Only the confirmation closes it.
	const progress = $derived.by(() => {
		if (model.stage === 'confirmed') return 1;
		if (model.stage !== 'submitted' || !model.eta) return undefined;
		return 0.92 * (1 - Math.exp((-1.4 * elapsedS) / Math.max(1, model.eta.typicalS)));
	});
</script>

<div class="receipt {layout}">
	<StatusHero
		stage={model.stage}
		title={model.title}
		captions={[...model.captions, ...etaLines]}
		{progress}
	/>

	{#if model.breakdown !== undefined}
		<div class="parts"><Breakdown rows={model.breakdown} title={model.breakdownTitle} /></div>
	{/if}

	<div class="foot">
		{#if model.hash !== undefined}
			<p class="hash">
				<span class="hash-label">{model.hash.label}</span>
				<span class="hash-value" title={model.hash.value}>{shortenAddress(model.hash.value)}</span>
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

	/* On a phone the buttons live at the bottom of the screen while the
	   status sits near the top: the gap between them is where the waiting
	   happens, and filling it would make the screen look busier than the
	   moment is. */
	.foot {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
		margin-top: auto;
		padding-block: var(--space-3xl) var(--space-xl);
	}

	/* Issue 199: in a window-tall column that same gap is most of the screen.
	   One group, 2:3 above the middle — the optical centre, where a dialog
	   would sit. When the content outgrows the column (a long split) the
	   spacers collapse to nothing and it scrolls as before. */
	.column::before,
	.column::after {
		content: '';
	}

	.column::before {
		flex: 2 1 0;
	}

	.column::after {
		flex: 3 1 0;
	}

	.column .foot {
		margin-top: 0;
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
