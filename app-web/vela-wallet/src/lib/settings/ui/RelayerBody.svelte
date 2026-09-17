<script lang="ts">
	/**
	 * SR4 — this chain's bundler treasury is out of gas.
	 *
	 * Two different situations wear the same symptom, and the sheet has to tell
	 * them apart (spec 060):
	 *
	 * - **A network Vela ships.** The operator runs that relayer and can refill
	 *   it; the useful act is telling them, and it stays fixed for everyone
	 *   after that. Funding it yourself would work too, but it is somebody
	 *   else's bill — so it lives behind a disclosure rather than as the first
	 *   thing a person sees.
	 * - **A network the person added** — a local devnet, a company chain. The
	 *   operator may have no way to hold gas there at all, so there is nobody
	 *   to report it to and funding it is simply what has to happen.
	 *
	 * In both cases the QR and the copyable address are the payload, and the
	 * amber note is the part that must not be missed: this gas is
	 * non-refundable and goes to the bundler operator, not to Vela and not to
	 * the person's own transaction. It sits between the address and the CTA for
	 * that reason.
	 */
	import type { RelayerModel } from '../model';
	import Button from '$lib/ui/Button.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import QRPlaceholder from '$lib/wallet/ui/QRPlaceholder.svelte';
	import QRCard from '$lib/flows/ui/QRCard.svelte';
	import Callout from './Callout.svelte';
	import ChainMark from './ChainMark.svelte';

	interface Props {
		panel: RelayerModel;
		onprimary?: () => void;
		oncopy?: () => void;
	}

	let { panel, onprimary, oncopy }: Props = $props();

	/**
	 * Where an out-of-gas relayer gets reported. A literal, like the feedback
	 * link beside it: the app must not ship a support destination nobody has
	 * confirmed, and one line moves it (to a Telegram group, say).
	 */
	const REPORT_URL = 'https://github.com/mondaylabsltd/vela-wallet/issues/new';

	/**
	 * Open by default exactly when funding is the only path there is. On a
	 * network Vela ships, a person opens this themselves or not at all — the
	 * sheet never puts a payment request in front of them unasked.
	 */
	let opened = $state(false);
	/** Open when the person asked, or when funding is the only path there is. */
	const selfFundOpen = $derived(opened || panel.report === undefined);
</script>

<div class="relayer">
	<p class="lead">{panel.lead}</p>

	<div class="identity">
		<ChainMark mark={panel.mark} />
		<span class="text">
			<span class="name">{panel.name}</span>
			<span class="hint">{panel.amountHint}</span>
		</span>
	</div>

	{#if panel.report}
		<a class="report" href={REPORT_URL} target="_blank" rel="noopener noreferrer">
			{panel.report.label}
		</a>
		<button
			type="button"
			class="disclosure"
			aria-expanded={selfFundOpen}
			onclick={() => (opened = !opened)}
		>
			{panel.report.selfFundLabel}
		</button>
	{/if}

	{#if selfFundOpen}
		{#if panel.code !== undefined}
			<!-- A live sheet encodes the treasury's real address (spec 028 Phase 8). -->
			<div class="qr"><QRCard label={panel.qrCaption} code={panel.code} /></div>
		{:else}
			<div class="qr"><QRPlaceholder caption={panel.qrCaption} /></div>
		{/if}

		<button type="button" class="address" onclick={oncopy}>
			<span>{panel.addressDisplay}</span>
			<Icon icon={UTILITY_ICONS.copy} size="sm" />
		</button>

		<Callout callout={panel.callout} />
	{/if}

	<Button variant="primary" shape="rounded" onclick={onprimary}>{panel.primary}</Button>
</div>

<style>
	.relayer {
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
		padding-block: var(--space-md) var(--space-xl);
	}

	.lead {
		margin: 0;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
	}

	.identity {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
	}

	.text {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	.name {
		font-size: calc(var(--text-xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
	}

	.hint {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.report {
		display: flex;
		align-items: center;
		justify-content: center;
		min-height: var(--size-control-md);
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		color: var(--color-fg-base);
		text-decoration: none;
	}

	.disclosure {
		align-self: center;
		padding: 0;
		border: none;
		background: none;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
		text-decoration: underline;
		cursor: pointer;
	}

	.qr {
		display: flex;
		justify-content: center;
	}

	.address {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-md);
		min-height: var(--size-control-md);
		border: none;
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
		font-family: var(--font-mono);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
		cursor: pointer;
	}
</style>
