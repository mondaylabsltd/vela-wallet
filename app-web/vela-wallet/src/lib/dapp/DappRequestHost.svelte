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
	import ConsentFacts from '$lib/dapp/ConsentFacts.svelte';
	import { identiconSvgForClient } from '$lib/wallet/identicon';
	import { shortenAddress } from '$lib/wallet/identity';
	import { chainName } from '$lib/services/networks';
	import { chainLogoURL } from '$lib/services/tokens-model';
	import type { RequestMessages } from '$lib/dapp/messages';
	import { panelSurface } from '$lib/dapp/panel-surface.svelte';
	import { focusOwnWindow, openInBrowserTab } from '$lib/extension/open-tab';
	import {
		answerRequest,
		readRequest,
		rejectRequest,
		type ExtensionRequest
	} from '$lib/dapp/transport';
	import AddNetworkCard from '$lib/dapp/AddNetworkCard.svelte';
	import { addNetworkCard, CHAIN_SETUP_URL } from '$lib/dapp/add-network';
	import { dappAddChainAsk, dappAddOutcomeError } from '$lib/core/kernels';
	import { networkAdmin, onDappAddSettled } from '$lib/settings/core/network-admin.svelte';
	import type { DappAddOutcome } from '$lib/core/generated/DappAddOutcome';
	import { track } from '$lib/analytics';
	import { dappRequestKind } from '$lib/analytics/dapp';

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
	/** How long an answered request window waits for the worker before closing itself. */
	const WINDOW_CLOSE_BACKSTOP_MS = 5000;

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
	/**
	 * Spec 100: the request in hand is a page asking to add a network; its
	 * sheet is `network_admin`'s (`networkAdmin.view.dapp_add`), and the
	 * chain it asked for names the answer's words.
	 */
	let addingChain = $state<number | null>(null);

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
			// Settled, not merely booted: mid-restore the session names nobody,
			// and "which account may this site see?" is asked of the account
			// the person is signed in to (spec 086, issue 315).
			await session.settled();
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
				leave(true);
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

			// Spec 100: not a connect and not a signature — the core's
			// add-network sheet, before the connect logic sees it.
			if (incoming.method === 'wallet_addEthereumChain') {
				await takeAddNetwork(incoming);
				return;
			}

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
		if (addingChain !== null && request) {
			// The sheet is the settings machine's: close it, settling nothing.
			networkAdmin.dispatch({ type: 'dapp_add_cancelled', tab: String(request.tabId), id: rid });
			addingChain = null;
		}
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
		const stopAdding = onDappAddSettled((_tab, id, outcome) => void addSettled(id, outcome));
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
			stopAdding();
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
				// Usage statistics: the person's answer, by request kind and chain
				// — never the site, the message or the transaction.
				const answered = { kind: dappRequestKind(incoming.method), chain: chainId || undefined };
				if (!error) track('dapp_request_approved', answered);
				else if (error.code === 4001) track('dapp_request_rejected', answered);
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
			now_ms: Date.now(),
			// A site's request is never the wallet's own, whatever its words.
			first_party: false
		});
	}

	/**
	 * Spec 100: a page asks to add a network. The core reads its params (a
	 * malformed ask is -32602 here, before any sheet) and `network_admin`
	 * checks and adds the chain the way Settings does; this surface only
	 * carries the request there and the ending back.
	 */
	async function takeAddNetwork(incoming: ExtensionRequest): Promise<void> {
		const read = dappAddChainAsk(incoming.params);
		if ('error' in read) {
			owing = null;
			request = null;
			await answerRequest(incoming.rid, { error: { code: -32602, message: read.error } });
			leave();
			return;
		}
		await networkAdmin.boot();
		if (owing !== incoming.rid) return;
		addingChain = read.ok.chain_id;
		networkAdmin.dispatch({
			type: 'dapp_add_requested',
			tab: String(incoming.tabId),
			id: incoming.rid,
			origin: incoming.origin,
			ask: read.ok
		});
	}

	/**
	 * The add-network sheet ended (`dapp_add_settled`). Added: the catalog the
	 * worker reads is published FIRST — the worker moves the site to the
	 * chain when it delivers this `null` — then the page is answered. Anything
	 * else: the core's one table names the error.
	 */
	async function addSettled(id: string, outcome: DappAddOutcome): Promise<void> {
		if (owing !== id || addingChain === null) return;
		const chain = addingChain;
		owing = null;
		addingChain = null;
		busy = false;
		if (outcome.type === 'added') {
			track('dapp_request_approved', { kind: 'add_network', chain });
			await publishExtChains();
			await answerRequest(id, { result: null });
		} else {
			const error = dappAddOutcomeError(outcome, chain) ?? {
				code: 4001,
				message: 'User rejected the request'
			};
			if (error.code === 4001) track('dapp_request_rejected', { kind: 'add_network', chain });
			await answerRequest(id, { error });
		}
		request = null;
		stage = { kind: 'loading' };
		leave();
	}

	/** The sheet's Add Network: the request must still be live (RB5), then the core saves. */
	async function onAddNetwork(): Promise<void> {
		if (!request || busy || addingChain === null) return;
		busy = true;
		const rid = request.rid;
		if (!(await claimFor(rid, 'approve'))) {
			busy = false;
			withdraw(rid);
			return;
		}
		networkAdmin.dispatch({ type: 'dapp_add_approved', now_iso: new Date().toISOString() });
	}

	/** Any way out of the sheet — the core decides what it answers. */
	function onAddNetworkClosed(): void {
		if (busy) return;
		networkAdmin.dispatch({ type: 'dapp_add_declined' });
	}

	function onAddNetworkRetry(): void {
		networkAdmin.dispatch({ type: 'dapp_add_retried' });
	}

	function openChainSetup(): void {
		window.open(CHAIN_SETUP_URL, '_blank', 'noopener');
	}

	/** The add-network sheet for the request in hand, in words. */
	const addCard = $derived.by(() => {
		const view = networkAdmin.view.dapp_add;
		if (addingChain === null || !request || !view || view.id !== request.rid) return null;
		return addNetworkCard(view, m.addNetwork);
	});

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
	function leave(nothingOwed = false): void {
		setTimeout(
			() => {
				if (mode === 'window') {
					window.close();
					return;
				}
				// Never over a landing: a person watching their transaction is not
				// interrupted by the next request. It waits, and this runs again when
				// the receipt is dismissed.
				if (!landing) void take();
			},
			// A window whose request was answered or withdrawn: the WORKER closes
			// it, or hands it to the next request of the same site (spec 094 S7)
			// — closing it from here first would end that request too. Only a
			// backstop then; a window that found no request closes at once.
			mode === 'window' && !nothingOwed ? WINDOW_CLOSE_BACKSTOP_MS : 400
		);
	}

	async function onConnect(): Promise<void> {
		if (!request || busy) return;
		busy = true;
		const current = request;
		try {
			await approve(current, facts, chainId, () => claimFor(current.rid, 'approve'));
			track('dapp_connect_approved', { chain: chainId || undefined });
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
		if (stage.kind === 'consent') track('dapp_connect_rejected', { chain: chainId || undefined });
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

	/**
	 * Nobody is signed in to this wallet (spec 094 S5) — the session machine's
	 * own route, the one that sends the PANEL to the welcome. The window cannot
	 * go there: it IS the request, and leaving it would answer the page. So it
	 * says so and opens the welcome in a tab; the session follows the other
	 * document's sign-in (`storage`), and the card turns into the consent with a
	 * live Connect. Before this, Connect asked the core to grant nobody, which
	 * threw, and the button did nothing at all.
	 */
	const needsWallet = $derived(
		mode === 'window' && !session.view.loading && session.view.allowed_route !== 'wallet'
	);

	let wasWaitingForWallet = false;
	$effect(() => {
		// Signed in elsewhere while this window waited: bring it forward.
		if (needsWallet) wasWaitingForWallet = true;
		else if (wasWaitingForWallet) {
			wasWaitingForWallet = false;
			void focusOwnWindow();
		}
	});

	function openOnboarding(create: boolean): void {
		void openInBrowserTab(create ? `${locale}/create.html` : `${locale}.html`);
	}

	/** The consent card's own words, shared by both shapes. */
	const cardTitle = $derived(request ? m.title.replace('{{host}}', hostLabel(request.origin)) : '');

	/**
	 * What a Connect shares (spec 096 F11): the account the core says the grant
	 * pins (`consent_address`), named as the wallet names it, and the network
	 * the grant is written on — the same `chainId` `approve` hands the core.
	 */
	const consentAccount = $derived.by(() => {
		const address = stage.kind === 'consent' ? stage.address : null;
		if (!address) return null;
		const row = session.view.accounts.find(
			(r) => r.account.address.toLowerCase() === address.toLowerCase()
		);
		return {
			name: row?.account.name ?? '',
			address,
			short: shortenAddress(address),
			identiconSvg: identiconSvgForClient(address)
		};
	});
	const consentNetwork = $derived({ name: chainName(chainId), logoUrl: chainLogoURL(chainId) });
	/** Unused by the panel; the window draws its own preparing line. */
	const showWindowChrome = $derived(mode === 'window' && stage.kind !== 'signing' && !landing);
</script>

{#snippet consentFacts()}
	<ConsentFacts
		accountLabel={m.accountLabel}
		networkLabel={m.networkLabel}
		account={consentAccount}
		network={consentNetwork}
	/>
{/snippet}

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
	{#if addCard}
		<div class="layer">
			<BottomSheet
				title={addCard.title}
				closeLabel={addCard.dismiss}
				dismissible={busy ? false : 'explicit'}
				onclose={onAddNetworkClosed}
			>
				<AddNetworkCard
					card={addCard}
					{busy}
					onadd={onAddNetwork}
					onretry={onAddNetworkRetry}
					ondismiss={onAddNetworkClosed}
					onsetup={openChainSetup}
				/>
			</BottomSheet>
		</div>
	{:else if stage.kind === 'consent' && request}
		<div class="layer">
			<BottomSheet
				title={cardTitle}
				closeLabel={m.cancel}
				dismissible={busy ? false : 'explicit'}
				onclose={onCancel}
			>
				<div class="card">
					{@render consentFacts()}
					<p class="body">{m.body}</p>
					{@render actions()}
				</div>
			</BottomSheet>
		</div>
	{/if}
{:else if showWindowChrome}
	<main>
		{#if addCard}
			<AddNetworkCard
				card={addCard}
				{busy}
				heading
				onadd={onAddNetwork}
				onretry={onAddNetworkRetry}
				ondismiss={onAddNetworkClosed}
				onsetup={openChainSetup}
			/>
		{:else if stage.kind === 'consent' && request && needsWallet}
			<h1>{cardTitle}</h1>
			<p class="body">{m.noWallet}</p>
			<div class="stack">
				<Button variant="primary" shape="rounded" onclick={() => openOnboarding(true)}>
					{m.createWallet}
				</Button>
				<Button variant="secondary" shape="rounded" onclick={() => openOnboarding(false)}>
					{m.haveWallet}
				</Button>
				<Button variant="secondary" shape="rounded" disabled={busy} onclick={onCancel}>
					{m.cancel}
				</Button>
			</div>
		{:else if stage.kind === 'consent' && request}
			<h1>{cardTitle}</h1>
			{@render consentFacts()}
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
	.stack {
		margin-top: auto;
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}
</style>
