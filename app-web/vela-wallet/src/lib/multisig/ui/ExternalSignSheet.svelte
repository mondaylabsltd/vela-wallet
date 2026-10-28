<script lang="ts">
	/**
	 * An outside owner signs: Vela opens a room and shows the connect page's
	 * link and code; the owner opens it where their wallet is (a browser with
	 * MetaMask or Rabby, or a wallet app's browser), connects, and confirms
	 * the SafeTx in their wallet. The answer is checked by the core before it
	 * counts. A wallet paired before only needs the page reopened.
	 *
	 * The wallet is asked for a SafeTx — typed data that says nothing a person
	 * can read about a call to another contract. So a call is drawn here
	 * first, as the signing sheet reads it (cs-12): this sheet is all the
	 * proposer of a dApp's call sees when no member is on this device.
	 */
	import { onMount } from 'svelte';
	import Button from '$lib/ui/Button.svelte';
	import { fillTemplate } from '$lib/i18n/fill';
	import { chainName } from '$lib/services/networks';
	import { encodeQr } from '$lib/wallet/qr';
	import { shortenAddress } from '$lib/wallet/identity';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import type { SigningMessages } from '$lib/signing/messages';
	import type { MultisigMessages } from '../messages';
	import { CallReading } from '../core/call-review.svelte';
	import { reviewTx } from '../core/kernel';
	import { ExternalWallet, WalletDeclinedError } from '../external/session.svelte';
	import { checked, typedDataOf, type ExternalRequest } from '../external/signing.svelte';
	import { shareOrigin } from '../core/link';
	import CallReview from './CallReview.svelte';
	import Sheet from './Sheet.svelte';

	interface Props {
		m: MultisigMessages;
		/** The signing sheet's words, for a call's review. */
		signing: SigningMessages;
		locale: string;
		request: ExternalRequest;
		wide: boolean;
		closeLabel: string;
	}

	let { m, signing, locale, request, wide, closeLabel }: Props = $props();
	const w = $derived(m.external);

	let wallet = $state<ExternalWallet | null>(null);
	let failure = $state<string | null>(null);
	let copied = $state(false);
	let controller = new AbortController();

	const qr = $derived.by(() => {
		if (!wallet || wallet.resumed) return null;
		try {
			return encodeQr(wallet.link);
		} catch {
			return null;
		}
	});

	const status = $derived.by(() => {
		if (!wallet) return '';
		switch (wallet.phase) {
			case 'waiting':
				return w.waiting;
			case 'connecting':
				return w.connecting;
			case 'wrongAccount':
				return fillTemplate(w.wrongAccount, {
					account: wallet.accounts[0] ? shortenAddress(wallet.accounts[0]) : '—'
				});
			case 'switching':
				return fillTemplate(w.switching, { chain: chainName(request.chainId) });
			case 'confirming':
				return w.confirming;
			default:
				return '';
		}
	});

	/** The call this SafeTx makes, when it is one to another contract (the core's reading). */
	let reading = $state<CallReading | null>(null);

	onMount(() => {
		let action = null;
		try {
			action = reviewTx(request.chainId, request.safe, request.tx).action;
		} catch {
			// Nothing the core can read: the wallet's own prompt is all there is.
		}
		if (action?.kind === 'call') {
			reading = new CallReading(request.chainId, request.safe, action);
			void reading.start();
		}
		// The connect page opens where the owner's wallet is: the wallet's web
		// address, never this extension's own.
		wallet = ExternalWallet.for(
			request.owners,
			`${shareOrigin(location.origin)}/${locale}/connect`
		);
		void run();
		return () => {
			controller.abort();
			wallet?.leave();
			reading?.dispose();
		};
	});

	async function run() {
		if (!wallet) return;
		failure = null;
		try {
			const answer = await wallet.signTypedData(
				request.chainId,
				request.owners,
				typedDataOf(request),
				controller.signal
			);
			const approval = checked(request.safeTxHash, request.owners, answer);
			if (!approval) {
				failure = w.badSignature;
				return;
			}
			wallet.phase = 'done';
			wallet.leave();
			request.resolve(approval);
		} catch (error) {
			if (error instanceof DOMException && error.name === 'AbortError') return;
			failure =
				error instanceof WalletDeclinedError ? w.declined : String((error as Error).message);
		}
	}

	function retry() {
		controller = new AbortController();
		void run();
	}

	function close() {
		controller.abort();
		wallet?.leave();
		request.reject(new DOMException('Cancelled', 'AbortError'));
	}

	async function copy() {
		if (!wallet) return;
		try {
			await navigator.clipboard.writeText(wallet.link);
			copied = true;
			setTimeout(() => (copied = false), 1600);
		} catch {
			// the link stays on screen
		}
	}
</script>

<Sheet title={w.sheetTitle} {closeLabel} {wide} onclose={close}>
	{#if reading}
		<CallReview {reading} m={signing} />
	{/if}
	{#if wallet}
		{#if wallet.resumed}
			<p class="body">{fillTemplate(w.resumeBody, { wallet: wallet.wallet ?? w.ownerWallet })}</p>
		{:else}
			<p class="body">{w.scanBody}</p>
			{#if qr}
				<svg
					class="qr"
					viewBox="-2 -2 {qr.modules + 4} {qr.modules + 4}"
					role="img"
					aria-label={w.sheetTitle}
				>
					<rect x="-2" y="-2" width={qr.modules + 4} height={qr.modules + 4} class="qr-bg" />
					<path d={qr.path} class="qr-fg" />
				</svg>
			{/if}
		{/if}
		<code class="link" data-testid="external-link">{wallet.link}</code>
		<Button variant="secondary" shape="rounded" onclick={copy}>
			<span class="label"
				><Icon icon={copied ? UTILITY_ICONS.check : UTILITY_ICONS.copy} size="sm" />{copied
					? w.copied
					: w.copyLink}</span
			>
		</Button>
		{#if failure}
			<p class="error" role="alert">{failure}</p>
			<Button variant="primary" shape="rounded" onclick={retry}>{w.retry}</Button>
		{:else if status}
			<p class="status" aria-live="polite" data-testid="external-status">
				<span class="dot" class:busy={wallet.phase !== 'wrongAccount'}></span>{status}
			</p>
		{/if}
	{/if}
</Sheet>

<style>
	.body {
		margin: 0 0 var(--space-lg);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.qr {
		display: block;
		width: min(var(--size-qrCard), 70%);
		margin: 0 auto var(--space-lg);
	}

	.qr-bg {
		fill: var(--color-bg-base);
	}

	.qr-fg {
		fill: var(--color-fg-base);
	}

	.link {
		display: block;
		margin-block-end: var(--space-md);
		padding: var(--space-sm) var(--space-md);
		border-radius: var(--radius-md);
		background: var(--color-bg-sunken);
		font-family: var(--font-mono);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		overflow-wrap: anywhere;
	}

	.label {
		display: inline-flex;
		align-items: center;
		gap: var(--space-sm);
	}

	.status {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		margin: var(--space-lg) 0 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	.dot {
		width: var(--space-sm);
		height: var(--space-sm);
		border-radius: 50%;
		background: var(--color-fg-subtle);
	}

	.dot.busy {
		animation: pulse 1.2s ease-in-out infinite;
	}

	@keyframes pulse {
		50% {
			opacity: 0.3;
		}
	}

	.error {
		margin: var(--space-lg) 0 var(--space-md);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-error-base);
	}
</style>
