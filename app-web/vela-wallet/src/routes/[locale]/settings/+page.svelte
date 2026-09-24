<script lang="ts">
	/**
	 * Settings, as a signed-in person reaches it (spec 023).
	 *
	 * The same three things happen here that happen on the wallet route:
	 *
	 * 1. **The guard.** The core rules on `allowed_route`; this page decides
	 *    when to move. A browser with no wallet goes back to Welcome rather
	 *    than being shown an account block it has no business seeing.
	 * 2. **The identity.** Name, address and identicon come from the session
	 *    over the top of the fixture model, rendered in the browser through
	 *    vela-core — the same module the session machine already loaded.
	 * 3. **The way out.** 退出登录 signs out. It used to be the Settings TAB
	 *    that did that, back when there was no settings screen to put it on;
	 *    now the tab opens this page and the row inside does the work, which is
	 *    where a person would look for it.
	 *
	 * What is live and what is a picture — corrected here rather than left to
	 * rot (spec 028 FR-411). This comment used to say everything but the
	 * identity was fixture-driven, which stopped being true in 024:
	 *
	 * - **Live**: the network list, its detail editor and the add-network
	 *   wizard (024); the display currency (024); the connected sites (027);
	 *   and, since 028, the theme, the language row, the number / date / time
	 *   presets and "erase this device".
	 * - **Still canon data**: the latency figures, the storage accounting and
	 *   the RPC-provider panel's health — those wait for the features that
	 *   measure them.
	 */
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { MediaQuery, SvelteMap } from 'svelte/reactivity';
	import SettingsDesktop from '$lib/settings/SettingsDesktop.svelte';
	import SettingsHome from '$lib/settings/SettingsHome.svelte';
	import IdenticonViewerHost from '$lib/wallet/ui/IdenticonViewerHost.svelte';
	import SignOutHost from '$lib/session/ui/SignOutHost.svelte';
	import { desktopWithIdentity, homeWithIdentity } from '$lib/settings/identity';
	import { BREAKPOINT_DESKTOP } from '$lib/tokens/tokens';
	import { session } from '$lib/session/core/session.svelte';
	import { foundingKeyOf, startEthereumBackup } from '$lib/backup/ethereum-backup';
	import { FeeQuote } from '$lib/flows/core/fee-quote.svelte';
	import SigningHost from '$lib/signing/SigningHost.svelte';
	import { signRequest } from '$lib/signing/core/sign-resident.svelte';
	import { checkEthereumBackup, type EthereumBackupState } from '$lib/services/registry-backup';
	import { readWalletKeys, type WalletKeys } from '$lib/services/wallet-keys';
	import { networkAdmin } from '$lib/settings/core/network-admin.svelte';
	import { currency } from '$lib/settings/core/currency.svelte';
	import { feeTierPreference } from '$lib/settings/core/fee-tier.svelte';
	import { signPreference } from '$lib/settings/core/sign-pref.svelte';
	import {
		withLiveAccounts,
		withLiveAccountsDesktop,
		withLiveConnections,
		withLiveCurrency,
		withLiveCurrencyDesktop,
		withLiveFeeSpeed,
		withLiveFeeSpeedDesktop,
		withLiveNetworks,
		withLiveNetworksDesktop,
		withLiveSigning,
		withLiveSigningDesktop,
		withLiveStorage
	} from '$lib/settings/live';
	import { listGrants, revokeAll, revokeGrant } from '$lib/dapp/connections';
	import {
		clearAllCaches,
		clearStorageItem,
		measureDeviceStorage,
		STORAGE_ITEM_IDS,
		type DeviceStorageReport,
		type StorageItemId
	} from '$lib/services/device-storage';
	import {
		feedbackLabels,
		themeFromSegment,
		withEraseFailure,
		withEraseFailureDesktop,
		withFeedback,
		withFeedbackDesktop,
		walletKeysModel,
		withLivePreferences,
		withLivePreferencesDesktop
	} from '$lib/settings/live';
	import { preferences } from '$lib/services/preferences.svelte';
	import { loadCurrencyCodes } from '$lib/settings/core/currency-catalog';
	import { SUPPORTED_LOCALES, type Locale } from '$lib/i18n/locales';
	import { eraseDeviceData } from '$lib/services/erase-device';
	import {
		AREA_OTHER,
		buildBugReport,
		sendBugReport,
		webPlatform,
		type DeviceFacts
	} from '$lib/services/bug-report';
	import { BUILD_COMMIT, BUILD_VERSION } from '$lib/build/info';
	import { netCounters } from '$lib/services/metrics';
	import { getFailedRpcChains } from '$lib/services/rpc-pool';
	import { chainName } from '$lib/services/networks';
	import type { FeedbackResult } from '$lib/settings/model';
	import type { SettingsPrefEvent } from '$lib/settings/pref-events';
	import { LOCALE_ENDONYMS } from '$lib/settings/fixtures';
	import type { SettingsNetEvent } from '$lib/settings/net-events';
	import { identiconSvgForClient } from '$lib/wallet/identicon';
	import { shortenAddress, type WalletIdentity } from '$lib/wallet/identity';
	import { WEB_DESTINATIONS } from '$lib/wallet/destinations';
	import { balance } from '$lib/wallet/core/balance.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const welcome = $derived(resolve('/[locale]', { locale: data.locale }));
	const createHref = $derived(resolve('/[locale]/create', { locale: data.locale }));
	const walletHref = $derived(resolve('/[locale]/wallet', { locale: data.locale }));
	const contactsHref = $derived(resolve('/[locale]/contacts', { locale: data.locale }));
	const wide = new MediaQuery(`(min-width: ${BREAKPOINT_DESKTOP}px)`, false);

	const view = $derived(session.view);
	const signedIn = $derived(view.allowed_route === 'wallet');

	const identity = $derived<WalletIdentity | null>(
		signedIn
			? {
					name: view.accounts[view.active_index]?.account.name ?? '',
					address: view.address,
					identiconSvg: identiconSvgForClient(view.address)
				}
			: null
	);

	/**
	 * The wide layout's app sidebar wears the same identity the phone header
	 * does — and the same network list the wallet shows, from the same balance
	 * resident, so the counts here are this wallet's and not the board's.
	 */
	const sidebar = $derived(
		identity === null
			? data.sidebar
			: {
					...data.sidebar,
					header: {
						name: identity.name,
						addressDisplay: shortenAddress(identity.address),
						addressFull: identity.address,
						identiconSvg: identity.identiconSvg
					}
					// No network list: it is the wallet's filter (spec 028 Phase 9,
					// RULING 2), and a click that left this page for the wallet was a
					// surprise beside rows that stay.
				}
	);

	$effect(() => {
		if (identity !== null) void balance.setAccount(identity.address);
	});

	/**
	 * The sites holding a grant (spec 027 T350). A grant is a standing
	 * permission, so a person has to be able to see which sites hold one and cut
	 * any of them off. Empty off the extension, where there is no dApp channel
	 * and the drawn fixture row stands.
	 */
	let grants = $state<{ origin: string; address: string }[]>([]);

	async function refreshGrants(): Promise<void> {
		grants = await listGrants();
	}

	onMount(() => {
		void session.boot();
		void networkAdmin.boot();
		void currency.boot();
		void feeTierPreference.boot();
		void signPreference.boot();
		void balance.boot();
		preferences.boot();
		void refreshGrants();
		void refreshStorage();
		void loadCurrencyCodes().then((codes) => (currencyCodes = codes));
	});

	/**
	 * What the currency sheet offers (spec 028 Phase 9, T491): the rate
	 * sources' coverage, once they have answered; the drawn eight until then.
	 */
	let currencyCodes = $state<string[]>([]);
	const currencyCatalog = $derived({ codes: currencyCodes, locale: data.locale });

	// --- Preferences (spec 028 T431–T434) -------------------------------------
	//
	// No core rules on any of these, so there is no session to open — only the
	// stored value, the control that shows it, and the one place that writes it.

	/** Set when an erase ran and something survived. Said, never swallowed. */
	let eraseFailed = $state(false);

	// --- The report (spec 081 FR-016) ----------------------------------------
	//
	// The sheet's 发送 was inert: `FeedbackBody` has always taken an `onsend`
	// and this page never passed one, so a person could type a bug report,
	// press the button, and be told nothing at all. These three pieces of state
	// are the whole wiring — what is in flight, and how the last one ended.

	let feedbackSending = $state(false);
	let feedbackResult = $state<FeedbackResult | undefined>(undefined);

	/**
	 * What this device is allowed to say about itself, and nothing more.
	 *
	 * Five named fields. No address, no balance, no endpoint or RPC URL — the
	 * last because a self-hosted endpoint carries an API key in its path often
	 * enough that a public issue tracker is the wrong place for one — and no
	 * raw `vela.*` value. The failure lines are the net counters' CLASSES
	 * (`rpc:final_failure ×3`), which say what broke without saying where.
	 */
	const deviceFacts = $derived.by<DeviceFacts>(() => {
		const failures: string[] = [];
		for (const [key, count] of netCounters()) {
			if (key.endsWith(':final_failure') && count > 0) failures.push(`${key} ×${count}`);
		}
		return {
			version: BUILD_VERSION,
			commit: BUILD_COMMIT,
			platform: webPlatform(),
			language: data.locale,
			unreachable: [...getFailedRpcChains()].map((id) => chainName(id)),
			failures
		};
	});

	/**
	 * Send it, and say what happened.
	 *
	 * Both endings are outcomes, not one success and one error: a report the
	 * endpoint could not file (503 while the token is unprovisioned, 429, any
	 * 5xx, no network) still has the prefilled GitHub form, and the sheet
	 * offers it rather than apologising. The person's typing is already in that
	 * URL — by the form's FIELD IDS, because `template=bug.yml` makes GitHub
	 * ignore `&body=` entirely.
	 */
	async function sendFeedback(report: { what: string; steps: string }): Promise<void> {
		if (feedbackSending) return;
		feedbackSending = true;
		feedbackResult = undefined;
		const payload = buildBugReport({
			what: report.what,
			steps: report.steps,
			area: AREA_OTHER,
			labels: feedbackLabels(m),
			facts: deviceFacts
		});
		const outcome = await sendBugReport(payload);
		feedbackSending = false;
		feedbackResult = outcome.ok
			? { filed: true, number: outcome.number, url: outcome.url, deduped: outcome.deduped }
			: { filed: false, fallbackUrl: outcome.fallbackUrl };
	}

	// --- The Ethereum backup row (spec 062) ---------------------------------
	//
	// Where the active wallet's founding record stands on Ethereum, asked of the
	// chain (never of our server) each time this page meets an account — and
	// again whenever a backup request settles, because the chain is what knows
	// whether it landed.
	let backupState = $state<EthereumBackupState | 'checking'>('checking');
	let backupAsked = $state(0);
	$effect(() => {
		const account = session.view.accounts[session.view.active_index]?.account;
		void backupAsked;
		if (!account) return;
		backupState = 'checking';
		let cancelled = false;
		void checkEthereumBackup(account.address, foundingKeyOf(account)).then((check) => {
			if (!cancelled) backupState = check.state;
		});
		return () => {
			cancelled = true;
		};
	});

	// Which passkeys control the active wallet — read from the registry contract
	// each time this page meets an account; `null` while the first answer is in
	// flight. The device's own record is the floor it falls back to.
	let walletKeys = $state<WalletKeys | null>(null);
	$effect(() => {
		const account = session.view.accounts[session.view.active_index]?.account;
		if (!account) return;
		walletKeys = null;
		let cancelled = false;
		void readWalletKeys(account).then((keys) => {
			if (!cancelled) walletKeys = keys;
		});
		return () => {
			cancelled = true;
		};
	});

	/**
	 * The backup is signed on THIS page (founder, 2026-09-18: tapping the row
	 * used to land a person on the wallet home with a sheet over it). The
	 * signing machine is app-resident; this route mounts a host for its sheet
	 * and an idle fee session, exactly as the extension's request window does.
	 */
	const feeQuote = new FeeQuote();
	onMount(() => () => feeQuote.dispose());
	let backupOpening = false;
	function startBackup(): void {
		// "Could not check" asks again rather than starting a backup: there is
		// nothing to sign until we know whether it is already backed up, and
		// the same effect below does the asking.
		if (backupState === 'could_not_check') {
			backupAsked += 1;
			return;
		}
		const account = session.view.accounts[session.view.active_index]?.account;
		if (!account || backupOpening) return;
		backupOpening = true;
		void (async () => {
			try {
				await signRequest.boot();
				signRequest.syncNetworks();
				signRequest.syncAccounts();
				const outcome = await startEthereumBackup(account);
				if (outcome.kind === 'requested') await outcome.settled;
			} finally {
				backupOpening = false;
				backupAsked += 1;
			}
		})();
	}

	/**
	 * The language row's own text: the endonym, plus "· system" when the person
	 * has not pinned one. The web's language is the URL, so choosing one
	 * NAVIGATES — every locale is its own prerendered page.
	 */
	const languageValue = $derived.by(() => {
		const pinned = preferences.language;
		const shown = pinned === 'auto' ? data.locale : pinned;
		const endonym = LOCALE_ENDONYMS.find((l) => l.id === shown)?.label ?? shown;
		return pinned === 'auto' ? `${endonym} · ${m.common.system}` : endonym;
	});

	/** The one translation table: what the person did → what is stored. */
	function onPrefEvent(event: SettingsPrefEvent): void {
		switch (event.kind) {
			case 'theme': {
				const choice = themeFromSegment(event.id);
				if (choice !== undefined) preferences.setTheme(choice);
				return;
			}
			case 'language': {
				// `system` unpins and follows the browser again; anything else is a
				// locale, and on the web a locale is a route.
				const chosen = event.id === 'system' ? 'auto' : event.id;
				preferences.setLanguage(chosen);
				const target = chosen === 'auto' ? data.locale : chosen;
				if (target !== data.locale && SUPPORTED_LOCALES.includes(target as Locale)) {
					void goto(resolve('/[locale]/settings', { locale: target as Locale }));
				}
				return;
			}
			case 'text-scale':
				preferences.setTextScaleIndex(event.index);
				return;
			case 'number-format':
				preferences.setNumberFormat(event.id as 'auto');
				return;
			case 'date-format':
				preferences.setDateFormat(event.id as 'auto');
				return;
			case 'time-format':
				preferences.setTimeFormat(event.id as 'auto');
				return;
			case 'currency':
				// One of the two preferences with a core behind it: the committed
				// pair reaches every money surface through `display_currency`.
				currency.choose(event.id);
				return;
			case 'fee-speed':
				// The other (spec 068). The core validates the name — an id this
				// build does not offer is refused there rather than reaching
				// storage and, later, the relay (which answers -32602).
				if (event.id === 'fast' || event.id === 'standard' || event.id === 'slow') {
					feeTierPreference.choose(event.id);
				}
				return;
			// Spec 071: how every signature starts, and the Trusted Signer's page.
			// The core refuses a method it does not offer and an address it would
			// not open (saying why in its view); nothing is judged here.
			case 'sign-with':
				signPreference.chooseMethod(event.id);
				return;
			case 'signer-page':
				signPreference.submitSignerUrl(event.text);
				return;
			case 'signer-page-reset':
				signPreference.resetSignerUrl();
				return;
			case 'erase':
				void erase();
				return;
		}
	}

	/**
	 * Erase, then leave (spec 028 T434).
	 *
	 * The order matters and it is the module's: the sweep VERIFIES before it
	 * resolves, so a rejected promise means data is still here — and a person
	 * sent to first run over a partial wipe would have been told their browser
	 * is clean when it is not. On failure they stay signed in, on this sheet,
	 * with the reason in the sheet's own callout, and the button still live.
	 */
	async function erase(): Promise<void> {
		eraseFailed = false;
		try {
			await eraseDeviceData();
		} catch (error) {
			console.error('[erase] incomplete:', error);
			eraseFailed = true;
			return;
		}
		// Nothing of this wallet is left to read, so the session machine's own
		// view is stale by construction: a full reload is the first run.
		location.assign(welcome);
	}

	/**
	 * Cut a site off — or all of them, from the fixture row that stands when
	 * none is connected. Revoking is the ABSENCE of a grant, which is why this
	 * asks no machine: what a grant means is `dapp_permissions`', and it will
	 * rule on the next request exactly as it rules on a first one.
	 */
	async function disconnect(id: string): Promise<void> {
		if (id === 'dapps') await revokeAll();
		else await revokeGrant(id);
		await refreshGrants();
	}

	// --- The storage page's own numbers (spec 028 Phase 8) --------------------
	//
	// Measured on entry and after every clear; until the first measurement the
	// drawn fixture figures stand, exactly as they did before this phase.
	let storageReport = $state<DeviceStorageReport | null>(null);
	async function refreshStorage(): Promise<void> {
		storageReport = await measureDeviceStorage();
	}

	/**
	 * A storage row's action. The dApp rows are grants (above); every other
	 * drawn row is a key list the service owns, cleared and re-measured here.
	 */
	async function clearRow(id: string): Promise<void> {
		const isItem = (STORAGE_ITEM_IDS as readonly string[]).includes(id);
		if (!isItem || id === 'dapps') {
			await disconnect(id);
			return;
		}
		await clearStorageItem(id as Exclude<StorageItemId, 'dapps'>);
		await refreshStorage();
	}

	async function clearCaches(): Promise<void> {
		await clearAllCaches();
		await refreshStorage();
	}

	// --- The account switcher (spec 028 Phase 8) ------------------------------
	//
	// Rows are the session's own, in its order; the totals are the balance
	// core's switcher cache, with the live total standing in for the active
	// account's. A pick is answered by the session, which persists the index.
	const accountsInput = $derived.by(() => {
		const balances = new SvelteMap<string, number>();
		for (const entry of balance.view.switcher.balances) {
			balances.set(entry.address.toLowerCase(), entry.usd);
		}
		if (balance.view.display_total_usd !== null && view.address !== '') {
			balances.set(view.address.toLowerCase(), balance.view.display_total_usd);
		}
		return {
			rows: view.accounts,
			activeIndex: view.active_index,
			balances,
			currency: currency.view,
			identicon: identiconSvgForClient
		};
	});

	function selectAccount(position: number): void {
		const row = view.accounts[position];
		if (row !== undefined) session.switchAccount(row.index);
	}

	/** The switcher is on screen: the balance core refreshes every row's total. */
	function accountsOpen(open: boolean): void {
		if (open) balance.openSwitcher(view.accounts.map((row) => row.account.address));
		else balance.closeSwitcher();
	}

	/** Which network's editor is open — render state; the ledger is the core's. */
	let selectedNetworkId = $state<string | undefined>(undefined);

	const net = $derived(networkAdmin.view);
	const m = $derived(data.settingsMessages);

	/** Fixture base → identity overlay → live network sections (research D7). */
	const liveHome = $derived.by(() => {
		if (identity === null) return data.home;
		let model = withLiveAccounts(homeWithIdentity(data.home, identity), accountsInput, m);
		model = withLiveNetworks(model, net, m, selectedNetworkId);
		if (storageReport !== null) model = withLiveStorage(model, storageReport, m);
		model = withLiveCurrency(model, currency.view, currencyCatalog);
		model = withLiveFeeSpeed(model, feeTierPreference.view);
		model = withLiveSigning(model, signPreference.view);
		// After the storage numbers: the connections row is the grants', not a key count.
		model = withLiveConnections(model, grants, m);
		model = withLivePreferences(model, m, languageValue, data.locale);
		model = withEraseFailure(model, m, eraseFailed);
		model = withFeedback(model, m, deviceFacts);
		model = { ...model, keys: walletKeysModel(walletKeys, backupState, m) };
		// The phone's first block (founder, 2026-09-05): 通讯录 is a tab on the
		// bar under this very screen, and 反馈 is not wanted here — so the block
		// they made up goes with them. The desktop nav keeps its own list.
		//
		// Spec 081 FR-016 puts 反馈 back on the screen, in the LAST block rather
		// than the first: the ruling was about that block, and a report button
		// reachable only from `/gallery` is a report button nobody can press.
		// Beside 关于, because "something is wrong" and "what is this" are the
		// same errand and the person looks in the same place for both.
		const feedbackRow = model.sections
			.flatMap((section) => section.rows)
			.find((row) => row.id === 'feedback');
		const trimmed = model.sections
			.map((section) => ({
				...section,
				rows: section.rows.filter((row) => row.id !== 'contacts' && row.id !== 'feedback')
			}))
			.filter((section) => section.rows.length > 0 || section.appearanceControls === true);
		const last = trimmed.length - 1;
		return {
			...model,
			sections:
				feedbackRow === undefined || last < 0
					? trimmed
					: trimmed.map((section, index) =>
							index === last ? { ...section, rows: [...section.rows, feedbackRow] } : section
						)
		};
	});
	const liveDesktop = $derived.by(() => {
		if (identity === null) return data.desktop;
		let model = withLiveAccountsDesktop(
			desktopWithIdentity(data.desktop, identity),
			accountsInput,
			m
		);
		model = withLiveNetworksDesktop(model, net, m, selectedNetworkId);
		if (storageReport !== null) model = withLiveStorage(model, storageReport, m);
		// After the storage numbers, as on the phone: the connections row is the
		// grants', not the drawn "4 sites" (spec 028 Phase 9, T485).
		model = withLiveConnections(model, grants, m);
		model = withLiveCurrencyDesktop(model, currency.view, currencyCatalog);
		// The desktop page reuses the phone sheet's rows — one list of tiers,
		// one set of words, whichever layout is showing.
		model = withLiveFeeSpeedDesktop(model, feeTierPreference.view, liveHome.feeSpeedSheet);
		model = withLiveSigningDesktop(model, signPreference.view, liveHome.signWithSheet);
		model = {
			...model,
			account: { ...model.account, keys: walletKeysModel(walletKeys, backupState, m) }
		};
		model = withEraseFailure(model, m, eraseFailed);
		// Spec 081: the wide layout's own two dead controls — the danger card
		// with no handler, and a report panel that did not exist.
		model = withEraseFailureDesktop(model, m, eraseFailed);
		model = withFeedbackDesktop(model, m, deviceFacts);
		return withLivePreferencesDesktop(model, m, languageValue, data.locale);
	});

	/**
	 * The one translation table: what the person did → what the core is told.
	 * No decisions — the core refuses, coalesces, or persists as its rules say.
	 */
	function onNetEvent(event: SettingsNetEvent): void {
		const chainOf = (id: string) => net.networks.find((row) => row.id === id)?.chain_id;
		switch (event.kind) {
			case 'select-network': {
				selectedNetworkId = event.id;
				const chainId = chainOf(event.id);
				if (chainId !== undefined)
					networkAdmin.dispatch({ type: 'override_expanded', chain_id: chainId });
				return;
			}
			case 'delete-network':
				networkAdmin.dispatch({ type: 'delete_confirmed', id: event.id });
				return;
			case 'detail-field': {
				const chainId = selectedNetworkId === undefined ? undefined : chainOf(selectedNetworkId);
				if (chainId !== undefined)
					networkAdmin.dispatch({
						type: 'override_field_edited',
						chain_id: chainId,
						field: event.field,
						value: event.value
					});
				return;
			}
			case 'detail-blur': {
				const chainId = selectedNetworkId === undefined ? undefined : chainOf(selectedNetworkId);
				if (chainId !== undefined)
					networkAdmin.dispatch({ type: 'override_blurred', chain_id: chainId });
				return;
			}
			case 'open-add':
				networkAdmin.dispatch({ type: 'wizard_reset' });
				return;
			case 'search':
				networkAdmin.dispatch({ type: 'search_input', query: event.query });
				return;
			case 'pick-suggestion':
				networkAdmin.dispatch({
					type: 'chain_selected',
					chain_id: event.chainId,
					keep_custom_rpc: false
				});
				return;
			case 'custom-rpc':
				networkAdmin.dispatch({ type: 'custom_rpc_edited', value: event.value });
				return;
			case 'confirm-add':
				networkAdmin.dispatch({ type: 'add_confirmed', now_iso: new Date().toISOString() });
				return;
			case 'recheck': {
				const chainId = net.wizard.chain_info?.chain_id;
				if (chainId !== undefined)
					networkAdmin.dispatch({
						type: 'chain_selected',
						chain_id: chainId,
						keep_custom_rpc: true
					});
				return;
			}
			case 'endpoints-open':
				networkAdmin.dispatch({ type: 'endpoints_opened' });
				return;
			case 'endpoint':
				networkAdmin.dispatch({ type: 'endpoint_edited', field: event.field, value: event.value });
				return;
			case 'endpoint-blur':
				networkAdmin.dispatch({ type: 'endpoint_blurred', field: event.field });
				return;
			case 'endpoints-reset':
				networkAdmin.dispatch({ type: 'reset_endpoints_to_defaults' });
				return;
			case 'providers-open':
				networkAdmin.dispatch({ type: 'providers_opened' });
				return;
			case 'provider-key':
				networkAdmin.dispatch({
					type: 'provider_key_edited',
					provider: event.provider,
					value: event.value
				});
				return;
			case 'provider-blur':
				networkAdmin.dispatch({ type: 'provider_key_blurred', provider: event.provider });
				return;
			case 'provider-test':
				networkAdmin.dispatch({ type: 'provider_test_requested', provider: event.provider });
				return;
		}
	}

	// The guard's other half. `loading` is deliberately not acted on: the core
	// has not ruled yet, and bouncing on a non-answer would throw a reloading
	// person back to Welcome mid-boot.
	$effect(() => {
		if (view.allowed_route === 'onboarding') void goto(welcome, { replaceState: true });
	});

	/**
	 * The tab bar. 钱包 and 通讯录 have routes (spec 024 gave the book its
	 * own); 探索 has none on web by decision (spec 022), so it stays put.
	 */
	function selectTab(id: 'wallet' | 'contacts' | 'explore' | 'settings') {
		if (id === 'wallet') void goto(walletHref);
		else if (id === 'contacts') void goto(contactsHref);
	}

	function signOut() {
		session.signOut();
	}
