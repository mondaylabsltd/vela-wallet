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
	 *
	 * r4C-6: the pairing is shown and the person's to change — which wallet
	 * and account it is, another wallet (a new room, a fresh code), Unpair —
	 * and the code is always there. r4C-4: Vela's own way onto the relay is
	 * said ("Vela can't reach the signing relay — retrying in N s"), never as
	 * the wallet page not being there. r4C-3: a wallet that does not answer
	 * is timed, then said to be waiting, maybe in another window, with Cancel
	 * and Try again. r4C-11: a wallet's refusal in words, with what to do.
	 * r4C-2: the link names the transaction, so the owner can approve from it
	 * with this sheet closed; while it is open, an approval that reaches the
	 * mailbox that way ends it here too.
	 */
	import { onMount, untrack } from 'svelte';
	import Button from '$lib/ui/Button.svelte';
	import { fillTemplate } from '$lib/i18n/fill';
	import { chainName } from '$lib/services/networks';
	import { encodeQr } from '$lib/wallet/qr';
	import { shortenAddress } from '$lib/wallet/identity';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { CallError } from '$lib/walletpair/client';
	import type { SigningMessages } from '$lib/signing/messages';
	import type { MultisigMessages } from '../messages';
	import { CallReading, readableCalls } from '../core/call-review.svelte';
	import { memberChanges } from '../core/display';
	import { isEcdsaApproval, reviewTx, sameAddress } from '../core/kernel';
	import { fetchProposals } from '../core/service';
	import {
		ExternalWallet,
		WalletDeclinedError,
		WalletOtherChainError
	} from '../external/session.svelte';
	import { checked, typedDataOf, type ExternalRequest } from '../external/signing.svelte';
	import { problemText, walletProblem } from '../external/errors';
	import { eip55 } from '../external/eip55';
	import type { RelayStatus } from '../external/relay';
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

	let { m, signing, locale, request: asked, wide, closeLabel }: Props = $props();
	const w = $derived(m.external);
	/**
	 * r5reg-11: the request this sheet was opened for, kept. The host opens
	 * one sheet per request (`{#key}`), and once it is answered or closed the
	 * host's request is gone (`null`) while a wallet's answer may still be on
	 * its way here — the room left by "Use another wallet", or by closing, on
	 * a relay that does not answer. Read live, that answer read `null`:
	 * "Cannot read properties of null (reading 'chainId')".
	 */
	const request = untrack(() => asked);

	/** r4C-3: a wallet still silent after this long is said to be waiting, maybe in another window. */
	const STILL_WAITING_MS = 20_000;
	/** r4C-2: how often the mailbox is looked at for an approval made from the link alone. */
	const MAILBOX_EVERY_MS = 8_000;

	let wallet = $state<ExternalWallet | null>(null);
	let failure = $state<string | null>(null);
	let copied = $state(false);
	let controller = new AbortController();
	let now = $state(Date.now());

	const page = $derived(
		typeof location === 'undefined' ? '' : `${shareOrigin(location.origin)}/${locale}/connect`
	);
	const tx = $derived({
		chainId: request.chainId,
		multisig: request.safe,
		safeTxHash: request.safeTxHash
	});

	// r4C-6: the code is always there — a paired room's link reopens the page.
	const qr = $derived.by(() => {
		if (!wallet) return null;
		try {
			return encodeQr(wallet.link);
		} catch {
			return null;
		}
	});

	const walletLabel = $derived(wallet?.wallet ?? w.ownerWallet);

	function elapsed(since: number): string {
		const s = Math.max(0, Math.floor((now - since) / 1000));
		return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
	}

	/** r4C-4: Vela's own side not on the relay, in words. */
	function relayText(status: RelayStatus): string {
		if (status.kind !== 'retrying' && status.kind !== 'failed') return '';
		const reason =
			status.why === 'refused'
				? w.relayRefused
				: status.why === 'noToken'
					? w.relayNoToken
					: status.detail;
		if (status.kind === 'failed') return fillTemplate(w.velaRelayFailed, { reason });
		if (status.at === null) return fillTemplate(w.velaRelayUnreachable, { reason });
		return fillTemplate(w.velaRelayRetrying, {
			reason,
			seconds: String(Math.max(1, Math.ceil((status.at - now) / 1000)))
		});
	}

	const relayTrouble = $derived(
		!!wallet && (wallet.relay.kind === 'retrying' || wallet.relay.kind === 'failed')
	);

	const status = $derived.by(() => {
		if (!wallet) return '';
		if (relayTrouble && wallet.phase === 'waiting') return relayText(wallet.relay);
		// ms-15: the page went away mid-request — reopen it, it resumes.
		if (wallet.away) return w.pageAway;
		switch (wallet.phase) {
			case 'waiting':
				return w.waiting;
			case 'connecting':
				return w.connecting;
			case 'wrongAccount':
				return fillTemplate(w.wrongAccount, {
					account: wallet.accounts[0] ? shortenAddress(eip55(wallet.accounts[0])) : '—'
				});
			case 'switching':
				return fillTemplate(w.switching, { chain: chainName(request.chainId) });
			case 'confirming':
				return fillTemplate(w.inWallet, { wallet: walletLabel, elapsed: elapsed(wallet.since) });
			default:
				return '';
		}
	});
	/** r4C-3: long enough in the wallet to say where to look, with Cancel and Try again. */
	const stalled = $derived(
		!!wallet &&
			wallet.phase === 'confirming' &&
			!wallet.away &&
			now - wallet.since > STILL_WAITING_MS
	);

	/** The call this SafeTx makes, when it is one to another contract (the core's reading). */
	let reading = $state<CallReading | null>(null);
	/** A batch's calls that change the multisig's own members, said first. */
	let changes = $state<string[]>([]);

	onMount(() => {
		let action = null;
		try {
			action = reviewTx(request.chainId, request.safe, request.tx).action;
		} catch {
			// Nothing the core can read: the wallet's own prompt is all there is.
		}
		changes = memberChanges(m, action);
		const calls = readableCalls(action);
		if (calls) {
			reading = new CallReading(request.chainId, request.safe, calls);
			void reading.start();
		}
		// The connect page opens where the owner's wallet is: the wallet's web
		// address, never this extension's own.
		wallet = ExternalWallet.for(request.owners, page, tx);
		void run();
		const clock = setInterval(() => (now = Date.now()), 1000);
		const mailbox = setInterval(() => void fromMailbox(), MAILBOX_EVERY_MS);
		return () => {
			clearInterval(clock);
			clearInterval(mailbox);
			controller.abort();
			wallet?.leave();
			reading?.dispose();
		};
	});

	/** One run per ask: a later Try again makes the earlier one's failure moot. */
	let runs = 0;

	async function run() {
		const current = wallet;
		if (!current) return;
		const mine = ++runs;
		const signal = controller.signal;
		failure = null;
		try {
			const answer = await current.signTypedData(
				request.chainId,
				request.owners,
				typedDataOf(request),
				signal
			);
			const approval = checked(request.safeTxHash, request.owners, answer);
			if (!approval) {
				if (mine === runs) failure = w.badSignature;
				return;
			}
			// A valid approval is welcome from any run, even one asked again since.
			finish(current, approval);
		} catch (error) {
			if (mine !== runs || current !== wallet) return;
			if (error instanceof DOMException && error.name === 'AbortError') return;
			// ms-12: a connection declined on the page is said as that, not as a
			// signature the wallet refused.
			const connecting = current.phase === 'waiting' || current.phase === 'connecting';
			failure =
				error instanceof WalletDeclinedError
					? connecting
						? w.connectDeclined
						: w.walletDeclined
					: error instanceof WalletOtherChainError
						? fillTemplate(w.otherChain, { chain: chainName(request.chainId) })
						: error instanceof CallError
							? problemText(
									walletProblem(error),
									w,
									fillTemplate,
									walletLabel,
									chainName(request.chainId)
								)
							: problemText(
									{ kind: 'unknown', reason: String((error as Error).message) },
									w,
									fillTemplate,
									walletLabel,
									''
								);
		}
	}

	let finished = false;
	function finish(current: ExternalWallet, approval: Parameters<ExternalRequest['resolve']>[0]) {
		if (finished) return;
		finished = true;
		current.to('done');
		controller.abort();
		current.leave();
		request.resolve(approval);
	}

	/**
	 * r4C-2: the owner approved from the link alone (the page handed it to the
	 * mailbox): checked here as any answer, it ends this request.
	 */
	async function fromMailbox() {
		if (!wallet || finished) return;
		try {
			const answer = await fetchProposals(request.chainId, request.safe, { background: true });
			const p = answer?.proposals.find(
				(x) => x.safeTxHash.toLowerCase() === request.safeTxHash.toLowerCase()
			);
			for (const a of p?.approvals ?? []) {
				if (!isEcdsaApproval(a) || !request.owners.some((o) => sameAddress(o, a.owner))) continue;
				const approval = checked(request.safeTxHash, request.owners, a);
				if (approval && wallet) return finish(wallet, approval);
			}
		} catch {
			// the mailbox away: the live path carries on
		}
	}

	/** Cancel the wallet's ask and make it again (r4C-3). */
	function retry() {
		controller.abort();
		controller = new AbortController();
		void run();
	}

	/** Stop waiting on the wallet: the page is told (`walletpair_cancel`); the sheet stays. */
	function stopWaiting() {
		controller.abort();
		controller = new AbortController();
		failure = w.stoppedWaiting;
	}

	/** r4C-6: a new room — another wallet, or the same one in another browser. */
	function anotherWallet(unpair: boolean) {
		controller.abort();
		controller = new AbortController();
		if (unpair) wallet?.end();
		else wallet?.leave();
		wallet = ExternalWallet.fresh(request.owners, page, tx);
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
		<CallReview {reading} m={signing} alerts={changes} />
	{/if}
	{#if wallet}
		{#if wallet.resumed}
			<!-- r4C-6: which wallet and account this device is paired with. -->
			<p class="paired" data-testid="external-paired">
				{fillTemplate(w.pairedWith, {
					account: wallet.pairedAccount ? shortenAddress(eip55(wallet.pairedAccount)) : '—',
					wallet: walletLabel
				})}
			</p>
			<p class="body">{fillTemplate(w.resumeBody, { wallet: walletLabel })}</p>
		{:else}
			<p class="body">{w.scanBody}</p>
		{/if}
		{#if qr}
			<svg
				class="qr"
				viewBox="-2 -2 {qr.modules + 4} {qr.modules + 4}"
				role="img"
				aria-label={w.sheetTitle}
				data-testid="external-qr"
			>
				<rect x="-2" y="-2" width={qr.modules + 4} height={qr.modules + 4} class="qr-bg" />
				<path d={qr.path} class="qr-fg" />
			</svg>
		{/if}
		<code class="link" data-testid="external-link">{wallet.link}</code>
		<p class="fine">{w.linkLater}</p>
		<Button variant="secondary" shape="rounded" onclick={copy}>
			<span class="label"
				><Icon icon={copied ? UTILITY_ICONS.check : UTILITY_ICONS.copy} size="sm" />{copied
					? w.copied
					: w.copyLink}</span
			>
		</Button>
		{#if failure}
			<p class="error" role="alert" data-testid="external-failure">{failure}</p>
			<Button variant="primary" shape="rounded" onclick={retry}>{w.retry}</Button>
		{:else if status}
			<p
				class="status"
				class:trouble={relayTrouble && wallet.phase === 'waiting'}
				aria-live="polite"
				data-testid="external-status"
			>
				<span class="dot" class:busy={wallet.phase !== 'wrongAccount'}></span>{status}
			</p>
			{#if relayTrouble && wallet.phase === 'waiting'}
				<Button variant="secondary" shape="rounded" onclick={() => wallet?.retryRelay()}
					>{w.retry}</Button
				>
			{/if}
			{#if stalled}
				<p class="fine" data-testid="external-still-waiting">
					{fillTemplate(w.stillWaiting, { wallet: walletLabel })}
				</p>
				<div class="two">
					<Button variant="secondary" shape="rounded" onclick={stopWaiting}>{w.stopWaiting}</Button>
					<Button variant="secondary" shape="rounded" onclick={retry}>{w.askAgain}</Button>
				</div>
			{/if}
		{/if}
		{#if wallet.resumed}
			<div class="pairing">
				<button type="button" class="text-button" onclick={() => anotherWallet(false)}
					>{w.useAnother}</button
				>
				<button
					type="button"
					class="text-button"
					data-testid="external-unpair"
					onclick={() => anotherWallet(true)}>{w.unpair}</button
				>
			</div>
		{/if}
	{/if}
</Sheet>

<style>
	.body {
		margin: 0 0 var(--space-lg);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-muted);
	}

	.paired {
		margin: 0 0 var(--space-sm);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
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
		margin-block-end: var(--space-sm);
		padding: var(--space-sm) var(--space-md);
		border-radius: var(--radius-md);
		background: var(--color-bg-sunken);
		font-family: var(--font-mono);
		font-size: calc(var(--text-xs) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		overflow-wrap: anywhere;
	}

	.fine {
		margin: 0 0 var(--space-md);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-muted);
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
		margin: var(--space-lg) 0 var(--space-md);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	.status.trouble {
		color: var(--color-error-base);
	}

	.dot {
		flex: none;
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

	.two {
		display: flex;
		gap: var(--space-sm);
	}

	.two > :global(*) {
		flex: 1;
	}

	.pairing {
		display: flex;
		gap: var(--space-xl);
		justify-content: center;
		margin-block-start: var(--space-lg);
	}

	.text-button {
		min-height: var(--size-control-sm);
		padding: 0;
		border: none;
		background: none;
		color: var(--color-fg-muted);
		font: inherit;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		text-decoration: underline;
		cursor: pointer;
	}

	.error {
		margin: var(--space-lg) 0 var(--space-md);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-error-base);
	}
</style>
