<script lang="ts">
	/**
	 * The wallet a signed-in person lands in — the web's answer to the iOS root
	 * view and the desktop's `SessionRoute::Wallet` branch (spec 019).
	 *
	 * Three things happen here and nowhere else on the web:
	 *
	 * 1. **The guard.** The core decides WHAT is allowed (`allowed_route`); this
	 *    page decides when to move, exactly as the native shells do. A browser
	 *    with no wallet is sent back to Welcome instead of being shown a wallet
	 *    body it has no business seeing — so nothing renders until the machine
	 *    has actually said `wallet`.
	 * 2. **The identity.** Name, address and identicon come from the session,
	 *    over the top of the fixture model. The identicon is rendered in the
	 *    BROWSER through vela-core, which is already loaded here: the session
	 *    machine that holds the address is that same module. Welcome stays
	 *    wasm-free; this page never could be.
	 * 3. **The way out.** The Settings tab opens the settings screen, and the
	 *    退出登录 row inside it signs out. Until spec 023 there was no such
	 *    screen, so the tab itself was the sign-out — which meant tapping
	 *    设置 to change your language logged you out instead.
	 */
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { MediaQuery } from 'svelte/reactivity';
	import WalletDesktop from '$lib/wallet/WalletDesktop.svelte';
	import WalletHome from '$lib/wallet/WalletHome.svelte';
	import SignOutHost from '$lib/session/ui/SignOutHost.svelte';
	import AccountSwitcher from '$lib/session/ui/AccountSwitcher.svelte';
	import IdenticonViewerHost from '$lib/wallet/ui/IdenticonViewerHost.svelte';
	import BottomSheet from '$lib/wallet/ui/BottomSheet.svelte';
	import ChainFilterList from '$lib/wallet/ui/ChainFilterList.svelte';
	import Dialog from '$lib/settings/ui/Dialog.svelte';
	import RpcFixBody from '$lib/settings/ui/RpcFixBody.svelte';
	import BalanceDetailBody from '$lib/settings/ui/BalanceDetailBody.svelte';
	import RelayerBody from '$lib/settings/ui/RelayerBody.svelte';
	import { liveBalanceDetail, liveRelayer, liveRpcFix } from '$lib/settings/live';
	import { networkAdmin } from '$lib/settings/core/network-admin.svelte';
	import { BREAKPOINT_DESKTOP } from '$lib/tokens/tokens';
	import { session } from '$lib/session/core/session.svelte';
	import { createContactsSession, type ContactsSession } from '$lib/contacts/core/contacts';
	import type { ContactGroupView } from '$lib/core/generated/ContactGroupView';
	import type { ContactsView } from '$lib/core/generated/ContactsView';
	import { readFlowHandoff } from '$lib/flows/contact-handoff';
	import { preferences } from '$lib/services/preferences.svelte';
	import { publishExtSnapshot } from '$lib/dapp/core/ext-cache';
	import { publishExtChains } from '$lib/dapp/core/ext-chains';
	import { followActiveAccount } from '$lib/dapp/follow';
	import { subscribeNetworks } from '$lib/services/networks';
	import { inExtension } from '$lib/dapp/transport';
	import { avatarSvgForClient } from '$lib/wallet/identicon';
	import { desktopWithIdentity, homeWithIdentity, type WalletIdentity } from '$lib/wallet/identity';
	import FlowsMobile from '$lib/flows/FlowsMobile.svelte';
	import FlowsPanel from '$lib/flows/FlowsPanel.svelte';
	import ScanSurface from '$lib/flows/ui/ScanSurface.svelte';
	import { FlowNav, type FlowEntry } from '$lib/flows/nav.svelte';
	import { balance } from '$lib/wallet/core/balance.svelte';
	import { feed } from '$lib/wallet/core/feed.svelte';
	import {
		createReceiveWatchSession,
		type ReceiveWatchSession
	} from '$lib/flows/core/receive-watch';
	import { loadCore } from '$lib/core/client';
	import { currency } from '$lib/settings/core/currency.svelte';
	import { withLiveWallet, withLiveWalletDesktop } from '$lib/wallet/live';
	import {
		receiveNetworks,
		visibleBalanceTokens,
		withLiveDesktopFlow,
		withLiveFlow
	} from '$lib/flows/live';
	import { createSendSession, type SendSession } from '$lib/flows/core/send-session';
	import { createBatchImportSession, type BatchImportSession } from '$lib/flows/core/batch-session';
	import {
		createManageTokensSession,
		type ManageTokensSession
	} from '$lib/wallet/core/manage-tokens-session';
	import type { MtokView } from '$lib/core/generated/MtokView';
	import { getAllNetworksSync, getCustomChainIdsSync, networkId } from '$lib/services/networks';
	import {
		makeRecipientId,
		sendTokenId,
		visibleSendTokens,
		type SendClassFilter
	} from '$lib/flows/live-send';
	import { prefetchForSend } from '$lib/services/safe-transaction';
	import type { BatchView } from '$lib/core/generated/BatchView';
	import { FeeQuote, IDLE_FEE_VIEW } from '$lib/flows/core/fee-quote.svelte';
	import { scanner, scanNotice } from '$lib/flows/core/scanner.svelte';
	import { isHexAddress, parseEIP681 } from '$lib/services/eip681';
	import { setSendTrackerSink } from '$lib/flows/core/send-executor';
	import { startTxTracker, trackSubmitted } from '$lib/wallet/core/tracker-resident';
	import SigningHost from '$lib/signing/SigningHost.svelte';
	import { signRequest } from '$lib/signing/core/sign-resident.svelte';
	import type { SendOpenParams } from '$lib/core/generated/SendOpenParams';
	import type { SendAlertKind } from '$lib/core/generated/SendAlertKind';
	import type { SendView } from '$lib/core/generated/SendView';

	import { WEB_DESTINATIONS, webNavItems } from '$lib/wallet/destinations';
	import { chainFilter } from '$lib/wallet/chain-filter.svelte';
	import { balanceTokenId, liveChainRows, narrowedFeed } from '$lib/wallet/live';
	import {
		feedItemAt,
		findFeedItem,
		liveTxDetail,
		shownTxDetailStateDesktop,
		shownTxDetailStateMobile,
		withLiveTxDetailDesktop,
		withLiveTxDetailMobile
	} from '$lib/wallet/live-detail';
	import type { PageProps } from './$types';

	/** The sidebar's copy of the rule in `destinations.ts`: three rows, not four. */
	function webNav(model: typeof data.desktop) {
		return { ...model, sidebar: { ...model.sidebar, nav: webNavItems(model.sidebar.nav) } };
	}

	let { data }: PageProps = $props();

	const welcome = $derived(resolve('/[locale]', { locale: data.locale }));
	const createHref = $derived(resolve('/[locale]/create', { locale: data.locale }));
	const settings = $derived(resolve('/[locale]/settings', { locale: data.locale }));
	const contactsHref = $derived(resolve('/[locale]/contacts', { locale: data.locale }));
	const wide = new MediaQuery(`(min-width: ${BREAKPOINT_DESKTOP}px)`, false);

	const view = $derived(session.view);
	const signedIn = $derived(view.allowed_route === 'wallet');

	/**
	 * Whose wallet this is. `address` rides in the view pre-derived; the name
	 * does not, so it is read from the active row — and the identicon is
	 * rendered from the address, never from the name.
	 */
	const identity = $derived<WalletIdentity | null>(
		signedIn
			? {
					name: view.accounts[view.active_index]?.account.name ?? '',
					address: view.address,
					identiconSvg: avatarSvgForClient(
						view.address,
						view.accounts[view.active_index]?.account.name ?? ''
					)
				}
			: null
	);

	/**
	 * The account switcher (founder call, 2026-09-05), opened from the header's
	 * name button on either layout. The identicon viewer needs no flag here any
	 * more: every artwork opens it through the resident store, and the host at
	 * the foot of this page draws it.
	 */
	let switching = $state(false);

	/**
	 * The third column's own two subjects (spec 015 D3 and 021 A2, live): the
	 * held token whose detail is open, and the feed item whose detail the
	 * flow shows. Keys, not copies — the live models re-derive from the
	 * stores, so a balance refresh updates an open detail rather than
	 * stranding a snapshot.
	 */
	let selectedAssetId = $state<string | null>(null);
	let selectedTxId = $state<string | null>(null);

	/**
	 * Spec 038 #E9: the networks the person added, listed in the filter from
	 * the moment they exist rather than from the first balance that lands on
	 * them. `networksVersion` ticks when the set changes.
	 */
	let networksVersion = $state(0);
	$effect(() => subscribeNetworks(() => (networksVersion += 1)));
	const customChainIds = $derived.by(() => {
		void networksVersion;
		return getCustomChainIdsSync();
	});
	/**
	 * The network whose code the receive screen shows (spec 028 Phase 9, T482):
	 * the row that was tapped, or the sidebar's filter (T495). The list and the
	 * page walk the same `receiveNetworks()` order, so an index names a chain.
	 */
	let selectedReceiveChainId = $state<number | null>(null);
	const selectedTx = $derived(findFeedItem(feed.view, selectedTxId));
	const txDetail = $derived(
		selectedTx === undefined
			? undefined
			: liveTxDetail(selectedTx, {
					m: data.flowMessages,
					wm: data.walletMessages,
					currency: currency.view,
					hidden: balance.view.hidden,
					identicon: (seed) => avatarSvgForClient(seed, '')
				})
	);

	/**
	 * Spec 021: Receive / Send / Activity / Assets, as pushed screens inside
	 * this route. `flows` and `desktopFlows` arrive prerendered from `load`;
	 * this only decides which one is showing.
	 */
	const nav = new FlowNav();

	// --- The send flow (spec 026) ---------------------------------------------
	//
	// One `send` session per visit to the flow, and ONE `fee_policy` session
	// beside it: the quote the core pre-checks against, the quote on screen and
	// the quote that is signed are one object with one owner. The core's own
	// `stage` decides which screen shows — the nav stack is not consulted while
	// a send is live, because the machine already knows where the person is.

	let sendView = $state<SendView | null>(null);
	let sendSession: SendSession | null = null;
	const feeQuote = new FeeQuote();
	/** The fee-coin sheet is a shell surface: the core has no state for it. */
	let feeSheetOpen = $state(false);
	/**
	 * The picker is choosing SEVERAL tokens (spec 028 T440). Shell state by
	 * precedent — the phone's `sweepActive` is its chain filter, a shell
	 * value too — because the core's `multi_select_mode` only flips when the
	 * selection is CONFIRMED. Everything the flag reveals (which rows are
	 * ticked, which are off-chain, what "all valuable" means, what the sweep
	 * moves) is the core's.
	 */
	let sweepPicking = $state(false);
	/**
	 * SD1's class chips (spec 028 Phase 10) — shell render state like the
	 * sidebar's chain filter. Both narrow the SAME list the picker's indices
	 * point into (`visibleSendTokens`), so a tap names the row that was tapped.
	 */
	let sendClassFilter = $state<SendClassFilter>('all');
	const sendVisible = $derived(
		sendView === null
			? []
			: visibleSendTokens(sendView, {
					chainFilter: chainFilter.chainId,
					classFilter: sendClassFilter
				})
	);

	// --- The batch importer (spec 026 US3) ------------------------------------
	//
	// Its own machine, opened with the sheet and disposed with it. The parse,
	// the duplicate check, the fiat→token conversion and the apply gate are all
	// its; when no source can price the chosen currency it refuses to convert,
	// which is the whole reason the machine exists.
	let batchView = $state<BatchView | null>(null);
	let batchSession: BatchImportSession | null = null;

	async function openBatch(): Promise<void> {
		const token = sendView?.selected_token;
		if (batchSession || !token) return;
		await loadCore();
		batchSession = createBatchImportSession({
			onView: (view) => (batchView = view),
			onError: (error) => console.error('[batch_import] core fault:', error)
		});
		batchSession.start({
			type: 'open',
			token: {
				symbol: token.symbol,
				decimals: token.decimals,
				balance: token.balance,
				price_usd: token.price_usd
			},
			currency_code: currency.view.code,
			max_recipients: 60
		});
	}

	function closeBatch(): void {
		batchSession?.dispose();
		batchSession = null;
		batchView = null;
	}

	const batchActions = $derived(
		batchView === null
			? undefined
			: {
					unit: (id: string) =>
						batchSession?.dispatch({ type: 'set_unit', unit: id === 'fiat' ? 'fiat' : 'token' }),
					paste: (text: string) => batchSession?.dispatch({ type: 'set_raw_text', text }),
					rate: (text: string) => batchSession?.dispatch({ type: 'edit_rate', text }),
					resetRate: () => batchSession?.dispatch({ type: 'reset_rate_to_auto' }),
					pickFile: () => batchSession?.dispatch({ type: 'pick_file_requested' }),
					saveTemplate: () => batchSession?.dispatch({ type: 'save_template_requested' }),
					apply: () => {
						const recipients = batchView?.recipients ?? [];
						if (recipients.length === 0) return;
						// The core parsed and priced them; the send core seeds its split
						// from exactly those rows, and nothing is recomputed here.
						sendSession?.dispatch({
							type: 'seed_split_recipients',
							recipients: recipients.map((r, index) => ({
								id: `b${index}`,
								address: r.address,
								amount: r.amount,
								name: r.name
							}))
						});
						closeBatch();
					}
				}
	);

	const batchInputs = $derived(
		batchView && sendView?.selected_token
			? { batch: batchView, m: data.flowMessages, symbol: sendView.selected_token.symbol }
			: undefined
	);

	// --- Adding a token (spec 028 US4) -------------------------------------
	//
	// `manage_tokens` has had an executor, a session, types and 22 Rust tests
	// since 025, and was constructed by nothing. It is built when the sheet
	// opens and disposed when it closes; the network snapshot rides on the
	// probe request because the registry (defaults + custom networks) is the
	// shell's, and a confirmed save invalidates the token cache through the
	// core's own `invalidate_token_cache`, which is where the balance list
	// learns to look again.
	let addTokenView = $state<MtokView | null>(null);
	let manageTokens: ManageTokensSession | null = null;
	/**
	 * T3's two tabs (spec 028 Phase 10). The drawn toggle switched nothing:
	 * the ERC-20 half is `manage_tokens`' and the native half — a network by
	 * name or chain ID — is `network_admin`'s add-network wizard, the same
	 * app-resident session the settings screen drives. The query, the tab and
	 * the chain this sheet added are the sheet's own.
	 */
	let addTokenTab = $state<'erc20' | 'native'>('erc20');
	let netQuery = $state('');
	let netAddedChainId = $state<number | null>(null);
	/** The chain whose add was confirmed here, until the ledger says it landed. */
	let netPendingAdd = $state<number | null>(null);

	async function openAddToken(): Promise<void> {
		if (manageTokens) return;
		await loadCore();
		if (manageTokens) return;
		// The wizard is ready the moment the native tab is chosen.
		void networkAdmin.boot();
		manageTokens = createManageTokensSession({
			account: () => identity?.address ?? '',
			onInvalidated: () => balance.refresh(true),
			onView: (view) => (addTokenView = view),
			onError: (error) => console.error('[manage_tokens] core fault:', error)
		});
		manageTokens.start({ type: 'start' });
	}

	function closeAddToken(): void {
		manageTokens?.dispose();
		manageTokens = null;
		addTokenView = null;
		if (addTokenTab === 'native' || netQuery !== '') {
			networkAdmin.dispatch({ type: 'wizard_reset' });
		}
		addTokenTab = 'erc20';
		netQuery = '';
		netAddedChainId = null;
		netPendingAdd = null;
	}

	// The ledger answers the add: the chain is in the registry, the card says
	// so, and the balances go and look at it.
	$effect(() => {
		const pending = netPendingAdd;
		const landed = networkAdmin.view.last_added_chain_id;
		if (pending === null || landed !== pending) return;
		netPendingAdd = null;
		netAddedChainId = pending;
		balance.refresh(true);
	});

	/**
	 * The registry as the core's `u32` can carry it — wire representability,
	 * not policy: a row that cannot be serialised would make the probe request
	 * throw and the field do nothing at all.
	 */
	function networkSnapshot(): { chain_id: number; name: string }[] {
		return getAllNetworksSync()
			.filter((n) => Number.isInteger(n.chainId) && n.chainId >= 0 && n.chainId <= 4_294_967_295)
			.map((n) => ({ chain_id: n.chainId, name: n.displayName }));
	}

	const addTokenActions = $derived(
		addTokenView === null
			? undefined
			: {
					input: (value: string) => {
						if (addTokenTab === 'native') {
							netQuery = value;
							netAddedChainId = null;
							networkAdmin.dispatch({ type: 'search_input', query: value });
							return;
						}
						manageTokens?.dispatch({ type: 'address_input', s: value });
					},
					submit: () => {
						if (addTokenTab === 'native') {
							const candidate = networkAdmin.view.wizard.chain_info?.chain_id ?? null;
							if (candidate === null || !networkAdmin.view.wizard.can_add) return;
							netPendingAdd = candidate;
							// A timestamp, not a clock: the record carries when it was added.
							const nowIso = new Date().toISOString();
							networkAdmin.dispatch({ type: 'add_confirmed', now_iso: nowIso });
							return;
						}
						const first = addTokenView?.found[0];
						if (first && !first.added) {
							manageTokens?.dispatch({ type: 'save_requested', chain_id: first.chain_id });
						}
					},
					tab: (id: string) => {
						const next = id === 'native' ? 'native' : 'erc20';
						if (next === addTokenTab) return;
						addTokenTab = next;
						if (next === 'native') void networkAdmin.boot();
						else if (netQuery !== '') networkAdmin.dispatch({ type: 'wizard_reset' });
					},
					pick: (id: string) => {
						const chainId = Number(id);
						if (!Number.isInteger(chainId)) return;
						networkAdmin.dispatch({
							type: 'chain_selected',
							chain_id: chainId,
							keep_custom_rpc: false
						});
					}
				}
	);

	// The probe fires the moment the address is well-formed. The phone has a
	// separate "search" button; the drawn sheet (T3) has one CTA, "add", so the
	// search is implicit. The core's echo gate discards an answer for an
	// address the person has already typed past.
	$effect(() => {
		const view = addTokenView;
		if (!view || !view.address_valid || view.detecting) return;
		if (view.found.length > 0 || view.not_found) return;
		manageTokens?.dispatch({ type: 'detect_requested', networks: networkSnapshot() });
	});

	const addTokenInputs = $derived(
		addTokenView
			? {
					view: addTokenView,
					m: data.flowMessages,
					tab: addTokenTab,
					native: {
						query: netQuery,
						wizard: networkAdmin.view.wizard,
						addedChainId: netAddedChainId
					}
				}
			: undefined
	);

	// --- The address book beside a send (spec 028 US5) -----------------------
	//
	// The recipient picker (SD2e / DSD2e) showed the gallery's three fixture
	// people in the middle of a live transfer, and `show_contact_picker` —
	// the core's own state for it — was read by nothing. While a send is open
	// this route holds its own ContactsCore session (024 D8: route-scoped,
	// not a global ledger) and hands its view to the picker; a pick dispatches
	// the core's `picked_address`, a group seeds split mode with its members.
	let contactsView = $state<ContactsView | null>(null);
	let contactsSession: ContactsSession | null = null;

	function openContactsBook(): void {
		if (contactsSession) return;
		contactsSession = createContactsSession({
			onView: (view) => (contactsView = view),
			onError: (error) => console.error('[contacts] core fault:', error)
		});
		contactsSession.start({ type: 'account_switched', my_address: identity?.address ?? null });
	}

	function closeContactsBook(): void {
		contactsSession?.dispose();
		contactsSession = null;
		contactsView = null;
	}

	/** A whole group as split-mode recipients: the core's `seed_split_recipients`, amounts blank. */
	function seedGroup(group: ContactGroupView): void {
		if (group.members.length === 0) return;
		sendSession?.dispatch({
			type: 'seed_split_recipients',
			recipients: group.members.map((member) => ({
				id: '',
				address: member.address,
				amount: '',
				name: member.name ?? member.resolved_name
			}))
		});
	}

	async function openSend(prefill?: Partial<SendOpenParams>): Promise<void> {
		if (sendSession || !identity) return;
		await loadCore();
		if (!identity) return;
		openContactsBook();
		// The tracker owns the receipt from the moment the op is accepted; the
		// send core only hears the verdict back (invariant ⑥'s ordering half).
		setSendTrackerSink((handoff) =>
			trackSubmitted(handoff.userOpHash, handoff.recordIds, handoff.chainId, (outcome) =>
				sendSession?.dispatch({
					type: 'receipt_update',
					user_op_hash: handoff.userOpHash,
					outcome
				})
			)
		);
		startTxTracker();
		const account = identity.address;
		const credentialId = session.view.accounts[session.view.active_index]?.account.id ?? '';
		sendSession = createSendSession({
			onView: (view) => (sendView = view),
			onError: (error) => console.error('[send] core fault:', error),
			ports: {
				tokensPartial: () => {},
				// The originals are the API's; the core carries the slice it needs and
				// the overlays read that. Indexing them here (as Expo does for its
				// token selector's logos) would be a second copy nothing reads.
				tokensFetched: () => {},
				credentialId: () => session.view.accounts[session.view.active_index]?.account.id ?? null,
				credentialLoaded: () => {},
				signingStarted: () => {},
				receiptUpdate: () => {},
				// Kept on screen until the person edits or moves on (spec 038 #D4);
				// it used to be a console line nobody reading the form could see.
				alert: (kind) => (sendAlert = kind),
				close: () => closeSend(),
				feeQuote: async (request) => {
					const outcome = await feeQuote.requestQuote(request);
					if (outcome.kind === 'ok') return { type: 'ok', estimate: outcome.estimate };
					if (outcome.kind === 'failed') return { type: 'failed', kind: outcome.failure };
					// The shell could not obtain an input the question requires, or
					// the surface moved on. Neither is a verdict about a fee — the
					// core hears the same "not estimated" either way.
					return { type: 'failed', kind: 'estimate_failed' };
				}
			}
		});
		sendSession.start({
			type: 'open',
			account: { id: credentialId, address: account, name: identity?.name ?? null },
			params: {
				preselected_symbol: null,
				preselected_network: null,
				prefilled_recipient: null,
				prefilled_chain_id: null,
				prefilled_token_address: null,
				prefilled_amount_base: null,
				locked: false,
				preselected_multi: null,
				// A code scanned from the wallet home arrives here: the core reads
				// these exactly as it reads the deep-link params on the phone.
				...prefill
			},
			display: { code: currency.view.code, rate: currency.view.rate, fiat_decimals: 2 }
		});
	}

	function closeSend(): void {
		closeBatch();
		closeContactsBook();
		sendSession?.dispose();
		sendSession = null;
		sendView = null;
		feeSheetOpen = false;
		sweepPicking = false;
		sendClassFilter = 'all';
		feeQuote.dispose();
		nav.close();
	}

	/**
	 * The shell's half of the warm-up (spec 028 Phase 10): the core asks for
	 * a transfer-sized quote the moment a token is picked; this warms the RPC
	 * reads that quote needs — deployment, nonce, gas — so the pipeline's
	 * first waves answer from cache. The core says this is the shell's
	 * (`prefetchForSend`, send-executor.ts), and until now nothing called it.
	 */
	let prefetched = '';
	$effect(() => {
		const token = sendView?.selected_token;
		const address = identity?.address;
		if (!token || address === undefined) return;
		const key = `${address}:${token.chain_id}`;
		if (key === prefetched) return;
		prefetched = key;
		prefetchForSend(address, token.chain_id);
	});

	/** The screen the core's stage names. The nav stack is not consulted here. */
	const sendState = $derived.by(() => {
		const view = sendView;
		if (!view) return undefined;
		// The scanner is the CORE's state, not a shell flag: `open_scanner` is
		// what the recipient row dispatches and `scan_resolved` is what closes it,
		// so the picker, the form and the sweep all open the same one.
		if (view.show_scanner) return 's1' as const;
		// The picker too (spec 028 US5): `open_contact_picker` is what the
		// recipient row dispatches, and a pick — or `close_contact_picker` —
		// is what takes it down.
		if (view.show_contact_picker) return 'sd2e' as const;
		if (feeSheetOpen) return 'sd2f' as const;
		if (batchView) return 'sd2c' as const;
		switch (view.stage) {
			case 'select_token':
				// SD1b is SD1 with checkboxes: the same list, choosing several.
				return sweepPicking ? ('sd1b' as const) : ('sd1' as const);
			case 'enter_details':
				return view.multi_select_mode ? ('sd2d' as const) : ('sd2' as const);
			case 'confirm':
				return view.multi_select_mode ? ('sd3c' as const) : ('sd3' as const);
			case 'receipt':
				return 'sd4b' as const;
			default:
				return 'sd1' as const;
		}
	});

	const sendActions = $derived(
		sendView === null
			? undefined
			: {
					selectToken: (index: number) => {
						// The picker's index is into what it is SHOWING (Phase 10).
						const token = sendVisible[index];
						if (!token) return;
						if (!sweepPicking) {
							sendSession?.dispatch({ type: 'select_token', token_id: sendTokenId(token) });
							return;
						}
						// A batch is one chain. The phone pins it with a filter; the
						// drawn picker (SD1b) pins it with the FIRST pick, so the first
						// tap names the network and the core refuses every other chain
						// from then on. Emptying the selection unpins, so a person can
						// start over without leaving the screen.
						if (sendView?.multi_chain_id === null) {
							sendSession?.dispatch({ type: 'set_multi_network', chain_id: token.chain_id });
						}
						sendSession?.dispatch({ type: 'toggle_multi_token', token_id: sendTokenId(token) });
					},
					selectAll: () => {
						// The scope is what the picker is showing; what counts as
						// valuable inside that scope stays the core's.
						sendSession?.dispatch({
							type: 'toggle_all_multi_tokens',
							visible_ids: sendVisible.map(sendTokenId)
						});
					},
					filterClass: (id: string) => {
						sendClassFilter =
							id === 'stable' || id === 'gas' || id === 'other' ? id : ('all' as const);
					},
					pickCta: () => {
						if (!sweepPicking) {
							sweepPicking = true;
							return;
						}
						if ((sendView?.multi_selected_ids.length ?? 0) === 0) return;
						// The core decides what this becomes: one pick is a normal
						// send, several are a sweep, and the warm-up estimate starts.
						sendSession?.dispatch({ type: 'confirm_multi_selection' });
					},
					amountChanged: (value: string) =>
						sendSession?.dispatch({ type: 'set_amount', amount: value }),
					// 最大 was drawn on the token card and wired to nothing (spec 028
					// Phase 9, T489); the core's rule fills it fee-aware.
					max: () => sendSession?.dispatch({ type: 'tap_max' }),
					recipientChanged: (value: string) =>
						sendSession?.dispatch({ type: 'set_recipient', recipient: value }),
					advance: () => sendSession?.dispatch({ type: 'continue' }),
					// "+ add recipient" turns one into many (the core's transition); on
					// the split form the same words add a blank row (Phase 10).
					addRecipient: () => {
						if (!sendView?.split_mode) {
							sendSession?.dispatch({ type: 'enter_split_mode' });
							return;
						}
						sendSession?.dispatch({
							type: 'recipients_changed',
							recipients: [...(sendView?.recipients ?? []), blankRecipient()]
						});
					},
					recipientRowChanged: (index: number, patch: { address?: string; amount?: string }) => {
						const rows = (sendView?.recipients ?? []).map((row, i) =>
							i === index ? { ...row, ...patch } : row
						);
						sendSession?.dispatch({ type: 'recipients_changed', recipients: rows });
					},
					// The book, for one row — or for a new one appended for it. The
					// core's `picker_target` puts the pick where it was asked for.
					pickContactFor: (index: number | null) => {
						const rows = sendView?.recipients ?? [];
						let target = index === null ? undefined : rows[index]?.id;
						if (target === undefined) {
							const row = blankRecipient();
							target = row.id;
							sendSession?.dispatch({
								type: 'recipients_changed',
								recipients: [...rows, row]
							});
						}
						sendSession?.dispatch({ type: 'open_contact_picker', target });
					},
					pickContact: (index: number) => {
						const contact = contactsView?.contacts[index];
						if (contact)
							sendSession?.dispatch({ type: 'picked_address', address: contact.address });
					},
					pickGroup: (index: number) => {
						const group = contactsView?.groups[index];
						if (group) seedGroup(group);
					},
					removeRecipient: (index: number) => {
						const rows = (sendView?.recipients ?? []).filter((_, i) => i !== index);
						sendSession?.dispatch({ type: 'recipients_changed', recipients: rows });
					},
					confirm: () => sendSession?.dispatch({ type: 'slide_confirm' }),
					pickFeeToken: (index: number) => {
						const option = feeQuote.view.options[index];
						if (option) {
							feeQuote.selectAsset(option.contract);
							sendSession?.dispatch({ type: 'choose_fee_token', token: option.contract });
						}
						feeSheetOpen = false;
					},
					done: () => {
						sendSession?.dispatch({ type: 'done' });
						closeSend();
					},
					// The three surfaces the phone raises as sheets and the desktop opens
					// as panels (spec 028 T453). They are the session's, so the host that
					// asks does not need to know which layout it is on.
					openFeeSheet: () => {
						feeSheetOpen = true;
					},
					openBatch: () => void openBatch(),
					openScanner: () => sendSession?.dispatch({ type: 'open_scanner' }),
					continueDisabled: !sendView.can_continue,
					confirmDisabled: !sendView.can_confirm
				}
	);

	/** A row the shell adds: its own id, nothing in it yet. */
	function blankRecipient() {
		return { id: makeRecipientId(), address: '', amount: '', name: null };
	}

	// An emptied selection unpins the chain (see `selectToken`). The core keeps
	// `multi_chain_id` until told otherwise, and a picker locked to a chain with
	// nothing ticked would grey every other row for no reason a person can see.
	$effect(() => {
		const view = sendView;
		if (!sweepPicking || !view || view.multi_chain_id === null) return;
		if (view.multi_selected_ids.length === 0 && !view.multi_select_mode) {
			sendSession?.dispatch({ type: 'set_multi_network', chain_id: null });
		}
	});

	/** The live inputs the send overlays read, or `undefined` while none is open. */
	/** The core's last refusal, shown on the form/confirm until an edit or a move. */
	let sendAlert = $state<SendAlertKind | null>(null);
	// What the person can change: a stage, a field, a row. Any of it changing
	// is the person acting on the refusal, and the line comes down. The
	// refusal's own render changes none of these, so it stays up.
	const sendAlertScope = $derived(
		sendView
			? `${sendView.stage}|${sendView.recipient}|${sendView.amount}|${sendView.recipients.map((r) => `${r.address}:${r.amount}`).join(',')}`
			: ''
	);
	$effect(() => {
		void sendAlertScope;
		sendAlert = null;
	});

	const sendInputs = $derived(
		sendView && identity
			? {
					send: sendView,
					fee: feeQuote.view ?? IDLE_FEE_VIEW,
					m: data.flowMessages,
					currency: currency.view,
					identity,
					identicon: avatarSvgForClient,
					sweepPicking,
					chainFilter: chainFilter.chainId,
					classFilter: sendClassFilter,
					alert: sendAlert
				}
			: undefined
	);

	const flowState = $derived(sendState ?? nav.mobileTop);
	/**
	 * The desktop's copy of the same rule (spec 028 T453): while a send is
	 * live, the core's stage names the panel and the nav stack is not
	 * consulted. Until this phase the third column read `nav.desktopTop` alone,
	 * so it showed the send screens with live DATA (026's overlays) and dead
	 * controls — a Continue that did nothing on a form that knew the balance.
	 */
	const desktopSendState = $derived.by(() => {
		const view = sendView;
		if (!view) return undefined;
		// The scanner is a centred modal on this layout; the host below draws
		// it for `ds1` and hides the panel.
		if (view.show_scanner) return 'ds1' as const;
		// The picker is the core's state too (spec 028 US5).
		if (view.show_contact_picker) return 'dsd2e' as const;
		if (feeSheetOpen) return 'dsd2f' as const;
		if (batchView) return 'dsd2c' as const;
		switch (view.stage) {
			case 'select_token':
				return 'dsd1' as const;
			case 'enter_details':
				return view.split_mode ? ('dsd2b' as const) : ('dsd2' as const);
			case 'confirm':
				return 'dsd3' as const;
			case 'receipt':
				return 'dsd4' as const;
			default:
				return 'dsd1' as const;
		}
	});
	const desktopFlow = $derived(desktopSendState ?? nav.desktopTop);

	/**
	 * Issue 213: the transaction screen only exists while its record does.
	 * `txDetail` is that record, live; without one the prerendered state would
	 * draw the mocks' "+120 USDT received" over whatever account is open — so
	 * the list underneath is shown instead, and `nav` is unwound to match.
	 */
	const shownFlowState = $derived(shownTxDetailStateMobile(flowState, txDetail));
	const shownDesktopFlow = $derived(shownTxDetailStateDesktop(desktopFlow, txDetail));
	$effect(() => {
		if (txDetail !== undefined) return;
		if (nav.mobileTop !== 'a2' && nav.desktopTop !== 'da2') return;
		selectedTxId = null;
		nav.back();
	});

	// --- The scanner (spec 028 T422/T423) ------------------------------------
	//
	// `ScanSurface` owns no camera and knows of none: it draws a frame, a hint
	// and three tools, and takes a snippet for whatever fills the frame. This is
	// what fills it. The refusals matter more than the decode — a black
	// viewfinder tells a person their camera is broken, when the truth is
	// usually a permission they can change or a URL that is not HTTPS.

	let scanVideo = $state<HTMLVideoElement | null>(null);
	let scanPicker = $state<HTMLInputElement | null>(null);
	/** A code that WAS read and is not one this wallet can act on. */
	let scanUnusable = $state(false);

	/** The scan screen is showing, on whichever layout is drawn. */
	const scanning = $derived(flowState === 's1' || desktopFlow === 'ds1');

	/**
	 * Expo re-arms its scanner two seconds after a decode, and the same reason
	 * applies here: a poster with an unusable code in frame must not end the
	 * scan, and re-arming instantly would decode that same code forever.
	 */
	const SCAN_REARM_MS = 2000;
	let scanRearm = 0;

	$effect(() => {
		const video = scanVideo;
		if (!scanning || !video) return;
		scanUnusable = false;
		void scanner.start(video);
		return () => {
			clearTimeout(scanRearm);
			scanner.stop();
		};
	});

	// One code is acted on once: the read is taken off the surface before it is
	// handled, so a still-set `result` cannot fire this twice.
	$effect(() => {
		const found = scanner.result;
		if (found === null) return;
		scanner.clear();
		handleScan(found);
	});

	function handleScan(value: string): void {
		const request = parseEIP681(value);
		// Inside a live send the CORE rules on the scan — whether the screen
		// locks, to which chain, and how base units become a figure are all
		// `scan_resolved`'s to decide (send.rs). The shell only tokenizes.
		if (sendSession && sendView) {
			sendSession.dispatch({
				type: 'scan_resolved',
				scan: request
					? {
							type: 'request',
							recipient: request.recipient,
							chain_id: request.chainId ?? null,
							token_address: request.tokenAddress ?? null,
							amount_base_units: request.amountBaseUnits?.toString() ?? null
						}
					: { type: 'text', data: value }
			});
			return;
		}
		// From the wallet home there is no session yet, so the code OPENS one,
		// prefilled — and locked when the request names a chain to lock to. This
		// is the phone's `onScan` (useHomeController.ts:529) with a session in
		// place of a route push.
		if (request && request.chainId != null) {
			nav.enter('send');
			void openSend({
				prefilled_recipient: request.recipient,
				prefilled_chain_id: String(request.chainId),
				prefilled_token_address: request.tokenAddress ?? null,
				prefilled_amount_base: request.amountBaseUnits?.toString() ?? null,
				locked: true
			});
			return;
		}
		const address = request?.recipient ?? value.trim();
		if (isHexAddress(address)) {
			nav.enter('send');
			void openSend({ prefilled_recipient: address });
			return;
		}
		// A code that is not a payment. Said, and the viewfinder stays alive.
		scanUnusable = true;
		clearTimeout(scanRearm);
		scanRearm = setTimeout(() => {
			if (scanning && scanVideo) void scanner.start(scanVideo);
		}, SCAN_REARM_MS) as unknown as number;
	}

	function scanTool(id: 'gallery' | 'torch' | 'flip'): void {
		if (id === 'gallery') scanPicker?.click();
		else if (id === 'torch') void scanner.toggleTorch();
		else void scanner.flip();
	}

	async function pickScanImage(event: Event): Promise<void> {
		const input = event.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		// Clear it before reading: picking the SAME file twice fires no second
		// change event, and the retry would look like a hang.
		input.value = '';
		if (!file) return;
		scanUnusable = false;
		await scanner.pick(file);
	}

	const scanCopy = $derived(
		scanNotice(
			{ status: scanner.status, nothingFound: scanner.nothingFound, unusable: scanUnusable },
			data.flowMessages
		)
	);

	// The destinations THIS client has — `WEB_DESTINATIONS` (spec 022 founder
	// call): the web has no 探索, and every route reads the same list.

	onMount(() => {
		void session.boot();
		void currency.boot();
		preferences.boot();
		// Money in flight outlives every screen (spec 026 T232): an operation
		// submitted before the tab closed is settled by the tracker's own
		// recovery sweep, which therefore has to run on EVERY wallet boot — not
		// only when someone opens the send flow. Idempotent and throttled by the
		// core, so calling it from here costs one dispatch.
		startTxTracker();
		// The signing machine is resident for the same reason the tracker is: a
		// request can arrive while any screen is showing. `syncNetworks` runs
		// with it — until a snapshot lands every chain is unsupported, which is
		// the fail-closed default a shell must not leave in place.
		void signRequest.boot().then(() => {
			signRequest.syncNetworks();
			signRequest.syncAccounts();
		});
	});

	/**
	 * Publish what an already-connected site may be told, whenever this wallet's
	 * accounts change (spec 027 T332).
	 *
	 * A page that is already connected asks `eth_accounts` and `eth_chainId` on
	 * every load, and the extension's service worker cannot run the core to
	 * answer them. So `ext_cache` decides what the snapshot contains and this
	 * stores it; the worker only reads. Off the extension there is no channel
	 * and no storage to write to, and `publishExtSnapshot` is a no-op.
	 */
	$effect(() => {
		const view = session.view;
		if (view.loading || !inExtension()) return;
		void publishExtSnapshot({
			isLoading: false,
			hasWallet: view.has_wallet,
			accounts: view.accounts.map((row) => row.account),
			active: view.accounts[view.active_index]?.account ?? null,
			theme: 'dark',
			locale: data.locale ?? 'en'
		});
	});

	/**
	 * The network catalog the worker answers reads and chain switches from —
	 * built-in chains plus the custom networks a person added. Published once
	 * the wallet is up, and again whenever the network list changes.
	 */
	onMount(() => {
		if (!inExtension()) return;
		void publishExtChains();
		return subscribeNetworks(() => void publishExtChains());
	});

	/**
	 * A connected site follows the active account (spec 027 T350's rule,
	 * performed). The FIRST address the session settles on is a boot, not a
	 * switch: only a change from one known address to another asks the core to
	 * re-pin the grants, and the worker announces each re-pinned grant to the
	 * site's tabs as `accountsChanged`.
	 */
	let followedAddress: string | null = null;
	$effect(() => {
		const view = session.view;
		if (view.loading || !inExtension() || !view.address) return;
		const previous = followedAddress;
		followedAddress = view.address;
		if (previous === null || previous.toLowerCase() === view.address.toLowerCase()) return;
		void followActiveAccount({
			activeAddress: view.address,
			addresses: view.accounts.map((row) => row.account.address)
		});
	});

	// The account the balances belong to. `account_changed` is also the
	// hydrate: the core reads its cache and fetches for whoever is signed in.
	$effect(() => {
		const address = identity?.address;
		if (address !== undefined) {
			void balance.setAccount(address);
			void feed.setAccount(address);
		}
	});

	// Balance privacy reaches the feed too — the core withholds the toast
	// while hidden (invariant ④), and every money row masks together.
	$effect(() => {
		feed.privacyChanged(balance.view.hidden);
	});

	// The 10s Activity poll the Expo home ran, only while the tab is visible.
	onMount(() => {
		const id = setInterval(() => {
			if (document.visibilityState === 'visible') feed.liveTick();
		}, 10_000);
		return () => clearInterval(id);
	});

	/**
	 * The deposit watcher lives exactly as long as a receive screen is showing
	 * (research D12): created on entry with THIS address, disposed on leave.
	 * A detected deposit refreshes the balances and nudges the feed — the row
	 * appearing and the total moving are the acknowledgement.
	 */
	let watcher: ReceiveWatchSession | null = null;
	const receiving = $derived(
		(flowState !== undefined && flowState.startsWith('r')) ||
			(desktopFlow !== undefined && desktopFlow.startsWith('dr'))
	);
	$effect(() => {
		const address = identity?.address;
		if (!receiving || address === undefined) return;
		let disposed = false;
		void loadCore().then(() => {
			if (disposed) return;
			watcher = createReceiveWatchSession({
				address,
				onView: () => {},
				onDeposit: () => {
					balance.refresh(true);
					feed.liveTick();
				},
				onError: (error) => console.error('[receive-watch] core fault:', error)
			});
			watcher.start({ type: 'start' });
		});
		return () => {
			disposed = true;
			watcher?.dispose();
			watcher = null;
		};
	});

	// Page visibility stands in for app focus (research D12): a hidden tab
	// pauses the pollers, a returning one refreshes by the core's rules.
	onMount(() => {
		const onvisibility = () => {
			if (document.visibilityState === 'visible') {
				balance.focused();
				feed.focusTick();
			} else balance.backgrounded();
		};
		document.addEventListener('visibilitychange', onvisibility);
		return () => document.removeEventListener('visibilitychange', onvisibility);
	});

	/** Fixture base → identity overlay → live balance/holdings (research D10). */
	const liveInputs = $derived({
		balance: balance.view,
		currency: currency.view,
		m: data.walletMessages,
		feed: feed.view,
		// The sidebar's network filter: holdings and feed narrow to it, the
		// hero total does not (the phone app's `selectedChainId` semantics).
		chainFilter: chainFilter.chainId,
		selectedToken:
			selectedAssetId === null
				? undefined
				: balance.view.tokens.find((t) => balanceTokenId(t) === selectedAssetId)
	});
	const liveHome = $derived(
		identity === null
			? data.home
			: withLiveWallet(homeWithIdentity(data.home, identity), liveInputs)
	);
	const liveDesktop = $derived(
		identity === null
			? data.desktop
			: withLiveWalletDesktop(webNav(desktopWithIdentity(data.desktop, identity)), {
					customChainIds,
					...liveInputs,
					// Two things cannot occupy one column (founder, 2026-09-05: the
					// token's detail and the picker were drawn side by side). While a
					// flow holds the column the asset panel stays closed; the flow
					// itself still reads `selectedToken` through `flowInputs`.
					selectedToken: desktopFlow === undefined ? liveInputs.selectedToken : undefined
				})
	);

	/** The pushed assets screen shows the same holdings as the home (D10). */
	const flowInputs = $derived({
		...liveInputs,
		identity: identity ?? undefined,
		fm: data.flowMessages,
		receiveChainId: selectedReceiveChainId,
		emptyCopy: data.flows.t4.base.kind === 'assets' ? data.flows.t4.base.model.empty : undefined,
		send: sendInputs,
		batch: batchInputs,
		addToken: addTokenInputs,
		contactPick:
			contactsView && sendView
				? { view: contactsView, m: data.flowMessages, identicon: avatarSvgForClient }
				: undefined
	});

	/**
	 * The browser's Back unwinds the flow stack before it leaves the wallet.
	 * `FlowNav` pushed a history entry for every step, so each `popstate` here
	 * corresponds to exactly one of them.
	 */
	onMount(() => {
		const onpop = () => {
			if (nav.open) nav.back();
		};
		addEventListener('popstate', onpop);
		return () => removeEventListener('popstate', onpop);
	});

	// The route guard's other half. `loading` is deliberately not acted on: the
	// core has not ruled yet, and bouncing on a non-answer would throw a
	// reloading person back to Welcome mid-boot.
	$effect(() => {
		if (view.allowed_route === 'onboarding') void goto(welcome, { replaceState: true });
	});

	/**
	 * 设置 (spec 023) and 通讯录 (spec 024) have routes; 探索 has none on web
	 * by decision (spec 022), so it stays put.
	 */
	function select(id: 'wallet' | 'contacts' | 'explore' | 'settings') {
		if (id === 'settings') void goto(settings);
		else if (id === 'contacts') void goto(contactsHref);
	}

	// --- The signing sheet (spec 026 Phase 5, hosted since 027 T340) ---------
	//
	// The wiring lives in `<SigningHost>` because 027 added a second place a
	// request can reach a person — the extension's request window — and a second
	// copy of the most dangerous screen in the product would be a second
	// implementation of it.

	/**
	 * A person arriving from the address book (spec 028 US5): `?to=` opens a
	 * send with the recipient filled — what a scanned address does — `?group=`
	 * opens one and seeds split mode with the group's members once the book has
	 * answered, `?flow=receive` opens the receive card. Read once; the query is
	 * then dropped from the URL so a reload is a plain visit.
	 */
	let handedOff = false;
	$effect(() => {
		if (handedOff || !identity) return;
		const handoff = readFlowHandoff(location.search);
		handedOff = true;
		if (handoff === null) return;
		void goto(resolve('/[locale]/wallet', { locale: data.locale }), { replaceState: true });
		if (handoff.kind === 'receive') {
			nav.enter('receive');
			return;
		}
		nav.enter('send');
		void openSend(
			handoff.kind === 'send' ? { prefilled_recipient: handoff.recipient } : undefined
		).then(() => {
			if (handoff.kind !== 'group-send') return;
			pendingGroup = handoff.groupId;
		});
	});

	/** A group hand-off waits for the book to load, then seeds the split. */
	let pendingGroup = $state<string | null>(null);
	$effect(() => {
		const id = pendingGroup;
		const view = contactsView;
		if (id === null || !view?.loaded || !sendSession) return;
		pendingGroup = null;
		const group = view.groups.find((g) => g.id === id);
		if (group) seedGroup(group);
	});

	/**
	 * What a pushed step is ABOUT, before the step opens (spec 028 Phase 9).
	 * The history screen names a row by position, the receive list a network,
	 * the assets screen a held token — each by an index into a list the live
	 * builders walk in the same order.
	 */
	function noteTarget(to: string, index: number | undefined): void {
		if (index === undefined) return;
		if (to === 'tx-detail') selectTxAt(index);
		else if (to === 'receive-qr')
			selectedReceiveChainId = receiveNetworks()[index]?.chainId ?? null;
		else if (to === 'token-detail') {
			// The assets screen lists the filtered holdings (Phase 10): same list.
			const token = visibleBalanceTokens(balance.view.tokens, chainFilter.chainId)[index];
			selectedAssetId = token === undefined ? null : balanceTokenId(token);
		}
	}

	/**
	 * Open a flow from the home, the asset column or the token sheet. `detail`
	 * names the held token a door is about (spec 028 Phase 9, RULING 3): 转账
	 * from a token opens the form with it chosen, 收款 from a token opens its
	 * own code — no picker in between, the token already names its chain.
	 */
	function enter(entry: FlowEntry, detail?: { assetId?: string }) {
		// A door taken while a send is open leaves that send (Phase 10): a
		// picker already up used to swallow a token's own 转账 — `openSend`
		// returned early — and stay beside the token's detail.
		if (sendSession) closeSend();
		if (manageTokens && entry !== 'add-token') closeAddToken();
		// One column: a flow opening closes the asset detail (and vice versa) —
		// except the token screen and the token's code, which ARE the asset.
		if (entry !== 'token-detail' && entry !== 'receive-token') selectedAssetId = null;
		if (detail?.assetId !== undefined) selectedAssetId = detail.assetId;
		if (entry === 'send') {
			nav.enter(entry);
			const token =
				detail?.assetId === undefined
					? undefined
					: balance.view.tokens.find((t) => balanceTokenId(t) === detail.assetId);
			void openSend(
				token === undefined
					? undefined
					: { preselected_symbol: token.symbol, preselected_network: networkId(token.chain_id) }
			);
			return;
		}
		if (entry === 'receive' && chainFilter.chainId !== null) {
			// RULING 1: the sidebar's filter already chose the network — straight
			// to its code, with the list one step back.
			nav.enter(entry);
			selectedReceiveChainId = chainFilter.chainId;
			nav.push('receive-qr');
			return;
		}
		nav.enter(entry);
		if (entry === 'add-token') void openAddToken();
	}

	/**
	 * The history screen names a row by position (`group * 100 + row`, its
	 * own convention); the feed is walked the way the groups were built.
	 */
	function selectTxAt(index: number) {
		// The history screen lists the narrowed feed (Phase 10): same feed.
		const shown = feed.view ? narrowedFeed(feed.view, chainFilter.chainId) : feed.view;
		selectedTxId = feedItemAt(shown, Math.floor(index / 100), index % 100)?.id ?? null;
	}

	/**
	 * The phone's network filter (spec 028 Phase 10): the pill on A1 / T1 /
	 * SD1 raises this sheet, and a row sets the same `chainFilter` the
	 * desktop's sidebar does — the home, the pushed lists and the picker all
	 * narrow to it. Shell state, as the sidebar's is.
	 */
	let chainSheetOpen = $state(false);
	const chainRows = $derived(
		liveChainRows(
			balance.view,
			data.walletMessages.networkFilter.allNetworks,
			chainFilter.chainId,
			customChainIds
		)
	);

	/**
	 * The open transaction's delete (spec 028 Phase 8). The feed tombstones the
	 * record and removes the row at once (`activity_feed`'s `DeleteRequested`),
	 * so the detail has nothing left to show: the sheet, or the third column,
	 * steps back to the list it came from.
	 */
	function deleteSelectedTx() {
		if (selectedTxId === null) return;
		feed.deleteRecord(selectedTxId);
		selectedTxId = null;
		nav.back();
	}

	// --- The rescue sheets (spec 028 Phase 8) -----------------------------------
	//
	// 023 drew three rescues as settings components and placed them over the
	// wallet (SR2 RPC fix, SR3 balance detail) and over the send (SR4 relayer
	// treasury). The balance status line is their door on this route: an
	// unreachable chain opens its RPC fix, anything else opens the breakdown.
	// The treasury sheet opens itself, from the send core's probe.
	type Rescue = 'rpc-fix' | 'balance-detail' | 'relayer';
	let rescue = $state<Rescue | null>(null);
	let rescueChainId = $state<number | null>(null);
	/** The URL being typed, until it is saved. */
	let rpcDraft = $state<string | null>(null);
	/** A save went to the core from this sheet; its probe decides "restored". */
	let rpcSaved = $state(false);
	const rm = $derived(data.rescueMessages);
	const rescueRow = $derived(
		rescueChainId === null
			? undefined
			: networkAdmin.view.networks.find((row) => row.chain_id === rescueChainId)
	);
	const rpcFixModel = $derived(
		rescueRow === undefined
			? undefined
			: liveRpcFix({ row: rescueRow, draft: rpcDraft, saved: rpcSaved }, rm)
	);
	const rpcRestored = $derived(
		rpcSaved && rpcDraft === null && rescueRow?.rpc_health?.type === 'ok'
	);
	const balanceDetailModel = $derived(
		liveBalanceDetail(balance.view, currency.view, rm, data.walletMessages.balance.unpriced)
	);
	const relayerModel = $derived(
		sendView?.treasury_bootstrap ? liveRelayer(sendView.treasury_bootstrap, rm) : undefined
	);
	const rescueTitle = $derived(
		rescue === 'rpc-fix'
			? rm.rescue.rpcFixTitle
			: rescue === 'balance-detail'
				? rm.balanceDetail.title
				: rm.relayer.title
	);

	function openRescue() {
		const failing = balance.view.banner_chain_ids;
		if (failing.length > 0) {
			openRpcFix(failing[0]!);
			return;
		}
		rescue = 'balance-detail';
	}

	function openRpcFix(chainId: number) {
		rescueChainId = chainId;
		rpcDraft = null;
		rpcSaved = false;
		rescue = 'rpc-fix';
		// The row's override fields are loaded on expand, as the settings page does.
		void networkAdmin
			.boot()
			.then(() => networkAdmin.dispatch({ type: 'override_expanded', chain_id: chainId }));
	}

	function closeRescue() {
		if (rescue === 'relayer') sendSession?.dispatch({ type: 'dismiss_treasury_sheet' });
		rescue = null;
	}

	/** Save & Retry, then — once the probe answers — Done. */
	function rpcFixPrimary() {
		if (rescueChainId === null) return;
		if (rpcRestored) {
			// The chain is reachable again: clear its failure in the balance core
			// and force a read — the core's own retry is throttled like any other
			// fetch, and the person just watched the probe succeed.
			balance.fixChainResolved(rescueChainId);
			balance.refresh(true);
			rescue = null;
			return;
		}
		if (rpcDraft !== null) {
			networkAdmin.dispatch({
				type: 'override_field_edited',
				chain_id: rescueChainId,
				field: 'rpc',
				value: rpcDraft
			});
		}
		networkAdmin.dispatch({ type: 'override_blurred', chain_id: rescueChainId });
		rpcDraft = null;
		rpcSaved = true;
	}

	function relayerRetry() {
		sendSession?.dispatch({ type: 'retry_after_bootstrap' });
		rescue = null;
	}

	function copyRelayerAddress() {
		const address = sendView?.treasury_bootstrap?.address;
		if (address) void navigator.clipboard?.writeText(address).catch(() => {});
	}

	// The send core says whether the treasury sheet is up; this only mirrors it.
	$effect(() => {
		if (sendView?.treasury_bootstrap) rescue = 'relayer';
		else if (rescue === 'relayer') rescue = null;
	});