</script>

<svelte:head>
	<title>{data.settingsMessages.title}</title>
	<meta name="robots" content="noindex" />
</svelte:head>

{#if identity}
	{#if wide.current}
		<div class="desktop-shell">
			<SettingsDesktop
				model={liveDesktop}
				{sidebar}
				onnav={selectTab}
				onsignout={signOut}
				onnetevent={onNetEvent}
				onprefevent={onPrefEvent}
				onaccountselect={selectAccount}
				onaccountcreate={() => void goto(createHref)}
				onaccountsignin={() => void goto(welcome)}
				onaccountsopen={accountsOpen}
				onstorageclear={clearRow}
				onclearcaches={clearCaches}
				onfeedbacksend={(report) => void sendFeedback(report)}
				{feedbackSending}
				{feedbackResult}
				onethereumbackup={startBackup}
			/>
		</div>
	{:else}
		<main class="page">
			<SettingsHome
				model={liveHome}
				destinations={WEB_DESTINATIONS}
				onselecttab={selectTab}
				onsignout={signOut}
				onnetevent={onNetEvent}
				oncurrencyselect={(code) => currency.choose(code)}
				onstorageclear={clearRow}
				onprefevent={onPrefEvent}
				onaccountselect={selectAccount}
				onaccountcreate={() => void goto(createHref)}
				onaccountsignin={() => void goto(welcome)}
				onaccountsopen={accountsOpen}
				onclearcaches={clearCaches}
				onfeedbacksend={(report) => void sendFeedback(report)}
				{feedbackSending}
				{feedbackResult}
				onethereumbackup={startBackup}
			/>
		</main>
	{/if}
{:else}
	<!-- The core has not ruled yet. An empty surface, not a fixture account. -->
	<div class="waiting" aria-busy="true"></div>
{/if}

<IdenticonViewerHost copy={data.identiconViewer} />

<!-- The way out of a signed-in wallet. The row is on this route, so the core's
     sheet — the one carrying the pending-upload warning — has to be here too;
     rendering it only on the wallet route left the confirm button doing
     nothing at all (issue 214). -->
<SignOutHost copy={data.signOut} />

<!--
	The wallet's own request to copy its keys to Ethereum is answered HERE
	(spec 062). Same host, same four machines, same sheet as a page's request.
-->
<!--
	Spec 077: the backup to Ethereum is a transaction, and it lands here
	like any other — it posts into the same seam a dApp does, so it gets the
	same receipt rather than a second one written for it.
-->
<SigningHost
	messages={data.signingMessages}
	fee={feeQuote}
	receipt={data.signingMessages.receipt}
/>

<style>
	/* The phone screens are `height: 100%` of whatever holds them, and the
	   root layout only sets a MIN-height — so without a frame the screen
	   stood as tall as its own rows, the DOCUMENT scrolled, and the tab bar
	   scrolled away with it (founder, 2026-09-05). The contacts route frames
	   its phone the same way. */
	.page {
		height: 100dvh;
		display: flex;
		flex-direction: column;
		background: var(--color-bg-base);
	}

	.waiting {
		min-height: 100dvh;
		background: var(--color-bg-base);
	}

	/* The three columns are `height: 100%` of whatever holds them, and nothing
	   above this page has a height — the body has a MIN-height — so without a
	   frame they stood as tall as their own rows and the sunken sidebars ended
	   mid-screen. The wallet route frames its desktop the same way. */
	.desktop-shell {
		height: 100dvh;
		overflow: hidden;
	}
</style>
