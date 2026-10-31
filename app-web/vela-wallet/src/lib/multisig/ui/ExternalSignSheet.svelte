<script lang="ts">
	/**
	 * An outside owner signs: Vela opens a room and shows the connect page's
	 * link and code; the owner opens it where their wallet is (a browser with
	 * MetaMask or Rabby, or a wallet app's browser), connects, and confirms
	 * the SafeTx in their wallet. The answer is checked by the core before it
	 * counts. A wallet paired before only needs the page reopened.
	 *
	 * The core's session machine (`multisig_outside::session`,
	 * `OutsideSessionHost`) runs the ask — the room or the pairing, the
	 * connection, the owner among the wallet's accounts, the network, the
	 * signature verified, the mailbox looked at — and this sheet draws its view
	 * and says it in the corpus' words.
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
	import type { SigningMessages } from '$lib/signing/messages';
	import type { MultisigMessages } from '../messages';
	import { asReview, readTx } from '../core/reading';
	import { nativeSymbol } from '$lib/services/networks';
	import { memberChanges } from '../core/display';
	import { reviewTx, type MultisigReviewOut } from '../core/kernel';
	import ReviewBody from '../external/ReviewBody.svelte';
	import type { ExternalRequest } from '../external/signing.svelte';
	import { problemText } from '../external/errors';
	import { eip55 } from '../external/eip55';
	import type { OutsideSessionFailure } from '../generated/OutsideSessionFailure';
	import type { RelayStatus } from '../generated/RelayStatus';
	import {
		connectPage,
		MAILBOX_EVERY_MS,
		OutsideSessionHost
	} from '../shell/outside-session.svelte';
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
		/**
		 * The one review of this SafeTx (§3.6), as the proposals machine gives
		 * it (`view.reviews['<chainId>:<safeTxHash>']`): drawn as the connect
		 * page draws it, the one reading every signer sees. Absent (not
		 * gathered yet): the call is read by the signing sheet machines alone.
		 */
		review?: MultisigReviewOut | null;
	}

	let { m, signing, locale, request: asked, wide, closeLabel, review = null }: Props = $props();
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

	let session = $state<OutsideSessionHost | null>(null);
	let copied = $state(false);
	let now = $state(Date.now());

	const wallet = $derived(session?.view ?? null);
	/**
	 * The page's link, once there is a room to open it to: a new room's
	 * invite is in it (the room opens a moment after the sheet), a pairing
	 * kept before needs none. Never a link without the room it names.
	 */
	const link = $derived(session && wallet && (wallet.resumed || wallet.invite) ? session.link : '');

	// r4C-6: the code is always there — a paired room's link reopens the page.
	const qr = $derived.by(() => {
		if (!link) return null;
		try {
			return encodeQr(link);
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
			!wallet.failure &&
			now - wallet.since > STILL_WAITING_MS
	);

	/** Why the ask ended without an answer, said with what to do (r4C-11, ms-12, ms-26). */
	function failureText(f: OutsideSessionFailure): string {
		const chain = chainName(request.chainId);
		switch (f.kind) {
			case 'connectDeclined':
				return w.connectDeclined;
			case 'walletDeclined':
				return w.walletDeclined;
			case 'otherChain':
				return fillTemplate(w.otherChain, { chain });
			case 'badSignature':
			case 'noAnswer':
				return w.badSignature;
			case 'stoppedWaiting':
				return w.stoppedWaiting;
			case 'noRoom':
				return problemText({ kind: 'unknown', reason: f.reason }, w, fillTemplate, walletLabel, '');
			case 'wallet':
				return problemText(
					f.problem,
					w,
					fillTemplate,
					walletLabel,
					f.problem.kind === 'unknown' ? '' : chain
				);
		}
	}
	const failure = $derived(wallet?.failure ? failureText(wallet.failure) : null);

	/** r8-1: the core's reading of its calls, before (or without) the review. */
	let reading = $state<MultisigReviewOut | null>(null);
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
		// The fallback only: with the review, its reading is drawn by `ReviewBody`.
		if (!untrack(() => review)) {
			const read = asReview(readTx(request.chainId, request.safe, request.tx));
			if (read && read.reading.calls.length > 0) reading = read;
		}
		// The connect page opens where the owner's wallet is: the wallet's web
		// address, never this extension's own.
		const host = new OutsideSessionHost(
			{
				chainId: request.chainId,
				safe: request.safe,
				tx: request.tx,
				safeTxHash: request.safeTxHash,
				owners: request.owners,
				send: request.send ?? null
			},
			connectPage(locale)
		);
		session = host;
		const clock = setInterval(() => (now = Date.now()), 1000);
		// r4C-2: an approval made from the link alone ends this request too.
		const mailbox = setInterval(() => host.dispatch({ type: 'mailbox_tick' }), MAILBOX_EVERY_MS);
		return () => {
			clearInterval(clock);
			clearInterval(mailbox);
			host.dispose();
		};
	});

	/** The ask ended (approved and verified, sent, or closed): the caller is told, once. */
	let settled = false;
	const outcome = $derived(wallet?.outcome ?? null);
	$effect(() => {
		const done = outcome;
		if (!done || settled) return;
		settled = true;
		untrack(() => request.settle(done));
	});

	/** Cancel the wallet's ask and make it again (r4C-3). */
	function retry() {
		session?.dispatch({ type: 'ask_again' });
	}

	/** Stop waiting on the wallet: the page is told (`walletpair_cancel`); the sheet stays. */
	function stopWaiting() {
		session?.dispatch({ type: 'stop_waiting' });
	}

	/** r4C-6: a new room — another wallet, or the same one in another browser. */
	function anotherWallet(unpair: boolean) {
		session?.dispatch({ type: 'another_wallet', unpair });
	}

	function close() {
		session?.dispatch({ type: 'close' });
		if (!settled) {
			settled = true;
			request.settle({ kind: 'cancelled' });
		}
	}

	async function copy() {
		if (!link) return;
		try {
			await navigator.clipboard.writeText(link);
			copied = true;
			setTimeout(() => (copied = false), 1600);
		} catch {
			// the link stays on screen
		}
	}
</script>

<Sheet title={w.sheetTitle} {closeLabel} {wide} onclose={close}>
	{#if review}
		<ReviewBody {m} {signing} chainId={request.chainId} safe={request.safe} {review} />
	{:else if reading}
		<CallReview
			review={reading}
			{m}
			nativeSymbol={nativeSymbol(request.chainId)}
			alerts={changes}
		/>
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
		{#if link}
			<code class="link" data-testid="external-link">{link}</code>
		{/if}
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
				<Button variant="secondary" shape="rounded" onclick={() => session?.retryRelay()}
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