</script>

<svelte:head>
	<title>{data.messages.metaTitle}</title>
	<meta name="robots" content="noindex" />
</svelte:head>

<SigningHost messages={data.signingMessages} fee={feeQuote} />

<!--
  What fills the scanner's frame. One definition for both layouts — only one of
  them is ever mounted, so there is only ever one camera.

  The file input is MOUNTED rather than conditional: `click()` on an input that
  is not in the document opens nothing, and the "choose a photo" tool is the
  whole way out for a person whose camera was refused.
-->
{#snippet scanFeed()}
	<video class="scan-video" bind:this={scanVideo} muted playsinline></video>
	<input
		class="scan-picker"
		type="file"
		accept="image/*"
		tabindex="-1"
		aria-hidden="true"
		bind:this={scanPicker}
		onchange={pickScanImage}
	/>
{/snippet}

{#if identity}
	{#if wide.current}
		<div class="desktop-shell">
			<WalletDesktop
				model={liveDesktop}
				onnav={select}
				onaccounts={() => (switching = true)}
				onflow={enter}
				onbalancetoggle={() => balance.togglePrivacy()}
				onstatus={openRescue}
				onchainselect={(row) => chainFilter.select(row.chainId ?? null)}
				onasset={(row) => {
					// The column is the asset's now: whatever flow held it closes.
					if (sendSession) closeSend();
					closeAddToken();
					nav.close();
					selectedAssetId = row.id ?? null;
				}}
				onassetclose={() => (selectedAssetId = null)}
				onactivity={(row) => (selectedTxId = row.id ?? null)}
			/>
			<!-- `ds1` is the one flow the third column cannot host: a viewfinder
			     in a narrow strip is the wrong shape, so the desktop shows the
			     scanner as a centred modal (DS1L). -->
			{#if shownDesktopFlow !== undefined && shownDesktopFlow !== 'ds1'}
				<FlowsPanel
					model={withLiveTxDetailDesktop(
						withLiveDesktopFlow(data.desktopFlows[shownDesktopFlow], flowInputs),
						txDetail
					)}
					onback={() => {
						if (sendView) {
							// The sheets the phone raises are panels here; leaving one is
							// closing it, not stepping the core back a stage.
							if (feeSheetOpen) feeSheetOpen = false;
							else if (batchView) closeBatch();
							else if (sendView.show_contact_picker)
								sendSession?.dispatch({ type: 'close_contact_picker' });
							else sendSession?.dispatch({ type: 'back' });
							return;
						}
						if (nav.desktopTop === 'dt3') closeAddToken();
						nav.back();
					}}
					onclose={() => {
						if (sendView) {
							closeSend();
							return;
						}
						closeAddToken();
						nav.close();
					}}
					onnavigate={(to, index) => {
						noteTarget(to, index);
						// The picker opens through the core, as the scanner does.
						if (to === 'contact-pick' && sendSession) {
							sendSession.dispatch({ type: 'open_contact_picker', target: null });
							return;
						}
						// No desktop token screen exists: the asset detail column is
						// it (nav.svelte.ts), and it opens through the model.
						if (to === 'token-detail') {
							nav.close();
							return;
						}
						nav.push(to);
						if (to === 'add-token') void openAddToken();
					}}
					addToken={addTokenActions}
					send={sendActions}
					batch={batchActions}
					ondeletetx={deleteSelectedTx}
				/>
			{/if}
		</div>
		{#if desktopFlow === 'ds1'}
			<div class="scan-scrim" role="presentation">
				<div class="scan-modal">
					<ScanSurface
						model={data.desktopScan}
						variant="modal"
						feed={scanFeed}
						notice={scanCopy}
						ontool={scanTool}
						onclose={() =>
							sendView?.show_scanner
								? sendSession?.dispatch({ type: 'close_scanner' })
								: nav.close()}
					/>
				</div>
			</div>
		{/if}
	{:else}
		<main class="page">
			{#if shownFlowState !== undefined}
				<FlowsMobile
					model={withLiveTxDetailMobile(
						withLiveFlow(data.flows[shownFlowState], flowInputs),
						txDetail
					)}
					onback={() => {
						// Backing out of the scanner is closing the scanner, not stepping
						// back a stage — the core opened it and the core closes it.
						if (sendView?.show_scanner) sendSession?.dispatch({ type: 'close_scanner' });
						else if (sendView?.show_contact_picker)
							sendSession?.dispatch({ type: 'close_contact_picker' });
						else if (sendView) sendSession?.dispatch({ type: 'back' });
						else nav.back();
					}}
					onnavigate={(to, index) => {
						noteTarget(to, index);
						// The token sheet's two doors leave the assets flow for the
						// token's own send form / code (RULING 3).
						if (to === 'send-token') {
							enter('send', { assetId: selectedAssetId ?? undefined });
							return;
						}
						if (to === 'receive-token') {
							enter('receive-token');
							return;
						}
						if (to === 'fee-token') feeSheetOpen = true;
						else if (to === 'batch-import') void openBatch();
						else if (to === 'scan' && sendSession) sendSession.dispatch({ type: 'open_scanner' });
						else if (to === 'contact-pick' && sendSession)
							sendSession.dispatch({ type: 'open_contact_picker', target: null });
						else if (to === 'add-token') {
							nav.push(to);
							void openAddToken();
						} else nav.push(to);
					}}
					onsheetclose={() => {
						// The sheet was a pushed step; dismissing it pops the step, so the
						// next tap on "add by address" pushes a fresh one.
						if (nav.mobileTop === 't3') {
							closeAddToken();
							nav.back();
						}
						if (sendView?.show_contact_picker)
							sendSession?.dispatch({ type: 'close_contact_picker' });
					}}
					send={sendActions}
					batch={batchActions}
					scan={{ feed: scanFeed, notice: scanCopy, tool: scanTool }}
					addToken={addTokenActions}
					ondeletetx={deleteSelectedTx}
					onchains={() => (chainSheetOpen = true)}
				/>
			{:else}
				<WalletHome
					model={liveHome}
					destinations={WEB_DESTINATIONS}
					onselect={select}
					onaccounts={() => (switching = true)}
					onflow={enter}
					onbalancetoggle={() => balance.togglePrivacy()}
					onstatus={openRescue}
					onactivity={(row) => (selectedTxId = row.id ?? null)}
					onasset={(row) => (selectedAssetId = row.id ?? null)}
				/>
			{/if}
			{#if chainSheetOpen}
				<BottomSheet
					title={data.walletMessages.networkFilter.sheetTitle}
					closeLabel={data.walletMessages.identiconViewer.close}
					onclose={() => (chainSheetOpen = false)}
				>
					<ChainFilterList
						rows={chainRows}
						onselect={(row) => {
							chainFilter.select(row.chainId ?? null);
							chainSheetOpen = false;
						}}
					/>
				</BottomSheet>
			{/if}
		</main>
	{/if}
{:else}
	<!-- The core has not ruled yet. An empty surface, not a fixture wallet. -->
	<div class="waiting" aria-busy="true"></div>
{/if}

<IdenticonViewerHost copy={data.walletMessages.identiconViewer} />

{#if switching && identity}
	<AccountSwitcher
		copy={{ accounts: data.accountsMessages, close: data.walletMessages.identiconViewer.close }}
		wide={wide.current}
		oncreate={() => void goto(createHref)}
		onsignin={() => void goto(welcome)}
		onclose={() => (switching = false)}
	/>
{/if}

<SignOutHost copy={data.walletMessages.signOut} />

<!-- The rescue sheets (spec 028 Phase 8): a sheet on the phone, a dialog on the desktop. -->
{#snippet rescueBody()}
	{#if rescue === 'rpc-fix' && rpcFixModel !== undefined}
		<RpcFixBody
			panel={rpcFixModel}
			onprimary={rpcFixPrimary}
			onfield={(value) => (rpcDraft = value)}
		/>
	{:else if rescue === 'balance-detail'}
		<BalanceDetailBody
			panel={balanceDetailModel}
			onretry={(id) => {
				balance.fixChainResolved(Number(id));
				balance.refresh(true);
			}}
		/>
	{:else if rescue === 'relayer' && relayerModel !== undefined}
		<RelayerBody panel={relayerModel} onprimary={relayerRetry} oncopy={copyRelayerAddress} />
	{/if}
{/snippet}

{#if rescue !== null && identity}
	{#if wide.current}
		<Dialog title={rescueTitle} closeLabel={rm.common.close} onclose={closeRescue}>
			{@render rescueBody()}
		</Dialog>
	{:else}
		<BottomSheet
			title={rescueTitle}
			closeLabel={rm.common.close}
			height="tall"
			onclose={closeRescue}
		>
			{@render rescueBody()}
		</BottomSheet>
	{/if}
{/if}

<style>
	/* The phone screens are `height: 100%` of whatever holds them, and the
	   root layout only sets a MIN-height — so without a frame the screen
	   stood as tall as its own rows, the DOCUMENT scrolled, and the tab bar
	   scrolled away with it (founder, 2026-09-05). The contacts route frames
	   its phone the same way. */
	.page {
		position: relative;
		height: 100dvh;
		display: flex;
		flex-direction: column;
		background: var(--color-bg-base);
	}

	.waiting {
		min-height: 100dvh;
		background: var(--color-bg-base);
	}

	/* The desktop keeps the wallet visible behind the third column — that is
	   the whole point of a column over a pushed screen. */
	.desktop-shell {
		display: flex;
		height: 100dvh;
		overflow: hidden;
	}

	/* The wallet is a flex ITEM here, beside the flow column, and a flex item
	   given no `flex` is as wide as its content — which with a skeleton
	   balance and an empty feed was a strip down the left of the screen. It
	   takes every column the flow panel leaves, as the gallery's block stage
	   gives it for free. */
	.desktop-shell > :global(.desktop) {
		flex: 1;
		min-width: 0;
	}

	.scan-scrim {
		position: fixed;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		background: var(--color-fixed-backdrop);
	}

	.scan-modal {
		width: min(90vw, calc(var(--size-qrCard) + var(--space-5xl) * 2));
		border-radius: var(--radius-2xl);
		background: var(--color-bg-base);
		box-shadow: var(--shadow-lg);
		overflow: hidden;
	}

	/* Fills the frame the brackets mark. `pointer-events: none` because a
	   `<video>` over the surface swallows the taps meant for the tools under
	   it — measured on the Expo build, and the same element here. */
	.scan-video {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		object-fit: cover;
		border-radius: var(--radius-md);
		background: var(--color-bg-sunken);
		pointer-events: none;
	}

	/* Mounted, not drawn: `click()` on a detached input opens nothing. */
	.scan-picker {
		position: absolute;
		width: var(--border-hairline);
		height: var(--border-hairline);
		opacity: 0;
		pointer-events: none;
	}
</style>
