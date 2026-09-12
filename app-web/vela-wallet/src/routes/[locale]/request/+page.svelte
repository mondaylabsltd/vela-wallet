<script lang="ts">
	/**
	 * The surface a dApp's request is answered in (spec 027 T322/T334).
	 *
	 * Two doors, one page. A request a person clicked for opens in the SIDE
	 * PANEL of the tab that asked (no `rid` in the URL: the panel asks the
	 * worker what that tab owes). A request a page fired on its own — no user
	 * gesture, so no panel — opens in a dedicated window (`?rid=`). Neither is
	 * the action popup, which would be dismissed the moment a passkey prompt
	 * takes focus (D34), and neither is an in-page sheet the site could style,
	 * cover or scroll.
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
	 * In the panel, one request is one page load: when an answer has gone out
	 * and the tab owes another, the page reloads into it — a fresh core, a fresh
	 * fee session (026's one-owner rule), nothing carried over.
	 */
	import { onMount } from 'svelte';
	import { session } from '$lib/session/core/session.svelte';
	import { hostLabel } from '$lib/dapp/host';
	import { publishExtSnapshot } from '$lib/dapp/core/ext-cache';
	import { publishExtChains } from '$lib/dapp/core/ext-chains';
	import { getOriginChain } from '$lib/dapp/grants';
	import { approve, evaluate, type RequestStage } from '$lib/dapp/request';
	import SigningHost from '$lib/signing/SigningHost.svelte';
	import { signRequest } from '$lib/signing/core/sign-resident.svelte';
	import { FeeQuote } from '$lib/flows/core/fee-quote.svelte';
	import {
		answerRequest,
		currentPanelRequest,
		panelDone,
		panelTabId,
		readRequest,
		rejectRequest,
		type ExtensionRequest
	} from '$lib/dapp/transport';

	let { data } = $props();
	const m = $derived(data.requestMessages);

	let request = $state<ExtensionRequest | null>(null);
	let stage = $state<RequestStage>({ kind: 'loading' });
	/** The chain the SITE is on — what the grant records and a signature is asked on. */
	let chainId = $state(0);
	let busy = $state(false);
	/** Panel mode: no `rid` in the URL; the tab is asked for instead. */
	let panel = $state(false);
	let tabId: number | undefined;
	/**
	 * ONE live fee session for this window (026's rule): the quote the core
	 * pre-checks against, the quote on screen and the quote that is signed are
	 * one object with one owner.
	 */
	const feeQuote = new FeeQuote();
	/** Cleared the moment an answer goes out, so teardown owes nothing. */
	let owing = $state<string | null>(null);

	const facts = $derived({
		activeAddress: session.view.address,
		addresses: session.view.loading ? null : session.view.accounts.map((r) => r.account.address)
	});

	onMount(() => {
		const rid = new URLSearchParams(location.search).get('rid') ?? '';
		panel = rid === '';
		let disposed = false;

		void (async () => {
			await session.boot();
			let incoming: ExtensionRequest | null;
			if (panel) {
				tabId = await panelTabId();
				incoming = await currentPanelRequest(tabId);
			} else {
				incoming = await readRequest(rid);
			}
			if (disposed) return;
			if (!incoming) {
				stage = { kind: 'refused', code: 4900, message: 'This request is no longer available' };
				closeSoon();
				return;
			}
			request = incoming;
			owing = incoming.rid;

			// Publish what the worker will need to answer this origin instantly
			// next time. The core authors it; this window only stores it.
			const [snapshot] = await Promise.all([
				publishExtSnapshot({
					isLoading: session.view.loading,
					hasWallet: session.view.has_wallet,
					accounts: session.view.accounts.map((row) => row.account),
					active: session.view.accounts[session.view.active_index]?.account ?? null,
					theme: 'dark',
					locale: data.locale ?? 'en'
				}),
				publishExtChains()
			]);
			chainId = await getOriginChain(incoming.origin, snapshot?.chain_id ?? 0);

			stage = await evaluate(incoming, facts);
			if (stage.kind === 'done' || stage.kind === 'refused') {
				owing = null;
				closeSoon();
			} else if (stage.kind === 'signing') {
				// The core said this origin may be answered, and named the address
				// the signature must be pinned to. Hand the request to
				// `sign_request` — the SAME machine and the SAME sheet the wallet's
				// own screens use — on a transport that answers this window's page.
				await handOffToSigning(incoming, stage.grantedAddress);
			}
		})();

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
			window.removeEventListener('pagehide', settle);
			settle();
		};
	});

	/**
	 * Register this window as the transport and deliver the request.
	 *
	 * The core speaks a `transport_id` and nothing else about transports: a
	 * response goes to the transport that OWNS the request, never a shared
	 * reference. This window owns exactly one, and answering through it is what
	 * clears what the window owes — so the teardown settlement no longer fires.
	 */
	async function handOffToSigning(incoming: ExtensionRequest, grantedAddress: string) {
		await signRequest.boot();
		signRequest.syncNetworks();
		signRequest.syncAccounts();
		const transportId = signRequest.registerTransport({
			sendResponse: (_id, result, error) => {
				owing = null;
				void answerRequest(incoming.rid, error ? { error } : { result });
				closeSoon();
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
	 * Leave — after a short delay, so the answer reaches the page before the
	 * document goes away. In the panel, first ask whether the tab owes another
	 * answer: if so, reload into it (a fresh page per request); if not, the
	 * worker dismisses the panel and this page closes itself as a backstop.
	 */
	function closeSoon(): void {
		setTimeout(() => {
			if (!panel) {
				window.close();
				return;
			}
			void currentPanelRequest(tabId).then((next) => {
				if (next) {
					location.reload();
					return;
				}
				void panelDone(tabId);
				window.close();
			});
		}, 400);
	}

	async function onConnect(): Promise<void> {
		if (!request || busy) return;
		busy = true;
		try {
			await approve(request, facts, chainId);
			owing = null;
			stage = { kind: 'done' };
			closeSoon();
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
		closeSoon();
	}
</script>

<svelte:head><title>Vela</title></svelte:head>

{#if stage.kind !== 'signing'}
	<main>
		{#if stage.kind === 'consent' && request}
			<h1>{m.title.replace('{{host}}', hostLabel(request.origin))}</h1>
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

<!--
	026's sheet, unchanged: the same four machines, the same 13 block kinds, the
	same never-unlimited guard. Dismissing it rejects, and the core answers this
	window's transport.
-->
<SigningHost messages={data.signingMessages} fee={feeQuote} />

<style>
	main {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		padding: var(--space-6);
		min-height: 100vh;
		background: var(--color-bg-base);
		color: var(--color-text-primary);
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
