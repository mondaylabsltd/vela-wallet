<script lang="ts">
	/**
	 * One dApp request, from arrival to answer — wherever it is answered.
	 *
	 * This used to be the body of `[locale]/request/+page.svelte`, which was
	 * both doors at once: the side panel (no `rid`, ask the worker what this tab
	 * owes) and the dedicated window (`?rid=`). Spec 077 FR-001 sends the panel
	 * to the WALLET instead, so the lifecycle had to leave the page it was
	 * written in — and a second copy of it on the wallet would be a second place
	 * for an answer to go missing. One copy, two modes.
	 *
	 * Every decision here is `dapp_permissions`'. The surface shows who is
	 * asking — the browser's own fact about the origin, never the page's claim —
	 * and performs what the core authors: the grant, the audit row, the answer.
	 *
	 * Leaving is not neutral. An explicit Cancel is 4001, "nothing happened". A
	 * surface torn down with an answer still owed settles with the CORE's code,
	 * which is 4900 unknown-pending — because a dApp reads 4001 as a clean
	 * decline and re-sends, double-spending an operation that may already be at
	 * the bundler.
	 *
	 * **What this does NOT own: the sheet.** The page mounts one `SigningHost`
	 * for everything it signs — a send, a backup, a dApp request — and this
	 * hands the request to the same resident machine that feeds it. That is the
	 * "签名管线应该统一" the owner asked for: one sheet, one landing, one
	 * pipeline, whatever asked.
	 */
	import { onMount } from 'svelte';
	import { session } from '$lib/session/core/session.svelte';
	import { hostLabel } from '$lib/dapp/host';
	import { publishExtSnapshot } from '$lib/dapp/core/ext-cache';
	import { publishExtChains } from '$lib/dapp/core/ext-chains';
	import { getOriginChain } from '$lib/dapp/grants';
	import { approve, evaluate, type RequestStage } from '$lib/dapp/request';
	import { signRequest } from '$lib/signing/core/sign-resident.svelte';
	import BottomSheet from '$lib/wallet/ui/BottomSheet.svelte';
	import type { RequestMessages } from '$lib/dapp/messages';
	import {
		answerRequest,
		currentPanelRequest,
		panelTabId,
		readRequest,
		rejectRequest,
		subscribeRequests,
		type ExtensionRequest
	} from '$lib/dapp/transport';

	interface Props {
		/**
		 * `panel` — the side panel, which IS the wallet (FR-001): the consent
		 * card rises as a sheet over it, and an answered request leaves the
		 * wallet standing.
		 *
		 * `window` — the dedicated `?rid=` window a page opened with no user
		 * gesture. It has nothing behind it, and the worker closes it the moment
		 * the answer goes out (FR-003), so it draws the card as the whole page.
		 */
		mode: 'panel' | 'window';
		messages: RequestMessages;
		locale: string;
	}

	let { mode, messages: m, locale }: Props = $props();

	/**
	 * How long this surface waits, after the answer, to see whether a
	 * transaction is landing before it leaves.
	 */
	const HANDOFF_GRACE_MS = 6000;

	let request = $state<ExtensionRequest | null>(null);
	let stage = $state<RequestStage>({ kind: 'loading' });
	/** The chain the SITE is on — what the grant records and a signature is asked on. */
	let chainId = $state(0);
	let busy = $state(false);
	let tabId: number | undefined;
	/** Cleared the moment an answer goes out, so teardown owes nothing. */
	let owing = $state<string | null>(null);
	/**
	 * Spec 077: a transaction is landing in the sheet, so this surface does NOT
	 * leave. The receipt's own Done is what releases it.
	 */
	let landing = $state(false);

	const facts = $derived({
		activeAddress: session.view.address,
		addresses: session.view.loading ? null : session.view.accounts.map((r) => r.account.address)
	});

	/**
	 * The sheet raised a landing. Called by the page, which owns the one
	 * `SigningHost` — the receipt belongs to the sheet, not to this.
	 */
	export function noteLanding(): void {
		landing = true;
	}

	/** The person pressed Done on the landing. The surface may leave now. */
	export function noteReceiptDone(): void {
		landing = false;
		leave();
	}

	/** True while `take()` is mid-flight, so two arrivals cannot both claim one. */
	let taking = false;
	let disposed = false;

	/**
	 * Take the request this surface is here for, and carry it to an answer.
	 *
	 * Runs at mount, and again — in the panel — whenever the worker records a new
	 * one, because the panel is the wallet now and STAYS OPEN: a second request
	 * only re-shows it, with no reload and no fresh mount (measured 2026-09-23,
	 * when the second request sat unanswered behind a perfectly good wallet).
	 *
	 * One at a time. A request already owed is finished before the next is taken;
	 * the worker hands them over oldest-first and keeps the rest.
	 */
	async function take(): Promise<void> {
		if (taking || disposed || owing) return;
		taking = true;
		try {
			await session.boot();
			let incoming: ExtensionRequest | null;
			if (mode === 'panel') {
				tabId ??= await panelTabId();
				incoming = await currentPanelRequest(tabId);
				// Nothing owed: the panel is simply the wallet, and stays it.
				if (!incoming) return;
			} else {
				incoming = await readRequest(new URLSearchParams(location.search).get('rid') ?? '');
			}
			if (disposed) return;
			if (!incoming) {
				stage = { kind: 'refused', code: 4900, message: 'This request is no longer available' };
				leave();
				return;
			}
			request = incoming;
			owing = incoming.rid;

			// Publish what the worker will need to answer this origin instantly
			// next time. The core authors it; this only stores it.
			const [snapshot] = await Promise.all([
				publishExtSnapshot({
					isLoading: session.view.loading,
					hasWallet: session.view.has_wallet,
					accounts: session.view.accounts.map((row) => row.account),
					active: session.view.accounts[session.view.active_index]?.account ?? null,
					theme: 'dark',
					locale
				}),
				publishExtChains()
			]);
			chainId = await getOriginChain(incoming.origin, snapshot?.chain_id ?? 0);

			stage = await evaluate(incoming, facts);
			if (stage.kind === 'done' || stage.kind === 'refused') {
				owing = null;
				leave();
			} else if (stage.kind === 'signing') {
				// The core said this origin may be answered, and named the address
				// the signature must be pinned to. Hand the request to
				// `sign_request` — the SAME machine and the SAME sheet the
				// wallet's own screens use — on a transport that answers here.
				await handOffToSigning(incoming, stage.grantedAddress);
			}
		} finally {
			taking = false;
		}
	}

	onMount(() => {
		void take();
		// A request that arrives while this surface is already standing.
		const unsubscribe = mode === 'panel' ? subscribeRequests(() => void take()) : () => {};

		// The surface is going away with an answer still owed. The code is NOT
		// this screen's to pick — it is asked of the core, once, in `settleOnClose`.
		const settle = () => {
			if (!owing) return;
			const rid = owing;
			owing = null;
			void settleOnClose(rid);
		};
		window.addEventListener('pagehide', settle);
		return () => {
			disposed = true;
			unsubscribe();
			window.removeEventListener('pagehide', settle);
			settle();
		};
	});

	/**
	 * Register this surface as the transport and deliver the request.
	 *
	 * The core speaks a `transport_id` and nothing else about transports: a
	 * response goes to the transport that OWNS the request, never a shared
	 * reference. This surface owns exactly one, and answering through it is what
	 * clears what it owes — so the teardown settlement no longer fires.
	 */
	async function handOffToSigning(incoming: ExtensionRequest, grantedAddress: string) {
		await signRequest.boot();
		signRequest.syncNetworks();
		signRequest.syncAccounts();
		const transportId = signRequest.registerTransport({
			sendResponse: (_id, result, error) => {
				// The answer goes out FIRST and unconditionally. Everything below
				// is about the chain, and nothing below may delay, swallow or
				// repeat this (spec 077, the invariant).
				owing = null;
				void answerRequest(incoming.rid, error ? { error } : { result });
				// A transaction the wallet can watch stays on screen and lands,
				// as a send does: the sheet raises its receipt and calls
				// `onlanding`, and the person's Done is what leaves. Anything
				// else — a message, a refusal, a transaction with no operation to
				// follow — leaves as it always has.
				setTimeout(() => {
					if (!landing) leave();
				}, HANDOFF_GRACE_MS);
			}
		});
		signRequest.dispatch({
			type: 'request_arrived',
			id: incoming.id,
			method: incoming.method,
			params_json: JSON.stringify(incoming.params),
			origin: incoming.origin,
			transport_id: transportId,
			dedicated_transport: true,
			// The chain the SITE is on (its `wallet_switchEthereumChain`, else the
			// chain it connected on). A chain the wallet does not support is the
			// machine's 4902, before any sheet.
			per_request_chain: chainId > 0 ? chainId : null,
			dapp: null,
			// Invariant ⑨: the signature is pinned to the GRANT's address, never
			// to whichever account happens to be active.
			granted_address: grantedAddress,
			requested_address: null,
			request_ts_ms: null,
			now_ms: Date.now()
		});
	}

	async function settleOnClose(rid: string): Promise<void> {
		try {
			const [{ popupCloseSettlement }, { dpermRejectMessage }] = await Promise.all([
				import('$lib/dapp/core/dperm-connect'),
				import('$lib/dapp/core/dperm-types')
			]);
			const settlement = popupCloseSettlement();
			await answerRequest(rid, {
				error: { code: settlement.code, message: dpermRejectMessage(settlement.reason) }
			});
		} catch {
			// Teardown must not throw. Saying nothing leaves the dApp on its own
			// deadline, which is the one honest fallback: anything invented here
			// would be a second statement of the core's rule.
		}
	}

	/**
	 * Leave — after a short delay, so the answer reaches the page before this
	 * surface changes underneath it.
	 *
	 * In the WINDOW that means closing it. In the PANEL it does not: the panel is
	 * the wallet, and a wallet that vanished when a site's request was answered is
	 * the complaint spec 077 started from. It stays, and takes the next request
	 * the tab owes, if any.
	 *
	 * The panel used to RELOAD per request — a fresh core and a fresh fee session
	 * (026's one-owner rule) — and cannot any more: a reload would throw away the
	 * wallet the person is looking at, and a request that arrives while the panel
	 * is open never reloads it anyway. Taking them one at a time is what keeps
	 * the one-owner rule instead: `take()` refuses to start while a request is
	 * owed, so the page's single fee session is only ever asked about one
	 * operation, exactly as it is when a person signs two sends in a row.
	 */
	function leave(): void {
		setTimeout(() => {
			if (mode === 'window') {
				window.close();
				return;
			}
			// Never over a landing: a person watching their transaction is not
			// interrupted by the next request. It waits — it is still owed, and
			// this runs again when the receipt is dismissed.
			if (!landing) void take();
		}, 400);
	}

	async function onConnect(): Promise<void> {
		if (!request || busy) return;
		busy = true;
		try {
			await approve(request, facts, chainId);
			owing = null;
			stage = { kind: 'done' };
			leave();
		} catch {
			// The core did not sanction it. Nothing was written and nothing sent,
			// so both buttons stay live rather than stranding the person.
			busy = false;
		}
	}

	async function onCancel(): Promise<void> {
		if (busy) return;
		busy = true;
		const rid = owing;
		owing = null;
		if (rid) await rejectRequest(rid);
		if (mode === 'panel') {
			// The card is gone; the wallet is what is behind it.
			request = null;
			stage = { kind: 'loading' };
			busy = false;
		}
		leave();
	}

	/** The consent card's own words, shared by both shapes. */
	const cardTitle = $derived(request ? m.title.replace('{{host}}', hostLabel(request.origin)) : '');
	/** Unused by the panel; the window draws its own preparing/refused lines. */
	const showWindowChrome = $derived(mode === 'window' && stage.kind !== 'signing' && !landing);
