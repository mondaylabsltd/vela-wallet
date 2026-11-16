<script lang="ts">
	/**
	 * Vela Connect's window (spec 109): a site opened `wallet.getvela.app/dapp`
	 * — as a popup, or with a redirect request — and asks the web wallet for an
	 * account, a signature or a transaction.
	 *
	 * The extension's request window, without the extension. Every decision is
	 * the core's, exactly as there: `dapp_rpc::classify` routes a request,
	 * `dapp_permissions` rules on a connect and on who may be signed for, and a
	 * signature or transaction goes to the one resident `sign_request` machine
	 * and 026's sheet — the same clear signing, simulation and guards a send
	 * from the wallet itself gets. What this adds is the transport
	 * (`channel.ts`) and the window around the request.
	 *
	 * What a person meets, in order: nothing (an answer that needs nobody —
	 * the chain, the accounts of a connected site, a batch's status), Welcome
	 * (no wallet yet: create one or sign in, in this window, and come back), a
	 * redirect's warning (Vela cannot confirm who made the link), the consent
	 * card (a site not yet connected), the signing sheet. Requests that need a
	 * person are taken one at a time, in the order they came; the rest are
	 * answered as they arrive — a site following its payment
	 * (`wallet_getCallsStatus`) while the receipt is on screen is told at once.
	 *
	 * Nothing is left owed in silence: a person who closes the window before
	 * deciding has declined (4001); a window closed while a signature was on
	 * its way, or a site that left, is the core's "it may have happened"
	 * (4900) — never a clean refusal a site would read as safe to send again.
	 */
	import { onMount, untrack } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import Button from '#lib/ui/Button.svelte';
	import { replaceUrl } from '#lib/ui/history.ts';
	import BrandMark from '#lib/ui/BrandMark.svelte';
	import ConsentFacts from '#lib/dapp/ConsentFacts.svelte';
	import { hostLabel } from '#lib/dapp/host.ts';
	import type { RequestMessages } from '#lib/dapp/messages.ts';
	import {
		approve,
		evaluate,
		type Answerer,
		type RequestStage,
		type WalletFacts
	} from '#lib/dapp/request.ts';
	import { publishExtSnapshot } from '#lib/dapp/core/ext-cache.ts';
	import {
		accountLivesOn,
		publishExtChains,
		setExtAccountChains
	} from '#lib/dapp/core/ext-chains.ts';
	import { getGrant, originHasOwnChain } from '#lib/dapp/grants.ts';
	import { loadCore } from '#lib/core/client.ts';
	import { popupCloseSettlement } from '#lib/dapp/core/dperm-connect.ts';
	import { dpermRejectMessage } from '#lib/dapp/core/dperm-types.ts';
	import { session } from '#lib/session/core/session.svelte.ts';
	import { signRequest } from '#lib/signing/core/sign-resident.svelte.ts';
	import { AskerGoneError } from '#lib/signing/core/sign-types.ts';
	import { balanceView, rereadHoldings } from '#lib/wallet/core/balance-resident.ts';
	import { accountName } from '#lib/multisig/core/account-name.ts';
	import { identiconSvgForClient } from '#lib/wallet/identicon.ts';
	import { shortenAddress } from '#lib/wallet/identity.ts';
	import { chainName } from '#lib/services/networks.ts';
	import { chainLogoURL } from '#lib/flows/marks.ts';
	import { SIGN_IN_PARAM, STAY_PARAM } from '#lib/intro/gate.ts';
	import { handle, originChain } from './answers';
	import {
		PopupChannel,
		RedirectChannel,
		markConnectReturn,
		type Channel,
		type ChannelRequest
	} from './channel';
	import type { ConnectMessages } from './messages';
	import { batchIdOf, rememberOp } from './ops';
	import { readRedirect, redirectRefusal, type WireAnswer } from './protocol';

	interface Props {
		messages: ConnectMessages;
		request: RequestMessages;
		locale: string;
	}

	let { messages: m, request: rm, locale }: Props = $props();

	/** How long a new site's consent waits for the account's holdings before it names a network. */
	const START_HOLDINGS_WAIT_MS = 8000;
	/** How often the window looks for the site that opened it. */
	const OPENER_POLL_MS = 1000;
	/** A redirect's answer waits this long to see whether a transaction lands on screen. */
	const REDIRECT_LANDING_GRACE_MS = 1500;

	/** "The person closed the request without approving it" — the extension's words (dapp-6). */
	const CLOSED_UNDECIDED = {
		code: 4001,
		message: 'The person closed the request without approving it'
	} as const;

	type Stage =
		| { kind: 'boot' }
		/** Opened with no site behind it. */
		| { kind: 'direct' }
		/** A redirect link that cannot be read. */
		| { kind: 'badlink' }
		/** Ready, nothing owed. `answered`: something was, and has been. */
		| { kind: 'idle'; answered: boolean }
		/** The site that opened the window is gone. */
		| { kind: 'gone' }
		/** A redirect answered: on the way back. */
		| { kind: 'returning' }
		/** A request that needs a person, and nobody is signed in. */
		| { kind: 'no-wallet' }
		/** A redirect request: said first, that Vela cannot confirm who asked. */
		| { kind: 'unverified' }
		| { kind: 'consent'; address: string | null }
		| { kind: 'signing' };

	let stage = $state<Stage>({ kind: 'boot' });
	let channel = $state.raw<Channel | null>(null);
	/** The request a person is deciding, if any. */
	let current = $state.raw<ChannelRequest | null>(null);
	/** Requests that need a person, waiting their turn. */
	const queue: ChannelRequest[] = [];
	/** The chain the current request is asked on (the site's). */
	let chainId = $state(0);
	let busy = $state(false);
	/** A transaction is landing in the sheet: the next request waits for its Done. */
	let landing = $state(false);
	/** The signing machine's transport for `current`, while it holds it. */
	let signingTransport: string | null = null;
	/** The person said Continue to a redirect's warning. */
	let unverifiedSeen = false;
	/** Leaving for Welcome: the requests wait, they are not settled. */
	let leavingForWallet = false;
	let disposed = false;
	/** Answers are posted in order, one chain of promises. */
	let instantChain: Promise<void> = Promise.resolve();
	/** The facts published once per window (snapshot, catalog). */
	let published: Promise<void> | null = null;

	const host = $derived(channel?.origin ? hostLabel(channel.origin) : '');
	const fill = (template: string) => template.replaceAll('{{host}}', host);

	/** Who is signed in — the only account a site is shown (spec 109: Vela accounts, not multisigs). */
	const facts = $derived<WalletFacts>({
		activeAddress: session.view.address,
		addresses: session.view.loading ? null : session.view.accounts.map((r) => r.account.address)
	});

	function signedIn(): boolean {
		return (
			!session.view.loading && session.view.allowed_route === 'wallet' && !!session.view.address
		);
	}

	/**
	 * A redirect's answer that needed nobody, held: the site it goes to is not
	 * one the person connected, so going there waits for the person's Continue
	 * (security review 2026-11-17: the wallet's address must not be a link that
	 * sends anyone anywhere without a click).
	 */
	let heldAnswer = $state.raw<{ request: ChannelRequest; answer: WireAnswer } | null>(null);

	/** Send `request`'s answer. */
	const answerTo =
		(request: ChannelRequest): Answerer =>
		async (_rid, answer) => {
			if (channel?.kind === 'redirect' && !unverifiedSeen && !(await getGrant(request.origin))) {
				heldAnswer = { request, answer };
				stage = { kind: 'unverified' };
				return true;
			}
			return (await channel?.answer(request.rid, answer)) ?? false;
		};

	async function send(request: ChannelRequest, answer: WireAnswer): Promise<boolean> {
		return answerTo(request)(request.rid, answer);
	}

	onMount(() => {
		if (PopupChannel.available()) {
			// A window a site opened takes requests from that site alone. A
			// redirect request arriving in it — another page re-pointing the
			// window by its name — is refused, never served under the pin.
			const fragment = readRedirect(location.hash);
			if (fragment.ok || fragment.reason === 'invalid') {
				stage = { kind: 'badlink' };
				return;
			}
			RedirectChannel.forget();
			channel = new PopupChannel();
		} else {
			const opened = RedirectChannel.open(location.hash);
			if (opened === 'invalid') {
				stage = { kind: 'badlink' };
				return;
			}
			if (!opened) {
				stage = { kind: 'direct' };
				return;
			}
			// The request is kept in this window's storage: the fragment goes, so
			// a reload or a shared screenshot of the address bar carries nothing.
			void replaceUrl(location.pathname + location.search);
			channel = opened;
		}
		const live = channel;
		stage = { kind: 'idle', answered: false };
		live.start(arrive);

		const poll =
			live.kind === 'popup'
				? setInterval(() => {
						if (!live.alive()) siteGone();
					}, OPENER_POLL_MS)
				: undefined;
		window.addEventListener('pagehide', settleAll);
		return () => {
			disposed = true;
			if (poll) clearInterval(poll);
			window.removeEventListener('pagehide', settleAll);
			live.stop();
			settleAll();
		};
	});

	/** Requests taken, not yet answered or queued: owed an answer if the window goes. */
	// Not reactive state: nothing on screen reads it.
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	const routing = new Set<ChannelRequest>();

	/** A request arrived: answer it, or queue it for a person. */
	function arrive(request: ChannelRequest): void {
		routing.add(request);
		instantChain = instantChain
			.then(() => route(request))
			.finally(() => routing.delete(request))
			.catch((error) => {
				console.warn('[connect] request failed:', error);
				void send(request, {
					error: {
						code: -32603,
						message: 'Vela could not answer this request. Nothing was signed.'
					}
				});
			});
	}

	async function route(request: ChannelRequest): Promise<void> {
		await Promise.all([session.settled(), loadCore()]);
		await publishFacts();
		const refused = channel?.kind === 'redirect' ? redirectRefusal(request.method) : null;
		const handling = refused
			? ({ kind: 'answer', answer: { error: { code: 4200, message: refused } } } as const)
			: await handle(request, request.origin, signedIn() ? session.view.address : null);
		if (handling.kind === 'answer') {
			await send(request, handling.answer);
			if (channel?.kind === 'redirect') {
				if (!heldAnswer) stage = { kind: 'returning' };
			} else if (!current && stage.kind === 'idle') stage = { kind: 'idle', answered: true };
			return;
		}
		queue.push(request);
		void takeNext();
	}

	/**
	 * What the core reads a site's chain from: the snapshot (who is signed in,
	 * on which chain) and the catalog (the networks of each space). Published
	 * once per window — this IS the wallet, so it reads them first-hand.
	 */
	function publishFacts(): Promise<void> {
		published ??= (async () => {
			await Promise.all([
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
		})().catch((error) => {
			published = null;
			throw error;
		});
		return published;
	}

	/** Take the next request a person decides — never over one in hand, or a landing. */
	async function takeNext(): Promise<void> {
		if (disposed || current || landing || busy) return;
		const next = queue.shift();
		if (!next) return;
		current = next;
		unverifiedSeen = channel?.verified ?? false;
		await proceed(next);
	}

	async function proceed(request: ChannelRequest): Promise<void> {
		if (current !== request) return;
		if (!signedIn()) {
			stage = { kind: 'no-wallet' };
			return;
		}
		chainId = await startChain(request.origin);
		if (current !== request) return;
		let ruled: RequestStage;
		try {
			// The core answers what needs nobody (a site already connected asking
			// for its account) itself, to the requester; anything else waits.
			ruled = await evaluate(request, facts, answerTo(request));
		} catch (error) {
			console.warn('[connect] request not shown:', error);
			await finish(request, {
				error: {
					code: -32603,
					message: 'Vela could not show this request. Nothing was signed — try again.'
				}
			});
			return;
		}
		if (current !== request) return;
		if ((ruled.kind === 'consent' || ruled.kind === 'signing') && !unverifiedSeen) {
			// A redirect: before the person decides anything, Vela says it cannot
			// confirm who sent the link (B10).
			heldRuling = ruled;
			stage = { kind: 'unverified' };
			return;
		}
		await carryOut(request, ruled);
	}

	/** What the core ruled for `request`, waiting on a redirect's warning. */
	let heldRuling: RequestStage | null = null;

	async function carryOut(request: ChannelRequest, ruled: RequestStage): Promise<void> {
		switch (ruled.kind) {
			case 'done':
			case 'refused':
				// The core answered it already.
				done(request);
				return;
			case 'consent':
				stage = { kind: 'consent', address: ruled.address };
				return;
			case 'signing':
				stage = { kind: 'signing' };
				await handOffToSigning(request, ruled.grantedAddress);
				return;
			default:
				stage = { kind: 'consent', address: null };
		}
	}

	/**
	 * The chain `origin` is asked on. A site with a network of its own is on
	 * it; a NEW one starts where the account lives (r12), so this window reads
	 * the account's holdings first — bounded — as the extension's panel does.
	 */
	async function startChain(origin: string): Promise<number> {
		const address = session.view.address;
		if (address && !(await originHasOwnChain(origin))) {
			let timer: ReturnType<typeof setTimeout> | undefined;
			await Promise.race([
				rereadHoldings(address).catch(() => {}),
				new Promise<void>((done) => (timer = setTimeout(done, START_HOLDINGS_WAIT_MS)))
			]);
			clearTimeout(timer);
			const view = balanceView();
			if (view.address?.toLowerCase() === address.toLowerCase()) {
				const lives = accountLivesOn(view);
				if (lives) await setExtAccountChains(address, lives);
			}
		}
		return originChain(origin);
	}

	/** Hand `request` to the one signing machine; this window is its transport. */
	async function handOffToSigning(request: ChannelRequest, grantedAddress: string): Promise<void> {
		await signRequest.boot();
		signRequest.syncNetworks();
		signRequest.syncAccounts();
		signRequest.syncMultisigs();
		const asked = chainId;
		const transportId = signRequest.registerTransport({
			sendResponse: (_id, result, error, opHash) => {
				// The answer goes out first, whatever follows (spec 077's invariant).
				signingTransport = null;
				if (!error && request.method === 'wallet_sendCalls') {
					const id = batchIdOf(result);
					if (id) rememberOp(id, opHash?.chainId ?? asked, request.origin);
				}
				void finish(
					request,
					error ? { error: { code: error.code, message: error.message } } : { result }
				);
			},
			// RB5: a site that is gone gets nothing signed and nothing sent.
			claim: async () => channel?.alive() ?? false
		});
		signingTransport = transportId;
		signRequest.dispatch({
			type: 'request_arrived',
			id: request.id,
			method: request.method,
			params_json: JSON.stringify(request.params),
			origin: request.origin,
			transport_id: transportId,
			dedicated_transport: true,
			per_request_chain: asked > 0 ? asked : null,
			dapp: null,
			// The signature is pinned to the GRANT's address (invariant ⑨).
			granted_address: grantedAddress,
			requested_address: null,
			request_ts_ms: null,
			now_ms: Date.now(),
			first_party: false
		});
	}

	/** Answer `request` and move on. */
	async function finish(request: ChannelRequest, answer: WireAnswer): Promise<void> {
		if (channel?.kind === 'redirect') {
			// The answer IS the way back. A transaction landing on screen is
			// watched to its end first — the receipt's Done takes the person back.
			await new Promise((r) => setTimeout(r, answer.error ? 0 : REDIRECT_LANDING_GRACE_MS));
			if (landing) {
				pendingReturn = { request, answer };
				return;
			}
			stage = { kind: 'returning' };
			await send(request, answer);
			return;
		}
		await send(request, answer);
		done(request);
	}

	/** A redirect's answer, held while its transaction lands. */
	let pendingReturn: { request: ChannelRequest; answer: WireAnswer } | null = null;

	function done(request: ChannelRequest): void {
		if (current !== request) return;
		current = null;
		busy = false;
		if (channel?.kind === 'redirect') {
			stage = { kind: 'returning' };
			return;
		}
		if (stage.kind !== 'gone') stage = { kind: 'idle', answered: true };
		void takeNext();
	}

	/** The sheet raised a landing (the page owns the one `SigningHost`). */
	export function noteLanding(): void {
		landing = true;
	}

	/** The person pressed Done on the landing. */
	export function noteReceiptDone(): void {
		landing = false;
		const held = pendingReturn;
		pendingReturn = null;
		if (held) {
			stage = { kind: 'returning' };
			void send(held.request, held.answer);
			return;
		}
		void takeNext();
	}

	/** The site that opened this window left: stop asking the person anything for it. */
	function siteGone(): void {
		if (stage.kind === 'gone') return;
		if (signingTransport) {
			signRequest.dispatch({ type: 'transport_dropped', transport_id: signingTransport });
			signingTransport = null;
		}
		queue.length = 0;
		current = null;
		busy = false;
		// A transaction landing stays on screen; its Done brings the person here.
		stage = { kind: 'gone' };
	}

	/**
	 * The window is going with answers owed. Never silence: undecided is the
	 * person declining (4001); a signature in progress may have happened — the
	 * core's code for a window closed on it (4900). A redirect answers by
	 * navigating, so it has nobody to tell.
	 */
	function settleAll(): void {
		if (leavingForWallet || !channel || channel.kind !== 'popup') return;
		const owed = current;
		const signing = stage.kind === 'signing' && owed !== null;
		if (signingTransport) {
			signRequest.dispatch({ type: 'transport_dropped', transport_id: signingTransport });
			signingTransport = null;
		}
		if (owed) {
			let answer: WireAnswer = { error: { ...CLOSED_UNDECIDED } };
			if (signing) {
				try {
					const settlement = popupCloseSettlement();
					answer = {
						error: { code: settlement.code, message: dpermRejectMessage(settlement.reason) }
					};
				} catch {
					// No core to ask: still "it may have happened", never a clean refusal.
					answer = { error: { code: 4900, message: dpermRejectMessage('browser_closed') } };
				}
			}
			void send(owed, answer);
		}
		// Queued, or still being read (never shown): nothing was signed or sent.
		for (const waiting of [...queue.splice(0), ...routing]) {
			if (waiting !== owed) void send(waiting, { error: { ...CLOSED_UNDECIDED } });
		}
		routing.clear();
		current = null;
	}

	async function onConnect(): Promise<void> {
		const request = current;
		if (!request || busy) return;
		busy = true;
		try {
			await approve(
				request,
				facts,
				chainId,
				async () => channel?.alive() ?? false,
				answerTo(request)
			);
			done(request);
		} catch (error) {
			if (error instanceof AskerGoneError) {
				siteGone();
				return;
			}
			// The core did not sanction it: nothing written, nothing sent.
			busy = false;
		}
	}

	async function onDecline(): Promise<void> {
		const declined = { error: { code: 4001, message: 'User rejected the request' } };
		const held = heldAnswer;
		if (held) {
			// A redirect's answer the person would not follow: they declined.
			heldAnswer = null;
			unverifiedSeen = true;
			stage = { kind: 'returning' };
			await send(held.request, declined);
			return;
		}
		const request = current;
		if (!request || busy) return;
		busy = true;
		await finish(request, declined);
	}

	function onContinueUnverified(): void {
		unverifiedSeen = true;
		const held = heldAnswer;
		if (held) {
			heldAnswer = null;
			stage = { kind: 'returning' };
			void send(held.request, held.answer);
			return;
		}
		const request = current;
		const ruled = heldRuling;
		if (!request || !ruled) return;
		heldRuling = null;
		void carryOut(request, ruled);
	}

	/** Create a wallet or sign in, in this window; the wallet sends it back here. */
	function toWallet(create: boolean): void {
		leavingForWallet = true;
		markConnectReturn();
		if (create) {
			void goto(resolve('/[locale]/create', { locale }));
			return;
		}
		// Welcome with its way-in sheet up, as `/import` sends a person there.
		const welcome = resolve('/[locale]', { locale });
		// eslint-disable-next-line svelte/no-navigation-without-resolve -- Welcome, resolved above, with its query
		void goto(`${welcome}?${STAY_PARAM}&${SIGN_IN_PARAM}`);
	}

	function closeWindow(): void {
		window.close();
	}

	/** Signed in elsewhere while a request waited (another tab of this wallet). */
	$effect(() => {
		const ready = signedIn();
		untrack(() => {
			if (ready && stage.kind === 'no-wallet' && current) void proceed(current);
		});
	});

	const consentAccount = $derived.by(() => {
		const address = stage.kind === 'consent' ? stage.address : null;
		if (!address) return null;
		const row = session.view.accounts.find(
			(r) => r.account.address.toLowerCase() === address.toLowerCase()
		);
		return {
			name: row ? accountName(row.account.address, row.account.name) : '',
			address,
			short: shortenAddress(address),
			identiconSvg: identiconSvgForClient(address)
		};
	});
	const consentNetwork = $derived({ name: chainName(chainId), logoUrl: chainLogoURL(chainId) });
</script>

<!--
	The signing sheet is the whole window while a request is in it, and while a
	transaction lands; this page shows only when neither is up.
-->
{#if (stage.kind !== 'signing' || !current) && !landing}
	<main data-testid="connect-window" data-stage={stage.kind}>
		<header class="brand"><BrandMark /></header>

		{#if stage.kind === 'boot'}
			<p class="body">{rm.preparing}</p>
		{:else if stage.kind === 'direct'}
			<h1>{m.directTitle}</h1>
			<p class="body">{m.directBody}</p>
			<div class="stack">
				<Button variant="primary" shape="rounded" href={resolve('/[locale]/wallet', { locale })}>
					{m.openWallet}
				</Button>
			</div>
		{:else if stage.kind === 'badlink'}
			<h1>{m.badLinkTitle}</h1>
			<p class="body">{m.badLinkBody}</p>
		{:else if stage.kind === 'gone'}
			<h1>{fill(m.goneTitle)}</h1>
			<p class="body">{m.goneBody}</p>
			<div class="stack">
				<Button variant="primary" shape="rounded" onclick={closeWindow}>{m.close}</Button>
			</div>
		{:else if stage.kind === 'returning'}
			<p class="body" role="status">{fill(m.returning)}</p>
		{:else if stage.kind === 'no-wallet' && current}
			<h1 data-testid="connect-no-wallet">
				{current.method === 'eth_requestAccounts' || current.method === 'wallet_requestPermissions'
					? fill(rm.title)
					: rm.signInTitle}
			</h1>
			<p class="body">{rm.noWallet}</p>
			<div class="stack">
				<Button variant="primary" shape="rounded" onclick={() => toWallet(true)}>
					{rm.createWallet}
				</Button>
				<Button variant="secondary" shape="rounded" onclick={() => toWallet(false)}>
					{rm.haveWallet}
				</Button>
				<Button variant="secondary" shape="rounded" disabled={busy} onclick={onDecline}>
					{rm.cancel}
				</Button>
			</div>
		{:else if stage.kind === 'unverified' && (current || heldAnswer)}
			<h1>{m.unverifiedTitle}</h1>
			<p class="body" data-testid="connect-unverified">{fill(m.unverifiedBody)}</p>
			<div class="actions">
				<Button variant="secondary" shape="rounded" disabled={busy} onclick={onDecline}>
					{rm.cancel}
				</Button>
				<Button variant="primary" shape="rounded" onclick={onContinueUnverified}>
					{m.continueLabel}
				</Button>
			</div>
		{:else if stage.kind === 'consent' && current}
			<h1>{fill(rm.title)}</h1>
			<ConsentFacts
				accountLabel={rm.accountLabel}
				networkLabel={rm.networkLabel}
				account={consentAccount}
				network={consentNetwork}
			/>
			<p class="body">{rm.body}</p>
			<div class="actions">
				<Button variant="secondary" shape="rounded" disabled={busy} onclick={onDecline}>
					{rm.cancel}
				</Button>
				<Button variant="primary" shape="rounded" loading={busy} onclick={onConnect}>
					{rm.connect}
				</Button>
			</div>
		{:else if stage.kind === 'idle' && stage.answered && host}
			<h1>{fill(m.doneTitle)}</h1>
			<p class="body">{fill(m.doneBody)}</p>
			<div class="stack">
				<Button variant="primary" shape="rounded" onclick={closeWindow}>{m.close}</Button>
			</div>
		{:else}
			<p class="body" role="status">{rm.preparing}</p>
		{/if}
	</main>
{/if}

<style>
	main {
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
		padding: var(--space-3xl);
		min-height: 100vh;
		box-sizing: border-box;
		background: var(--color-bg-base);
		color: var(--color-fg-base);
	}
	.brand {
		display: flex;
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
