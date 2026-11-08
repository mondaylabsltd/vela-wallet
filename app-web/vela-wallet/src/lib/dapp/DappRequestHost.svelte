<script lang="ts">
	import { accountName } from '$lib/multisig/core/account-name';
	import { multisig } from '$lib/multisig/core/store.svelte';
	import { multisigAccountsForExt, multisigAsAccount } from '$lib/multisig/core/ext';
	import { multisigUi } from '$lib/multisig/core/ui.svelte';
	import { holdDappRequest, type DappHold } from '$lib/multisig/shell/dapp-requests.svelte';
	import type { MultisigDappAnswer } from '$lib/multisig/generated/MultisigDappAnswer';
	import type { MultisigHeldRequest } from '$lib/multisig/generated/MultisigHeldRequest';

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
	import { accountLivesOn, publishExtChains, setExtAccountChains } from '$lib/dapp/core/ext-chains';
	import { getOriginChain, originHasOwnChain, watchStartChain } from '$lib/dapp/grants';
	import { balanceView, rereadHoldings } from '$lib/wallet/core/balance-resident';
	import { approve, evaluate, type RequestStage } from '$lib/dapp/request';
	import { signRequest } from '$lib/signing/core/sign-resident.svelte';
	import { AskerGoneError, type ClaimPhase, type SubmitClaim } from '$lib/signing/core/sign-types';
	import BottomSheet from '$lib/wallet/ui/BottomSheet.svelte';
	import Button from '$lib/ui/Button.svelte';
	import ConsentFacts from '$lib/dapp/ConsentFacts.svelte';
	import { identiconSvgForClient } from '$lib/wallet/identicon';
	import { shortenAddress } from '$lib/wallet/identity';
	import { chainName } from '$lib/services/networks';
	import { chainLogoURL } from '$lib/flows/marks';
	import type { RequestMessages } from '$lib/dapp/messages';
	import { multisigRefusalWords } from '$lib/dapp/multisig-refusal';
	import { panelSurface } from '$lib/dapp/panel-surface.svelte';
	import { focusOwnWindow, openInBrowserTab } from '$lib/extension/open-tab';
	import { oneSurface } from '$lib/extension/one-surface.svelte';
	import {
		answerNotShown,
		answerRequest,
		readRequest,
		rejectRequest,
		type ExtensionRequest
	} from '$lib/dapp/transport';
	import AddNetworkCard from '$lib/dapp/AddNetworkCard.svelte';
	import { addNetworkCard } from '$lib/dapp/add-network';
	import { dappAddChainAsk, dappAddOutcomeError, signRequestTtlMs } from '$lib/core/kernels';
	import { poolRpcCall } from '$lib/services/rpc-pool';
	import { networkAdmin, onDappAddSettled } from '$lib/settings/core/network-admin.svelte';
	import type { DappAddOutcome } from '$lib/core/generated/DappAddOutcome';
	import { isSigningMethod } from '$lib/services/dapp-submit';
	import { track } from '$lib/analytics';
	import { dappRequestKind } from '$lib/analytics/dapp';
	import { TX_REQUEST_KINDS } from '$lib/analytics/catalog';

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
	/**
	 * r12: how long a new site's sheet waits for the panel's holdings read
	 * before it says a network — past it, the sheet opens on what is known
	 * and follows the holdings when they land (`watchStartChain`).
	 */
	const START_HOLDINGS_WAIT_MS = 8000;

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

	/**
	 * One wallet per window: a side panel standing aside for a wallet tab
	 * (`oneSurface`) comes back for as long as a request is here — shown,
	 * owed, or landing. A request is answered where spec 082 routes it.
	 */
	$effect(() => {
		if (request === null && owing === null && !landing) return;
		return oneSurface.hold();
	});

	// A multisig shown is the account dApps see, as on the wallet.
	const facts = $derived({
		activeAddress: multisig.active?.address ?? session.view.address,
		addresses: session.view.loading
			? null
			: [
					...session.view.accounts.map((r) => r.account.address),
					...multisig.accounts.map((a) => a.address)
				]
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
		const live = panelSurface.caller
			? panelSurface.claim(rid, phase, submit)
			: Promise.resolve(true);
		if (phase === 'submit') {
			void live.then((yes) => {
				if (yes) submittedRid = rid;
			});
		}
		return live;
	}

	/**
	 * dapp-12: the request whose transaction was claimed for submit — signed,
	 * its bytes on their way. If its page leaves now (a reload right after
	 * Confirm), the transaction goes on regardless: the window stays for its
	 * receipt, so the person sees what happened, instead of closing on a
	 * transfer that is landing.
	 */
	let submittedRid: string | null = null;
	/** How long a window whose page left after submit waits for a landing before it goes. */
	const ORPHAN_LANDING_WAIT_MS = 90_000;

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

			// Safari (r13): nobody is signed in here — said at once, with create
			// and sign in, before any read that needs a wallet (the wasm core,
			// the network lists, the grant). The request waits for the sign-in
			// (`awaitingSignIn`) and is read then, the reads after this line
			// with it. A network to add needs no wallet: it goes on as before.
			if (mode === 'window' && signedOut() && incoming.method !== 'wallet_addEthereumChain') {
				awaitingSignIn = true;
				stage = {
					kind: 'consent',
					origin: incoming.origin,
					method: incoming.method,
					address: null
				};
				return;
			}
			await prepareAndProceed(incoming);
		} finally {
			taking = false;
		}
	}

	/**
	 * What a taken request is read with before its sheet — the snapshot the
	 * worker answers from, the network lists, the start chain — then the
	 * request itself. Also what a request that waited for a sign-in runs
	 * once somebody has signed in.
	 */
	async function prepareAndProceed(incoming: ExtensionRequest): Promise<void> {
		{
			// Publish what the worker will need to answer this origin instantly
			// next time. The core authors it; this only stores it.
			// r11-2: a read before the sheet that throws leaves no request owed
			// with nothing on screen (`notShown`).
			let snapshot: Awaited<ReturnType<typeof publishExtSnapshot>>;
			try {
				[snapshot] = await Promise.all([
					publishExtSnapshot({
						isLoading: session.view.loading,
						hasWallet: session.view.has_wallet,
						accounts: [
							...session.view.accounts.map((row) => row.account),
							...multisigAccountsForExt().accounts
						],
						active:
							multisigAccountsForExt().active ??
							session.view.accounts[session.view.active_index]?.account ??
							null,
						theme: 'dark',
						locale
					}),
					publishExtChains()
				]);
				chainId = await startChainFor(incoming, snapshot);
			} catch (error) {
				await notShown(incoming, error);
				return;
			}
			// Withdrawn while the snapshot was being published.
			if (owing !== incoming.rid) return;

			// Spec 100: not a connect and not a signature — the core's
			// add-network sheet, before the connect logic sees it.
			if (incoming.method === 'wallet_addEthereumChain') {
				await takeAddNetwork(incoming);
				return;
			}

			// dapp-8: nobody is signed in, and the site asks for a signature. Its
			// grant is for an account, and no account is here to sign — which is
			// not "connect first": the window says sign in, and asks the core
			// once somebody has.
			if (mode === 'window' && signedOut() && isSigningMethod(incoming.method)) {
				awaitingSignIn = true;
				stage = {
					kind: 'consent',
					origin: incoming.origin,
					method: incoming.method,
					address: null
				};
				return;
			}
			await proceed(incoming);
		}
	}

	/**
	 * r11-2: `incoming` was taken (owed, reported shown) and its sheet could
	 * not be built. It is not left owed behind a plain wallet: the site hears
	 * that nothing was shown or signed, and this surface takes the next one.
	 */
	async function notShown(incoming: ExtensionRequest, error: unknown): Promise<void> {
		console.warn('[dapp] request not shown:', error);
		if (owing !== incoming.rid) return;
		owing = null;
		request = null;
		stage = { kind: 'loading' };
		await answerNotShown(incoming.rid);
		leave();
	}

	/**
	 * The chain `incoming` is asked on. A site with a network of its own (its
	 * switch, its grant) is on it. A NEW site starts where the account lives
	 * (`site_start_chain_for`), and r12 found that input stale: an account
	 * funded on Arc Testnet a few seconds before the connect was asked on
	 * Sepolia, because no page had read its holdings since the money came and
	 * the grant then kept Sepolia for good. The panel IS the wallet page, so it
	 * reads the account's holdings first (bounded) and stores where it lives;
	 * the window, which reads none, follows the wallet page's write while the
	 * sheet is open (`followStart`).
	 */
	async function startChainFor(
		incoming: ExtensionRequest,
		snapshot: Awaited<ReturnType<typeof publishExtSnapshot>>
	): Promise<number> {
		const fallback = snapshot?.chain_id ?? 0;
		const address = snapshot?.address ?? '';
		if (mode === 'panel' && address && !(await originHasOwnChain(incoming.origin))) {
			await readWhereItLives(address);
		}
		return getOriginChain(incoming.origin, fallback);
	}

	/** One holdings round for `address` (bounded), then where it lives, stored as the wallet page stores it. */
	async function readWhereItLives(address: string): Promise<void> {
		let timer: ReturnType<typeof setTimeout> | undefined;
		await Promise.race([
			rereadHoldings(address),
			new Promise<void>((resolve) => (timer = setTimeout(resolve, START_HOLDINGS_WAIT_MS)))
		]);
		clearTimeout(timer);
		const view = balanceView();
		if (view.address === null || view.address.toLowerCase() !== address.toLowerCase()) return;
		const active = multisig.active;
		const own = active && active.address.toLowerCase() === address.toLowerCase() ? active : null;
		const lives = accountLivesOn(view, own?.chainIds ?? [], multisig.view?.recent_networks ?? []);
		if (lives) await setExtAccountChains(view.address, lives);
	}

	/**
	 * r12: while a new site's consent is open, its network follows where the
	 * account lives — holdings landing in the wallet page, the network last
	 * used — until Connect is pressed (the grant then keeps it).
	 */
	$effect(() => {
		if (stage.kind !== 'consent' || !request) return;
		const asked = request;
		let own: boolean | null = null;
		return watchStartChain(() => {
			void (async () => {
				own ??= await originHasOwnChain(asked.origin);
				if (own || request !== asked || stage.kind !== 'consent' || busy) return;
				const next = await getOriginChain(asked.origin, chainId);
				if (request === asked && stage.kind === 'consent' && !busy && next > 0) chainId = next;
			})();
		});
	});

	/** Ask the core about `incoming` and carry out what it says. */
	async function proceed(incoming: ExtensionRequest): Promise<void> {
		try {
			stage = await evaluate(incoming, facts);
		} catch (error) {
			await notShown(incoming, error);
			return;
		}
		if (stage.kind === 'done' || stage.kind === 'refused') {
			owing = null;
			request = null;
			leave();
		} else if (stage.kind === 'signing') {
			const shared = multisig.find(stage.grantedAddress);
			if (shared) await handOffToMultisig(incoming, shared.address);
			else await handOffToSigning(incoming, stage.grantedAddress);
		}
	}

	/** Nobody is signed in to this wallet (settled). */
	function signedOut(): boolean {
		return !session.view.loading && session.view.allowed_route !== 'wallet';
	}

	/** dapp-8: a signing request waiting for somebody to sign in, in the window. */
	let awaitingSignIn = $state(false);

	/**
	 * dapp-7: a multisig was asked for a message signature, which it cannot
	 * give yet. The window says why before the site is told no; its button
	 * (or any way out) sends the refusal.
	 */
	let multisigNo = $state<{
		rid: string;
		host: string;
		error: { code: number; message: string; data?: unknown };
	} | null>(null);

	/**
	 * What the refusal is said in (`multisigRefusalWords`): §31's parent-Safe
	 * sentence when the core says `data.reason: "parent_safe"` (the site asked
	 * this multisig to act for a Safe it owns), else dapp-7's.
	 */
	const multisigNoWords = $derived(
		multisigRefusalWords(multisigNo?.error, multisigNo?.host ?? '', m)
	);
	const multisigNoTitle = $derived(multisigNoWords.title);
	const multisigNoBody = $derived(multisigNoWords.body);

	/**
	 * The worker withdrew `rid` — its page left, its surface closed, or its
	 * time ran out (RB15). The card closes with no words; a request already
	 * with the signing machine is dropped there, which also stops a pipeline
	 * still short of the passkey (RB2).
	 */
	function withdraw(rid: string, cause = ''): void {
		if (owing !== rid && request?.rid !== rid) return;
		// dapp-12: the page left with the transaction already on its way. The
		// pipeline goes on (the core keeps one past the passkey); this window
		// stays for its receipt, whose Done closes it — or, with no landing
		// after a while, it goes.
		const orphaned = cause === 'page_left' && submittedRid === rid && mode === 'window';
		if (addingChain !== null && request) {
			// The sheet is the settings machine's: close it, settling nothing.
			networkAdmin.dispatch({ type: 'dapp_add_cancelled', tab: String(request.tabId), id: rid });
			addingChain = null;
		}
		if (signingTransport) {
			signRequest.dispatch({ type: 'transport_dropped', transport_id: signingTransport });
			signingTransport = null;
		}
		// A multisig's review goes too (the machine withdraws it, answering
		// nobody); a proposal already made stays in the mailbox for the members.
		const held = multisigHold;
		multisigHold = null;
		multisigNo = null;
		awaitingSignIn = false;
		owing = null;
		held?.leave();
		multisigUi.closeDapp(rid);
		request = null;
		busy = false;
		if (orphaned) {
			setTimeout(() => {
				if (!landing) leave(true);
			}, ORPHAN_LANDING_WAIT_MS);
			return;
		}
		stage = { kind: 'loading' };
		leave();
	}

	onMount(() => {
		if (mode === 'window' && windowRid) void panelSurface.start({ kind: 'window', rid: windowRid });
		const stopWithdrawn = panelSurface.onWithdrawn((rid, cause) => withdraw(rid, cause));
		const stopAdding = onDappAddSettled((_tab, id, outcome) => void addSettled(id, outcome));
		if (mode === 'window') void take();

		// The surface is going away with an answer still owed. The code is NOT
		// this screen's to pick — it is asked of the core, once, in `settleOnClose`
		// — except a multisig's proposal, which is not rejected by the window
		// closing: the site is told it was proposed.
		const settle = () => {
			if (!owing) return;
			const rid = owing;
			owing = null;
			const multisigAnswer = multisigHold?.closing();
			multisigHold = null;
			if (multisigAnswer?.error && multisigAnswer.error.code !== 4001) {
				void answerRequest(rid, { error: multisigAnswer.error }).catch(() => {});
			} else void settleOnClose(rid);
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
		signRequest.syncMultisigs();
		const transportId = signRequest.registerTransport({
			sendResponse: (_id, result, error, opHash) => {
				// The answer goes out FIRST and unconditionally. Everything below
				// is about the chain, and nothing below may delay, swallow or
				// repeat this (spec 077, the invariant).
				owing = null;
				signingTransport = null;
				void answerRequest(incoming.rid, error ? { error } : { result }, opHash);
				// Usage statistics: the person's answer, by request kind and chain
				// — never the site, the message or the transaction, and never for
				// a request that puts a transaction on chain (§35, review 1-8: no
				// event may be linked to one).
				const answered = { kind: dappRequestKind(incoming.method), chain: chainId || undefined };
				if (!TX_REQUEST_KINDS.includes(answered.kind)) {
					if (!error) track('dapp_request_approved', answered);
					else if (error.code === 4001) track('dapp_request_rejected', answered);
				}
				// A request the CORE refuses outright is answered the moment it
				// arrives — so the page never hangs — but the sheet is still
				// explaining WHY; the person's dismissal is what leaves (081).
				if (
					error?.kind === 'self_call_blocked' ||
					error?.kind === 'parent_safe_blocked' ||
					error?.kind === 'sign_in_refused'
				)
					return;
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
		// r10 follow-up: a network-space switch moved this site since its last
		// request — its sheet says so once (the worker took the note off).
		if (incoming.spaceNote && incoming.spaceNote.chainId > 0) {
			signRequest.dispatch({
				type: 'space_noted',
				id: incoming.id,
				chain_id: incoming.spaceNote.chainId
			});
		}
	}

	/**
	 * A multisig's request, held by the proposals machine: what the site is
	 * owed if this surface goes away mid-way (the machine's answer for leaving
	 * it — "proposed, waiting" once it is in the mailbox), and the person's
	 * way out.
	 */
	let multisigHold: DappHold | null = null;

	/**
	 * A request to a multisig (docs/multisig-vault-design.md §6), handed to the
	 * proposals machine (`shell/dapp-requests.svelte.ts`): its review opens
	 * first — what the site asks, read by the core, at the number it would
	 * take — and nothing is signed until the person presses "Propose and
	 * approve" (approved here with a member's passkey, or by a member's own
	 * wallet), asked of the worker first (RB5). The machine holds an
	 * `eth_sendTransaction` until the transaction that executed it — but never
	 * past the extension's limit: before it, the site is told it was proposed
	 * and is waiting (-32000, never a fake hash) — answers a `wallet_sendCalls`
	 * the batch's id once it is proposed (`wallet_getCallsStatus` follows it
	 * from there, in the worker), and answers every way out of the sheet:
	 * Decline or a close before proposing is the person's 4001; a close after
	 * it says it was proposed, and it stays in Activity. A multisig signs no
	 * messages yet (dapp-7: said in the window first).
	 */
	async function handOffToMultisig(incoming: ExtensionRequest, address: string): Promise<void> {
		const rid = incoming.rid;
		const account = multisig.find(address);
		if (!account) return;
		// Where a batch's execution is looked for later: read while it is reviewed.
		const head = incoming.method === 'wallet_sendCalls' ? headBlock(chainId) : undefined;
		const hold = await holdDappRequest({
			rid,
			account,
			method: incoming.method,
			params: incoming.params,
			origin: incoming.origin,
			chainId,
			limitMs: signRequestTtlMs(),
			// RB5: a page that is gone gets nothing signed.
			claim: async () => {
				const live = await claimFor(rid, 'approve');
				if (!live) withdraw(rid);
				return live;
			},
			// dapp-7: said in the window first; the site hears it when the person
			// has read it (or leaves).
			refused: (error) => {
				if (owing === rid) multisigNo = { rid, host: hostLabel(incoming.origin), error };
			},
			answer: (answer, held) => answerMultisig(incoming, answer, held, head)
		});
		if (owing === rid) multisigHold = hold;
		else hold.leave();
	}

	/**
	 * The machine's answer to a multisig's request, to its site — once. A
	 * `wallet_sendCalls` proposed is answered the batch's id, and the worker
	 * is told where to read it from (the multisig, the proposal's number, the
	 * block it was proposed around), so `wallet_getCallsStatus` says pending
	 * until the members execute it. The sheet stays up on the proposal.
	 */
	async function answerMultisig(
		incoming: ExtensionRequest,
		answer: MultisigDappAnswer,
		held: MultisigHeldRequest,
		head: Promise<string | undefined> | undefined
	): Promise<void> {
		const rid = incoming.rid;
		if (owing !== rid) return;
		owing = null;
		request = null;
		multisigHold = null;
		multisigNo = null;
		const batch =
			held.method === 'wallet_sendCalls' && !answer.error && !!held.safe_tx_hash && !!held.nonce;
		if (batch) {
			const fromBlock = await head;
			const nonce = `0x${BigInt(held.nonce ?? '0').toString(16)}`;
			await answerRequest(
				rid,
				{ result: answer.result },
				{ chainId, multisig: { safe: held.safe, nonce, ...(fromBlock ? { fromBlock } : {}) } }
			);
		} else {
			await answerRequest(
				rid,
				answer.error
					? {
							error: {
								code: Number(answer.error.code),
								message: answer.error.message,
								...(answer.error.data !== undefined ? { data: answer.error.data } : {})
							}
						}
					: { result: answer.result }
			);
		}
		leave();
	}

	/** The chain's head block now, or `undefined` when it does not answer within a moment. */
	async function headBlock(chain: number): Promise<string | undefined> {
		const read = poolRpcCall('eth_blockNumber', [], chain)
			.then((answer) => (typeof answer.result === 'string' ? answer.result : undefined))
			.catch(() => undefined);
		const late = new Promise<undefined>((done) => setTimeout(() => done(undefined), 4_000));
		return Promise.race([read, late]);
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

	/**
	 * The sheet's Add Network — or, after "unable to verify", its confirmed
	 * "Add unverified network" (dapp-13): the request must still be live
	 * (RB5), then the core saves (and refuses what it does not allow).
	 */
	async function onAddNetwork(unverified = false): Promise<void> {
		if (!request || busy || addingChain === null) return;
		busy = true;
		const rid = request.rid;
		if (!(await claimFor(rid, 'approve'))) {
			busy = false;
			withdraw(rid);
			return;
		}
		const now_iso = new Date().toISOString();
		if (unverified) networkAdmin.dispatch({ type: 'dapp_add_approved_unverified', now_iso });
		else networkAdmin.dispatch({ type: 'dapp_add_approved', now_iso });
	}

	/** Any way out of the sheet — the core decides what it answers. */
	function onAddNetworkClosed(): void {
		if (busy) return;
		networkAdmin.dispatch({ type: 'dapp_add_declined' });
	}

	function onAddNetworkRetry(): void {
		networkAdmin.dispatch({ type: 'dapp_add_retried' });
	}

	/** Chain Setup, on the chain that was checked — the core's `setup_url`. */
	function openChainSetup(url: string): void {
		window.open(url, '_blank', 'noopener');
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
					// A receipt on screen is the person's to dismiss (its Done).
					if (landing) return;
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
			// dapp-8: the signature it waited for is the core's to rule on now.
			if (awaitingSignIn && request && owing === request.rid) {
				awaitingSignIn = false;
				const waiting = request;
				// Read now what waited for a wallet (the snapshot, the
				// networks, the start chain), then the request.
				void prepareAndProceed(waiting);
			}
		}
	});

	/** dapp-7: the person read why a multisig cannot sign it; the site hears the refusal. */
	function closeMultisigNo(): void {
		const no = multisigNo;
		if (!no || owing !== no.rid) return;
		// The machine answers it its refusal (`answerMultisig`).
		multisigHold?.leave();
	}

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
		// A multisig shown is the account the site sees: named as the wallet names it.
		const shared = multisig.find(address);
		return {
			name: row
				? accountName(row.account.address, row.account.name)
				: shared
					? multisigAsAccount(shared).name
					: '',
			address,
			short: shortenAddress(address),
			identiconSvg: identiconSvgForClient(address)
		};
	});
	const consentNetwork = $derived({ name: chainName(chainId), logoUrl: chainLogoURL(chainId) });
	/**
	 * r10 follow-up: a network-space switch moved this site since its last
	 * request — the consent says so once (`connect.browser.spaceMoved`).
	 */
	const spaceMovedLine = $derived(
		request?.spaceNote && request.spaceNote.chainId > 0
			? m.spaceMoved.replace('{{network}}', chainName(request.spaceNote.chainId))
			: null
	);
	/** dapp-7: a connect for a multisig says what a multisig cannot be asked. */
	const consentMultisig = $derived(
		stage.kind === 'consent' && !!stage.address && !!multisig.find(stage.address)
	);
	/** Unused by the panel; the window draws its own preparing line. */
	const showWindowChrome = $derived(
		mode === 'window' && (stage.kind !== 'signing' || !!multisigNo) && !landing
	);
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
					onadd={() => onAddNetwork()}
					onaddunverified={() => onAddNetwork(true)}
					onretry={onAddNetworkRetry}
					ondismiss={onAddNetworkClosed}
					onsetup={openChainSetup}
				/>
			</BottomSheet>
		</div>
	{:else if multisigNo}
		<div class="layer">
			<BottomSheet
				title={multisigNoTitle}
				closeLabel={m.gotIt}
				dismissible="explicit"
				onclose={closeMultisigNo}
			>
				<div class="card">
					<p class="body" data-testid="multisig-refusal">{multisigNoBody}</p>
					<Button variant="primary" shape="rounded" onclick={closeMultisigNo}>{m.gotIt}</Button>
				</div>
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
					{#if consentMultisig}<p class="body">{m.multisigConnectNote}</p>{/if}
					{#if spaceMovedLine}<p class="body" data-testid="consent-space-note">
							{spaceMovedLine}
						</p>{/if}
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
				onadd={() => onAddNetwork()}
				onaddunverified={() => onAddNetwork(true)}
				onretry={onAddNetworkRetry}
				ondismiss={onAddNetworkClosed}
				onsetup={openChainSetup}
			/>
		{:else if multisigNo}
			<h1>{multisigNoTitle}</h1>
			<p class="body" data-testid="multisig-refusal">{multisigNoBody}</p>
			<div class="stack">
				<Button variant="primary" shape="rounded" onclick={closeMultisigNo}>{m.gotIt}</Button>
			</div>
		{:else if needsWallet && (stage.kind === 'consent' || stage.kind === 'loading')}
			<!-- Nobody is signed in: said from the moment the session knows it
			     (Safari r13: "Preparing wallet…" for 70 s), the request read or not. -->
			<h1 data-testid="request-no-wallet">
				{!request || isSigningMethod(request.method) ? m.signInTitle : cardTitle}
			</h1>
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
			{#if consentMultisig}<p class="body">{m.multisigConnectNote}</p>{/if}
			{#if spaceMovedLine}<p class="body" data-testid="consent-space-note">
					{spaceMovedLine}
				</p>{/if}
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
