<script lang="ts">
	/**
	 * One dApp request, from arrival to answer — wherever it is answered.
	 *
	 * Two modes, one lifecycle: the side panel (which IS the wallet since spec
	 * 077 FR-001, the request rising as a card or sheet over it) and the
	 * dedicated `?rid=` window a page fired a request into with no user
	 * gesture. A second copy of this on another surface would be a second
	 * place for an answer to go missing.
	 *
	 * Every decision here is `dapp_permissions'` or `sign_request`'s. The
	 * surface shows who is asking — the browser's own fact about the origin,
	 * never the page's claim — and performs what the core authors: the grant,
	 * the audit row, the answer.
	 *
	 * Spec 082 moved the request's LIFE to the service worker (RB1–RB10): the
	 * worker hands this surface what it owes (`panelSurface.current`, oldest
	 * first across every tab of the window — G18), withdraws a request whose
	 * page left or whose time ran out (the card or sheet closes with no words,
	 * RB15), and is asked — before a grant, before the passkey, before the
	 * relay POST — whether the request is still live (RB5). A surface torn down
	 * with an answer owed still settles with the CORE's code (4900) on
	 * `pagehide`, as a backstop to the worker seeing the port close.
	 *
	 * **What this does NOT own: the sheet.** The page mounts one `SigningHost`
	 * for everything it signs, and this hands the request to the same resident
	 * machine that feeds it.
	 */
	import { onMount } from 'svelte';
	import { session } from '$lib/session/core/session.svelte';
	import { hostLabel } from '$lib/dapp/host';
	import { publishExtSnapshot } from '$lib/dapp/core/ext-cache';
	import { publishExtChains } from '$lib/dapp/core/ext-chains';
	import { getOriginChain } from '$lib/dapp/grants';
	import { approve, evaluate, type RequestStage } from '$lib/dapp/request';
	import { signRequest } from '$lib/signing/core/sign-resident.svelte';
	import { AskerGoneError, type ClaimPhase, type SubmitClaim } from '$lib/signing/core/sign-types';
	import BottomSheet from '$lib/wallet/ui/BottomSheet.svelte';
	import Button from '$lib/ui/Button.svelte';
	import type { RequestMessages } from '$lib/dapp/messages';
	import { panelSurface } from '$lib/dapp/panel-surface.svelte';
	import {
		answerRequest,
		readRequest,
		rejectRequest,
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
	/** Cleared the moment an answer goes out, so teardown owes nothing. */
	let owing = $state<string | null>(null);
	/** The signing transport of the request being signed, for a withdrawal. */
	let signingTransport: string | null = null;
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

	/** The window's own request id (`?rid=`), or `''` in the panel. */
	const windowRid = $derived(
		mode === 'window' && typeof location !== 'undefined'
			? (new URLSearchParams(location.search).get('rid') ?? '')
			: ''
	);

	/**
	 * Is `rid` still live enough to act on (RB5)? A surface with no port cannot
	 * ask: yes. The `submit` claim carries the op's hash and chain (spec 082
	 * RJ2), sent after the write-ahead and right before the POST, so the worker
	 * answers the page by that hash — never 4900 — if this surface goes first.
	 */
	function claimFor(rid: string, phase: ClaimPhase, submit?: SubmitClaim): Promise<boolean> {
		return panelSurface.caller ? panelSurface.claim(rid, phase, submit) : Promise.resolve(true);
	}

	/**
	 * Take the request this surface is here for, and carry it to an answer.
	 *
	 * In the panel it is whatever the worker says the window owes next
	 * (`panelSurface.current`), taken whenever that changes; in the window it is
	 * the one request the window was opened for. One at a time: a request
	 * already owed is finished before the next is taken, and nothing is taken
	 * over a landing — the receipt replaces the sheet, so a request taken now
	 * would have an invisible sheet. The worker keeps the rest.
	 */
	async function take(): Promise<void> {
		if (taking || disposed || owing || landing) return;
		taking = true;
		try {
			await session.boot();
			let incoming: ExtensionRequest | null;
			if (mode === 'panel') {
				incoming = panelSurface.current;
				// Nothing owed: the panel is simply the wallet, and stays it.
				if (!incoming) return;
			} else {
				incoming = await readRequest(windowRid);
			}
			if (disposed) return;
			if (!incoming) {
				// A window with no live request has nothing to show: it closes,
				// rather than hard-coding a sentence the corpus does not have.
				stage = { kind: 'loading' };
				leave();
				return;
			}
			request = incoming;
			owing = incoming.rid;
			signingTransport = null;
			// A fresh request gets live buttons. The panel outlives the request
			// that raised it, so `busy` left over from the LAST one would render
			// the next consent card with both buttons disabled.
			busy = false;
			panelSurface.shown(incoming.rid);

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
			// Withdrawn while the snapshot was being published.
			if (owing !== incoming.rid) return;

			stage = await evaluate(incoming, facts);
			if (stage.kind === 'done' || stage.kind === 'refused') {
				owing = null;
				request = null;
				leave();
			} else if (stage.kind === 'signing') {
				await handOffToSigning(incoming, stage.grantedAddress);
			}
		} finally {
			taking = false;
		}
	}

	/**
	 * The worker withdrew `rid` — its page left, its surface closed, or its
	 * time ran out (RB15). The card closes with no words; a request already
	 * with the signing machine is dropped there, which also stops a pipeline
	 * still short of the passkey (RB2).
	 */
	function withdraw(rid: string): void {
		if (owing !== rid && request?.rid !== rid) return;
		if (signingTransport) {
			signRequest.dispatch({ type: 'transport_dropped', transport_id: signingTransport });
			signingTransport = null;
		}
		owing = null;
		request = null;
		busy = false;
		stage = { kind: 'loading' };
		leave();
	}

	onMount(() => {
		if (mode === 'window' && windowRid) void panelSurface.start({ kind: 'window', rid: windowRid });
		const stopWithdrawn = panelSurface.onWithdrawn((rid) => withdraw(rid));
		if (mode === 'window') void take();

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
			stopWithdrawn();
			window.removeEventListener('pagehide', settle);
			settle();
		};
	});

	// The panel: a request the worker says this window owes — at mount, and
	// whenever the next one moves up (a second tab, a request that arrives
	// while the panel stands open).
	$effect(() => {
		if (mode !== 'panel') return;
		if (panelSurface.current) void take();
	});

	/**
	 * Register this surface as the transport and deliver the request.
	 *
	 * The core speaks a `transport_id` and nothing else about transports: a
	 * response goes to the transport that OWNS the request. This surface owns
	 * exactly one, and answering through it is what clears what it owes.
	 */
	async function handOffToSigning(incoming: ExtensionRequest, grantedAddress: string) {
		await signRequest.boot();
		signRequest.syncNetworks();
		signRequest.syncAccounts();
		const transportId = signRequest.registerTransport({
			sendResponse: (_id, result, error, opHash) => {
				// The answer goes out FIRST and unconditionally. Everything below
				// is about the chain, and nothing below may delay, swallow or
				// repeat this (spec 077, the invariant).
				owing = null;
				signingTransport = null;
				void answerRequest(incoming.rid, error ? { error } : { result }, opHash);
				// A request the CORE refuses outright is answered the moment it
				// arrives — so the page never hangs — but the sheet is still
				// explaining WHY; the person's dismissal is what leaves (081).
				if (error?.kind === 'self_call_blocked') return;
				// A transaction the wallet can watch stays on screen and lands; the
				// person's Done is what leaves. Anything else leaves as it always has.
				setTimeout(() => {
					if (!landing) leave();
				}, HANDOFF_GRACE_MS);
			},
			// RB5: asked before the passkey and before the relay POST. A page
			// that is gone gets nothing signed, nothing sent, and no answer.
			claim: (_id, phase, submit) => claimFor(incoming.rid, phase, submit)
		});
		signingTransport = transportId;
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
			// Teardown must not throw. The worker settles on the port closing.
		}
	}

	/**
	 * Leave — after a short delay, so the answer reaches the page before this
	 * surface changes underneath it.
	 *
	 * In the WINDOW that means closing it. In the PANEL it does not: the panel is
	 * the wallet. It stays, and takes the next request the window owes, if any —
	 * one at a time, so the page's single fee session is only ever asked about
	 * one operation (026's one-owner rule).
	 */
	function leave(): void {
		setTimeout(() => {
			if (mode === 'window') {
				window.close();
				return;
			}
			// Never over a landing: a person watching their transaction is not
			// interrupted by the next request. It waits, and this runs again when
			// the receipt is dismissed.
			if (!landing) void take();
		}, 400);
	}

	async function onConnect(): Promise<void> {
		if (!request || busy) return;
		busy = true;
		const current = request;
		try {
			await approve(current, facts, chainId, () => claimFor(current.rid, 'approve'));
			owing = null;
			stage = { kind: 'done' };
			request = null;
			busy = false;
			leave();
		} catch (error) {
			if (error instanceof AskerGoneError) {
				// The page left before the grant was written: nothing was granted,
				// nobody is waiting. The card goes, with no words (RB15).
				withdraw(current.rid);
				return;
			}
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
		// The card goes BEFORE the answer is awaited: the worker hands the next
		// request of the window over as soon as it has this answer, and a card
		// cleared after the await would clear THAT one (EX4 caught it).
		if (mode === 'panel') {
			request = null;
			stage = { kind: 'loading' };
		}
		if (rid) await rejectRequest(rid);
		if (mode === 'panel') busy = false;
		leave();
	}

	/** The consent card's own words, shared by both shapes. */
	const cardTitle = $derived(request ? m.title.replace('{{host}}', hostLabel(request.origin)) : '');
	/** Unused by the panel; the window draws its own preparing line. */
	const showWindowChrome = $derived(mode === 'window' && stage.kind !== 'signing' && !landing);
</script>

{#snippet actions()}
	<!--
		RB12 (G16): the shared Button — Cancel is the bordered secondary, Connect
		the filled accent with a white label, both at the control height. Busy is
		not disabled: Connect spins while the grant is written; Cancel is what is
		disabled then, because a refusal racing a grant is two answers.
	-->
	<div class="actions">
		<Button variant="secondary" shape="rounded" disabled={busy} onclick={onCancel}>
			{m.cancel}
		</Button>
		<Button variant="primary" shape="rounded" loading={busy} onclick={onConnect}>
			{m.connect}
		</Button>
	</div>
{/snippet}

{#if mode === 'panel'}
	<!--
		The wallet is VISIBLE under a pending request, and must not be DRIVABLE:
		a person who could start a send while a dApp waits on them would have two
		operations in one fee session (spec 077, plan.md's third risk).
	-->
	{#if owing}
		<div class="guard" role="presentation" aria-hidden="true"></div>
	{/if}

	<!--
		FR-001: over the wallet, as on Android and iOS. Closing the card is the
		same answer as Cancel — the core's 4001 — so the × goes through
		`onCancel`. Spec 079: the × and Cancel are the ONLY ways out.
	-->
	{#if stage.kind === 'consent' && request}
		<div class="layer">
			<BottomSheet
				title={cardTitle}
				closeLabel={m.cancel}
				dismissible={busy ? false : 'explicit'}
				onclose={onCancel}
			>
				<div class="card">
					<p class="body">{m.body}</p>
					{@render actions()}
				</div>
			</BottomSheet>
		</div>
	{/if}
{:else if showWindowChrome}
	<main>
		{#if stage.kind === 'consent' && request}
			<h1>{cardTitle}</h1>
			<p class="body">{m.body}</p>
			{@render actions()}
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
	  READ, not to operate. It sits just under the sheet's scrim
	  (`SigningSheet.svelte`, 20) so the sheet always wins.
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
		gap: var(--space-xl);
		padding: var(--space-3xl);
		min-height: 100vh;
		background: var(--color-bg-base);
		color: var(--color-fg-base);
	}
	.card {
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
		padding-bottom: var(--space-xl);
	}
	h1 {
		font-size: calc(var(--text-2xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		margin: 0;
	}
	.body {
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		color: var(--color-fg-muted);
		margin: 0;
	}
	.actions {
		margin-top: auto;
		display: flex;
		gap: var(--space-lg);
	}
	.actions > :global(*) {
		flex: 1;
	}
</style>