</script>

{#if mode === 'panel'}
	<!--
		The wallet is VISIBLE under a pending request, and must not be DRIVABLE:
		a person who could start a send while a dApp waits on them would have two
		operations in one fee session, and the dApp's would be the one that
		quietly lost (spec 077, plan.md's third risk).
	-->
	{#if owing}
		<div class="guard" role="presentation" aria-hidden="true"></div>
	{/if}

	<!--
		FR-001: over the wallet, as on Android and iOS. Dismissing the sheet is
		the same answer as Cancel — the core's 4001 — so the scrim and the × go
		through `onCancel` rather than merely hiding the card.

		The layer is this component's, not `BottomSheet`'s: the sheet positions
		itself `absolute`, which over a scrollable wallet would land wherever the
		nearest positioned ancestor happens to be. Fixed, and stacked with the
		signing sheet's own 20/21, it is the same modal the sheet is.
	-->
	{#if stage.kind === 'consent' && request}
		<div class="layer">
			<BottomSheet title={cardTitle} closeLabel={m.cancel} onclose={onCancel}>
				<div class="card">
					<p class="body">{m.body}</p>
					<p class="method">{request.method}</p>
					<div class="actions">
						<button type="button" class="ghost" onclick={onCancel} disabled={busy}>
							{m.cancel}
						</button>
						<button type="button" class="primary" onclick={onConnect} disabled={busy}>
							{m.connect}
						</button>
					</div>
				</div>
			</BottomSheet>
		</div>
	{/if}
{:else if showWindowChrome}
	<main>
		{#if stage.kind === 'consent' && request}
			<h1>{cardTitle}</h1>
			<p class="body">{m.body}</p>
			<p class="method">{request.method}</p>
			<div class="actions">
				<button type="button" class="ghost" onclick={onCancel} disabled={busy}>{m.cancel}</button>
				<button type="button" class="primary" onclick={onConnect} disabled={busy}>
					{m.connect}
				</button>
			</div>
		{:else if stage.kind === 'refused'}
			<p class="body">{stage.message}</p>
		{:else}
			<p class="body">{m.preparing}</p>
		{/if}
	</main>
{/if}

<style>
	/*
	  Invisible on purpose. The wallet behind a pending request is something to
	  READ — whose wallet this is, what is in it — not something to operate, and
	  a dimming layer over it would take the reading away too. It sits just under
	  the sheet's scrim (`SigningSheet.svelte`, 20) so the sheet always wins.
	*/
	.guard {
		position: fixed;
		inset: 0;
		z-index: 19;
	}

	.layer {
		position: fixed;
		inset: 0;
		z-index: 20;
	}

	main {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		padding: var(--space-6);
		min-height: 100vh;
		background: var(--color-bg-base);
		color: var(--color-text-primary);
	}
	.card {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		padding-bottom: var(--space-4);
	}
	h1 {
		font-size: var(--font-size-title-3);
		font-weight: var(--font-weight-semibold);
		margin: 0;
	}
	.body {
		font-size: var(--font-size-body);
		color: var(--color-text-secondary);
		margin: 0;
	}
	.method {
		font-family: var(--font-family-mono);
		font-size: var(--font-size-caption);
		color: var(--color-text-tertiary);
		margin: 0;
	}
	.actions {
		margin-top: auto;
		display: flex;
		gap: var(--space-3);
	}
	button {
		flex: 1;
		padding: var(--space-4);
		border-radius: var(--radius-md);
		font-size: var(--font-size-body);
		cursor: pointer;
	}
	.ghost {
		border: var(--border-width-hairline) solid var(--color-border-subtle);
		background: var(--color-bg-elevated);
		color: var(--color-text-primary);
	}
	.primary {
		border: none;
		background: var(--color-accent-base);
		color: var(--color-accent-on);
	}
</style>
