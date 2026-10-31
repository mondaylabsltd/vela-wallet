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
	 * - **Still canon data**: the latency figures and the storage accounting
	 *   — those wait for the features that measure them.
	 */
	import CoreWaiting from '$lib/ui/CoreWaiting.svelte';
	import { onMount } from 'svelte';
	import { replaceUrl } from '$lib/ui/history';
	import { goto } from '$app/navigation';
	import { STAY_QUERY } from '$lib/intro/gate';
	import { resolve } from '$app/paths';
	import { MediaQuery } from 'svelte/reactivity';
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
	import {
		CHECKING_ROW,
		checkEthereumBackup,
		lastBackupRow,
		type EthereumBackupRow
	} from '$lib/services/registry-backup';
	import {
		deviceOnlyKeys,
		keysWorthAskingAgain,
		readWalletKeys,
		type WalletKeys
	} from '$lib/services/wallet-keys';
	import { networkAdmin } from '$lib/settings/core/network-admin.svelte';
	import { currency } from '$lib/settings/core/currency.svelte';
	import { feeTierPreference } from '$lib/settings/core/fee-tier.svelte';
	import { signingVenueBlock, signingVenueChoices } from '$lib/core/kernels';
	import { venueModel } from '$lib/settings/venue';
	import { track } from '$lib/analytics';
	import { analyticsConsent } from '$lib/analytics/consent.svelte';
	import {
		agentSetUp,
		agentSettings,
		bridgeUrlProblem,
		mcpEndpoint
	} from '$lib/agent/settings.svelte';
	import { agentLink, type LinkProblem } from '$lib/agent/link.svelte';
	import type { AgentMessages } from '$lib/agent/messages';
	import { page } from '$app/state';
	import { fill } from '$lib/wallet/messages';
	import type {
		AgentPanelModel,
		SettingsRowModel,
		SitesPanelModel,
		StatusPillModel,
		SwitchRowModel
	} from '$lib/settings/model';
	import { hostLabel } from '$lib/dapp/host';
	import {
		accountAmount,
		accountLoading,
		switcherBalances,
		withLiveAccounts,
		withLiveAccountsDesktop,
		withLiveConnections,
		withLiveCurrency,
		withLiveCurrencyDesktop,
		withLiveFeeSpeed,
		withLiveFeeSpeedDesktop,
		withLiveNetworks,
		withLiveNetworksDesktop,
		withLiveStorage
	} from '$lib/settings/live';
	import type { LiveOwnNodes } from '$lib/settings/live';
	import { ownNodes } from '$lib/services/own-node.svelte';
	import { listGrants, revokeAll, revokeGrant } from '$lib/dapp/connections';
	import {
		measureDeviceStorage,
		STORAGE_ITEM_IDS,
		type DeviceStorageReport
	} from '$lib/services/device-storage';
	import { ERASE_HELD } from '$lib/services/erase-device';
	import {
		addNetworkEntered,
		type EraseFailure,
		feedbackLabels,
		themeFromSegment,
		withEraseFailure,
		withEraseFailureDesktop,
		withEraseUnsynced,
		withFeedback,
		withFeedbackDesktop,
		walletKeysModel,
		withLivePreferences,
		withLivePreferencesDesktop
	} from '$lib/settings/live';
	import { preferences, TEXT_SCALE_LEVELS } from '$lib/services/preferences.svelte';
	import { intents } from '$lib/intents/index.svelte';
	import { screenDesk } from '$lib/flows/screen-desk.svelte';
	import type { IntentScreen } from '$lib/core/generated/IntentScreen';
	import type { IntentPreference } from '$lib/core/generated/IntentPreference';
	import { loadCurrencyCodes } from '$lib/settings/core/currency-catalog';
	import { negotiateLanguages, SUPPORTED_LOCALES, type Locale } from '$lib/i18n/locales';
	import {
		AREA_OTHER,
		buildBugReport,
		readWorkerFailureLines,
		type DeviceFacts
	} from '$lib/services/bug-report';
	import { reportSend } from '$lib/settings/report-send.svelte';
	import { deviceFacts as currentDeviceFacts } from '$lib/settings/device-facts';
	import { fileReport } from '$lib/settings/file-report';
	import type { SettingsPrefEvent } from '$lib/settings/pref-events';
	import { LOCALE_ENDONYMS } from '$lib/settings/fixtures';
	import type { SettingsNetEvent } from '$lib/settings/net-events';
	import { identiconSvgForClient } from '$lib/wallet/identicon';
	import { shortenAddress, type WalletIdentity } from '$lib/wallet/identity';
	import { WEB_DESTINATIONS } from '$lib/wallet/destinations';
	import { balance } from '$lib/wallet/core/balance.svelte';
	import { networkSpace } from '$lib/network-space/network-space.svelte';
	import type { LiveSpace } from '$lib/settings/live';
	import type { PageProps } from './$types';
	import { shownAddress, shownIdentity } from '$lib/multisig/core/shown';
	import { multisigName } from '$lib/multisig/core/display';
	import { multisig } from '$lib/multisig/core/store.svelte';
	import MultisigHost from '$lib/multisig/ui/MultisigHost.svelte';
	import MultisigRows from '$lib/multisig/ui/MultisigRows.svelte';
	import MultisigSettings from '$lib/multisig/ui/MultisigSettings.svelte';
	import { multisigUi } from '$lib/multisig/core/ui.svelte';
	import Button from '$lib/ui/Button.svelte';

	let { data }: PageProps = $props();

	/**
	 * Welcome, on purpose (ob-6): sent there with no wallet, or to sign into
	 * another one — so it does not hand the visit straight back to the wallet.
	 */
	const welcome = $derived(resolve(`/[locale]${STAY_QUERY}`, { locale: data.locale }));
	const createHref = $derived(resolve('/[locale]/create', { locale: data.locale }));
	const walletHref = $derived(resolve('/[locale]/wallet', { locale: data.locale }));
	const contactsHref = $derived(resolve('/[locale]/contacts', { locale: data.locale }));
	const wide = new MediaQuery(`(min-width: ${BREAKPOINT_DESKTOP}px)`, false);

	const view = $derived(session.view);
	const signedIn = $derived(view.allowed_route === 'wallet');

	// A multisig shown is the account these settings are for, as on the wallet.
	const shownMultisig = $derived(signedIn ? multisig.active : null);
	const identity = $derived<WalletIdentity | null>(
		signedIn ? shownIdentity(view, data.multisigMessages, data.locale) : null
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
						identiconSvg: identity.identiconSvg,
						...(identity.rule ? { rule: identity.rule } : {}),
						...(identity.waiting ? { waiting: identity.waiting } : {})
					}
					// No network list: it is the wallet's filter (spec 028 Phase 9,
					// RULING 2), and a click that left this page for the wallet was a
					// surprise beside rows that stay.
				}
	);

	// Noted, not read (perf-10): this page draws a balance only in the account
	// switcher, and the core reads the networks when it opens.
	$effect(() => {
		if (identity !== null) void balance.noteAccount(identity.address);
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
		void balance.boot();
		preferences.boot();
		analyticsConsent.boot();
		agentSettings.boot();
		// The root layout dialled already when it was on; this only learns of the connection.
		if (agentSettings.enabled) syncAgent();
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
	let eraseFailed = $state<EraseFailure | false>(false);

	// --- The report (spec 081 FR-016; 078) ------------------------------------
	//
	// The sheet's 发送 was inert until 081: `FeedbackBody` has always taken an
	// `onsend` and this page never passed one. What is in flight, how the last
	// one ended, and what was typed and attached all live in `reportSend` —
	// app-resident, because the person may close the sheet (or leave this
	// page) while the report is sending, and the send, its outcome and the
	// words must all outlive the sheet (078: the outcome is then a toast).

	/**
	 * What this device is allowed to say about itself, and nothing more.
	 *
	 * Five named fields. No address, no balance, no endpoint or RPC URL — the
	 * last because a self-hosted endpoint carries an API key in its path often
	 * enough that a public issue tracker is the wrong place for one — and no
	 * raw `vela.*` value. The failure lines are the net counters' CLASSES
	 * (`rpc:final_failure ×3`), which say what broke without saying where.
	 */
	/** The extension worker's failure counters (spec 082 RB14), read once per visit. */
	let workerFailures = $state<string[]>([]);
	onMount(() => {
		void readWorkerFailureLines().then((lines) => (workerFailures = lines));
	});
	const deviceFacts = $derived.by<DeviceFacts>(() =>
		currentDeviceFacts(data.locale, workerFailures, m.bugReport.chainFallback)
	);

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
	async function sendFeedback(report: { what: string; screenshots: string[] }): Promise<void> {
		// Read now: the words of the page that sent, for a toast that may be
		// shown on another page.
		const copy = {
			filed: m.bugReport.thanksTitle,
			view: m.bugReport.viewIssue,
			fellBack: m.bugReport.fallbackTitle,
			open: m.bugReport.openGithub,
			close: m.common.close
		};
		const payload = buildBugReport({
			// One box since round 3: the steps, if any, are in the words —
			// the payload's `steps` goes as the empty string it always could be.
			what: report.what,
			area: AREA_OTHER,
			labels: feedbackLabels(m),
			facts: deviceFacts,
			screenshots: report.screenshots
		});
		await reportSend.send(() => fileReport(payload), copy);
	}

	// --- The Ethereum copy's row (spec 062) ---------------------------------
	//
	// Where the active wallet's record stands on Ethereum, asked of the chain
	// (never of our server) each time this page meets an account — and again
	// whenever a copy request settles, because the chain is what knows whether
	// it landed. What is kept is the ROW the core says to draw for where it
	// stands (`registry_backup::BackupRow`): its words, its tone and what a tap
	// does. `null` is "draw nothing" (no registry on Ethereum, no record).
	//
	// While the chain is asked, the block stands on the row it drew last time
	// for this account (perf-8): "Checking…" with its paragraph, then nothing
	// at all for a wallet with no record to copy, dropped everything under it
	// by a block's height. Asked AGAIN on this visit — a retry, a copy that
	// settled — it says "Checking…": the person is waiting on that answer.
	let backupRow = $state<EthereumBackupRow | null>(CHECKING_ROW);
	let backupAsked = $state(0);
	// Which account, and which ask, the row was last drawn for. The effect also
	// re-runs whenever the session's view moves: the same account and the same
	// ask keep the row on screen while the chain is asked again.
	let backupDrawn: { account: string; asked: number } | null = null;
	$effect(() => {
		const account = session.view.accounts[session.view.active_index]?.account;
		const asked = backupAsked;
		if (!account) return;
		const foundingKey = foundingKeyOf(account);
		const drawnFor = `${account.address}#${foundingKey}`;
		const sameAccount = backupDrawn?.account === drawnFor;
		const sameAsk = sameAccount && backupDrawn?.asked === asked;
		backupDrawn = { account: drawnFor, asked };
		let cancelled = false;
		if (!sameAsk) {
			const remembered = sameAccount ? undefined : lastBackupRow(account.address, foundingKey);
			backupRow = remembered === undefined ? CHECKING_ROW : remembered;
		}
		void checkEthereumBackup(account.address, foundingKey).then((check) => {
			if (!cancelled) backupRow = check.row;
		});
		return () => {
			cancelled = true;
		};
	});

	// Which passkeys control the active wallet — read from the registry contract
	// each time this page meets an account. The device's own record stands in
	// the block from the first frame (`deviceOnlyKeys`: its rows, marked as
	// still being asked), and is the floor the answer falls back to: the block
	// was one grey row until the registry answered, and then everything under
	// it dropped by the rest of the keys (device walk, item 13.1).
	let walletKeys = $state<WalletKeys | null>(null);
	$effect(() => {
		const account = session.view.accounts[session.view.active_index]?.account;
		if (!account) return;
		walletKeys = deviceOnlyKeys(account);
		let cancelled = false;
		void readWalletKeys(account).then((keys) => {
			if (!cancelled) walletKeys = keys;
		});
		return () => {
			cancelled = true;
		};
	});

	/**
	 * ob-11: "Couldn't reach the registry" is a fact about a moment, not about
	 * the wallet. When the network is back — or the person returns to the
	 * page — the registry is asked again, behind the rows already shown (no
	 * flash back to "asking"), and the block takes the answer.
	 */
	let rereadingKeys = false;
	function rereadKeys() {
		const account = session.view.accounts[session.view.active_index]?.account;
		if (!account || rereadingKeys || !keysWorthAskingAgain(walletKeys)) return;
		if (typeof document !== 'undefined' && document.visibilityState === 'hidden') return;
		rereadingKeys = true;
		void readWalletKeys(account)
			.then((keys) => {
				const still = session.view.accounts[session.view.active_index]?.account;
				if (still?.address === account.address && keys.source !== 'device') walletKeys = keys;
			})
			.finally(() => {
				rereadingKeys = false;
			});
	}

	/**
	 * The backup is signed on THIS page (founder, 2026-09-18: tapping the row
	 * used to land a person on the wallet home with a sheet over it). The
	 * signing machine is app-resident; this route mounts a host for its sheet
	 * and an idle fee session, exactly as the extension's request window does.
	 */
	const feeQuote = new FeeQuote();
	onMount(() => () => feeQuote.dispose());

	let desktopSettings = $state<SettingsDesktop | undefined>();
	let phoneSettings = $state<SettingsHome | undefined>();

	/** A settings page by name, for the intents (spec 103): the layout's own nav. */
	async function present(screen: IntentScreen): Promise<string | null> {
		if (screen.type !== 'settings' || screen.page === null) return null;
		const layout = () => (wide.current ? desktopSettings : phoneSettings);
		const deadline = Date.now() + 5_000;
		while (layout() === undefined && Date.now() < deadline)
			await new Promise((done) => setTimeout(done, 50));
		const shown = layout();
		if (shown === undefined) return 'settings did not open';
		return shown.open(screen.page) ? null : 'this layout has no such settings page';
	}
	onMount(() => screenDesk.attach(['settings'], present));

	// `?addNetwork=<chainId>` (a multisig's proposal on a network this device
	// lacks): Add network, that chain filled in — the wizard looks it up in
	// the chain index, and asks for an RPC when the index does not know it.
	onMount(() => {
		const asked = new URLSearchParams(location.search).get('addNetwork') ?? '';
		if (!/^[1-9]\d{0,15}$/.test(asked)) return;
		const chainId = Number(asked);
		replaceUrl(location.pathname + location.hash);
		void present({ type: 'settings', page: 'add-network' }).then((failed) => {
			if (failed === null)
				networkAdmin.dispatch({
					type: 'chain_selected',
					chain_id: chainId,
					keep_custom_rpc: false
				});
		});
	});

	// Spec 106: back from clientmcp.dev — finish the sign-in where it began
	// (its language), then show the AI agent page with the result.
	onMount(() => {
		const back = agentLink.returned();
		if (!back.here) return;
		if (back.elsewhere !== null) {
			location.replace(back.elsewhere);
			return;
		}
		void agentLink
			.finish(async (linked) => {
				// E8: the first connection offers the wallet in front; a list the
				// person already chose is kept.
				const active = shownAddress(session.view.address);
				await intents.invoke('person', {
					type: 'configure_agent',
					enabled: true,
					bridge_url: linked.bridge_url,
					client_token: linked.client_token,
					...(agentSettings.wallets.length === 0 && active !== '' && { wallets: [active] })
				});
				syncAgent();
			})
			.then(() => present({ type: 'settings', page: 'agent' }));
	});
	let backupOpening = false;
	function startBackup(): void {
		// What a tap does is the core's (`BackupRow.action`). `retry` — only
		// "couldn't check" — asks again rather than starting a copy: there is
		// nothing to sign until we know whether it is already there, and the
		// effect above does the asking. `none` is a statement: copied, still
		// checking, or an older wallet that can never be copied — which used
		// to read "could not check" and be asked again for ever.
		if (backupRow === null || backupRow.action === 'none') return;
		if (backupRow.action === 'retry') {
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
				if (outcome.kind === 'requested') {
					track('backup_keys_started');
					const settled = await outcome.settled;
					if (settled.kind === 'submitted') track('backup_keys_done');
				}
			} finally {
				backupOpening = false;
				backupAsked += 1;
			}
		})();
	}

	/**
	 * What "Follow system" follows: the browser's own languages, resolved to a
	 * locale we ship (set-13) — not the page's, which is only where the person
	 * happens to be. Read on mount: the prerendered page has no navigator.
	 */
	let browserLocale = $state<Locale | null>(null);
	const systemLocale = $derived(browserLocale ?? data.locale);
	onMount(() => {
		browserLocale = negotiateLanguages(navigator.languages);
	});

	/**
	 * The language row's own text: the endonym, plus "· system" when the person
	 * has not pinned one. The web's language is the URL, so choosing one
	 * NAVIGATES — every locale is its own prerendered page.
	 */
	const languageValue = $derived.by(() => {
		const pinned = preferences.language;
		const shown = pinned === 'auto' ? systemLocale : pinned;
		const endonym = LOCALE_ENDONYMS.find((l) => l.id === shown)?.label ?? shown;
		return pinned === 'auto' ? `${endonym} · ${m.common.system}` : endonym;
	});

	/** The one translation table: what the person did → what is stored. */
	function onPrefEvent(event: SettingsPrefEvent): void {
		switch (event.kind) {
			case 'theme': {
				const choice = themeFromSegment(event.id);
				if (choice !== undefined) {
					prefer({ kind: 'theme', value: choice });
					track('setting_theme_changed', { theme: choice });
				}
				return;
			}
			case 'language': {
				// `system` unpins and follows the browser again; anything else is a
				// locale, and on the web a locale is a route (the intent goes there).
				const chosen = event.id === 'system' ? 'auto' : event.id;
				const target = chosen === 'auto' ? systemLocale : chosen;
				// Before the navigation: the page is about to become another locale's.
				const locale = SUPPORTED_LOCALES.find((code) => code === target);
				if (locale !== undefined) track('setting_language_changed', { locale });
				prefer({ kind: 'language', value: chosen });
				return;
			}
			case 'text-scale': {
				const level = TEXT_SCALE_LEVELS[event.index]?.key;
				if (level !== undefined) prefer({ kind: 'text_scale', value: level });
				return;
			}
			case 'number-format':
				prefer({ kind: 'number_format', value: event.id });
				return;
			case 'date-format':
				prefer({ kind: 'date_format', value: event.id });
				return;
			case 'time-format':
				prefer({ kind: 'time_format', value: event.id });
				return;
			case 'currency':
				// One of the two preferences with a core behind it: the committed
				// pair reaches every money surface through `display_currency`.
				chooseCurrency(event.id);
				return;
			case 'fee-speed':
				// The other (spec 068). The core validates the name — an id this
				// build does not offer is refused there rather than reaching
				// storage and, later, the relay (which answers -32602).
				if (event.id === 'fast' || event.id === 'standard' || event.id === 'slow') {
					prefer({ kind: 'fee_speed', value: event.id });
					track('setting_fee_speed_changed', { speed: event.id });
				}
				return;
			// Spec 102: where the account reviews and signs — on the web, where
			// its multisig approvals go (2026-10-29). The core rules (R1).
			case 'signing-venue':
				if (view.address !== '') session.chooseVenue(view.address, event.venue);
				return;
			case 'analytics':
				prefer({ kind: 'analytics', value: event.on });
				return;
			case 'agent':
				configureAgent({ enabled: event.on, bridge_url: null, client_token: null });
				return;
			case 'agent-url':
				configureAgent({ enabled: null, bridge_url: event.value, client_token: null });
				return;
			case 'agent-token':
				configureAgent({ enabled: null, bridge_url: null, client_token: event.value });
				return;
			case 'agent-link':
				void agentLink.start(location.pathname, 'Web browser');
				return;
			case 'agent-wallet': {
				// The list whole, as the person now wants it (spec 107 E8).
				const others = agentSettings.wallets.filter(
					(w) => w.toLowerCase() !== event.address.toLowerCase()
				);
				configureAgent({
					enabled: null,
					bridge_url: null,
					client_token: null,
					wallets: event.on ? [...others, event.address] : others
				});
				return;
			}
			case 'agent-use-here':
				agentBridge?.useHere();
				return;
			case 'agent-unlink':
				// Forgotten here; the client stays on clientmcp.dev until deleted there.
				configureAgent({ enabled: false, bridge_url: '', client_token: '' });
				return;
			case 'erase':
				void erase();
				return;
		}
	}

	/** A preference, through the intents (spec 103) — the road an agent's takes too. */
	function prefer(preference: IntentPreference): void {
		void intents.invoke('person', { type: 'set_preference', preference });
	}

	/** The agent's own connection: the person's alone (spec 103). */
	function configureAgent(change: {
		enabled: boolean | null;
		bridge_url: string | null;
		client_token: string | null;
		wallets?: string[];
	}): void {
		void intents.invoke('person', { type: 'configure_agent', ...change }).then(syncAgent);
	}

	/** Both layouts' currency pick (the phone's sheet reports it on its own callback). */
	function chooseCurrency(code: string): void {
		prefer({ kind: 'currency', value: code });
		track('setting_currency_changed', { currency: code });
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
		// On success the intent leaves for the first run (`perform.ts`); a
		// refusal means data is still here, said in the sheet's own callout.
		const outcome = await intents.invoke('person', { type: 'erase_device' });
		if (outcome.status === 'rejected') {
			console.error('[erase] incomplete:', outcome.reason.detail);
			eraseFailed = outcome.reason.detail === ERASE_HELD ? 'held' : 'incomplete';
		}
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
		if (!isItem) {
			await disconnect(id);
			return;
		}
		// The intent clears it — the dApp grants too — and has the resident
		// address book read the cleared stores again.
		await intents.invoke('person', { type: 'clear_storage_item', item: id });
		if (id === 'dapps') await refreshGrants();
		await refreshStorage();
	}

	async function clearCaches(): Promise<void> {
		await intents.invoke('person', { type: 'clear_caches' });
		await refreshStorage();
	}

	// --- The account switcher (spec 028 Phase 8) ------------------------------
	//
	// Rows are the session's own, in its order; the totals are the balance
	// core's switcher cache, with the live total standing in for the active
	// account's. A pick is answered by the session, which persists the index.
	const accountsInput = $derived.by(() => {
		// The live total stands in for the active row only while it is a
		// figure — not hidden, not a home that read nothing (`switcherBalances`).
		// The live total is the account shown — a multisig's, when one is.
		const balances = switcherBalances(balance.view, shownAddress(view.address));
		const hidden = balance.view.switcher.hidden;
		return {
			rows: view.accounts,
			// A multisig shown: no Vela row is the active account.
			activeIndex: shownMultisig ? -1 : view.active_index,
			balances,
			hidden,
			currency: currency.view,
			identicon: identiconSvgForClient,
			others: multisig.accounts.map((a) => a.address),
			// home-9: no figure for test coins in the testnet space.
			noValue: networkSpace.testnet,
			// ms-24: a row the core is still reading is drawn as loading.
			loading: balance.view.switcher.loading,
			unsynced: view.unsynced
		};
	});

	function selectAccount(position: number): void {
		const row = view.accounts[position];
		multisig.activate(null);
		if (row !== undefined)
			void intents.invoke('person', { type: 'switch_account', address: row.account.address });
	}

	/**
	 * ob-14: the account's ×, confirmed — the switcher's own act (the core's
	 * `remove_wallet`), so a removal here and there is one thing.
	 */
	function removeAccount(position: number): void {
		const row = view.accounts[position];
		if (row !== undefined)
			void intents.invoke('person', { type: 'remove_wallet', address: row.account.address });
	}

	/** The switcher is on screen: the balance core refreshes every row's total. */
	function accountsOpen(open: boolean): void {
		if (open)
			balance.openSwitcher([
				...view.accounts.map((row) => row.account.address),
				...multisig.accounts.map((a) => a.address)
			]);
		else balance.closeSwitcher();
	}

	/** Which network's editor is open — render state; the ledger is the core's. */
	let selectedNetworkId = $state<string | undefined>(undefined);

	const net = $derived(networkAdmin.view);
	const m = $derived(data.settingsMessages);

	/**
	 * "Share anonymous usage statistics" — its live state. Beside the AI agent
	 * and the connected sites (owner, 2026-10-29): in 高级 on the phone, its
	 * own Privacy panel on the desktop.
	 */
	const analyticsRow = $derived<SwitchRowModel>({
		id: 'settings-analytics',
		icon: 'eye-off',
		title: m.analytics.title,
		subtitle: m.analytics.subtitle,
		on: analyticsConsent.enabled
	});

	// --- The AI agent (spec 103) ----------------------------------------------
	//
	// Web only: its row and nav entry are added here, not drawn by the shared
	// fixtures. The connection lives in `$lib/agent/bridge.svelte`, loaded on
	// first use so a wallet that never turns the agent on never carries it.

	let agentBridge = $state<(typeof import('$lib/agent/bridge.svelte'))['agentBridge'] | null>(null);

	/** Dial, redial or hang up to match what Settings now says. */
	function syncAgent(): void {
		void import('$lib/agent/bridge.svelte').then((bridge) => {
			agentBridge = bridge.agentBridge;
			bridge.agentBridge.sync();
		});
	}

	const agentStatus = $derived.by((): StatusPillModel => {
		const a = m.agent;
		if (!agentSettings.ready) return { tone: 'neutral', label: a.statusOff, dot: true };
		switch (agentBridge?.status ?? 'connecting') {
			case 'online':
				return { tone: 'ok', label: a.statusOnline, dot: true };
			case 'offline':
				return { tone: 'warn', label: a.statusOffline, dot: true };
			case 'closed':
				return { tone: 'error', label: a.statusRefused, dot: true };
			// Another tab of this wallet answers the agent (the bridge keeps one).
			case 'elsewhere':
				return { tone: 'neutral', label: a.statusElsewhere, dot: true };
			default:
				return { tone: 'accent', label: a.statusConnecting, dot: true };
		}
	});

	const agentPanel = $derived.by((): AgentPanelModel => {
		const a = m.agent;
		const url = agentSettings.url;
		const invalid = url.trim() !== '' && bridgeUrlProblem(url) !== null;
		const endpoint = mcpEndpoint(url);
		// The agent's latest call, by its action's name and how it ended — in
		// words, as the bar and the approval name it.
		const latest = intents.activity[0];
		const names = (page.data as { agent?: AgentMessages } | undefined)?.agent?.actions;
		const ended =
			latest?.outcome == null
				? a.lastWaiting
				: latest.outcome.status === 'rejected'
					? a.lastRefused
					: a.lastDone;
		const last =
			latest === undefined ? null : `${names?.[latest.action] ?? latest.action} · ${ended}`;
		return {
			title: a.title,
			description: a.description,
			enable: {
				id: 'settings-agent',
				title: a.enable,
				subtitle: a.subtitle,
				on: agentSettings.enabled
			},
			bridge: {
				id: 'agent-bridge',
				label: a.bridge,
				value: url,
				placeholder: 'wss://bridge.example.com/connect',
				hint: invalid ? a.bridgeInvalid : a.bridgeHint,
				tone: invalid ? 'error' : 'default'
			},
			token: {
				id: 'agent-token',
				label: a.token,
				value: agentSettings.token,
				hint: a.tokenHint,
				secret: true
			},
			status: agentStatus,
			useHere: agentBridge?.status === 'elsewhere' ? a.useHere : undefined,
			endpoint:
				endpoint === null
					? undefined
					: { label: a.endpoint, value: endpoint, hint: a.endpointHint },
			lastAction: last === null ? undefined : { label: a.lastAction, value: last },
			link: {
				connectedAs:
					agentSettings.linked === null
						? undefined
						: fill(a.connectedAs, { login: agentSettings.linked.login }),
				clientId:
					agentSettings.linked === null
						? undefined
						: fill(a.clientId, { id: agentSettings.linked.clientId }),
				connect: a.connect,
				connectHint: a.connectHint,
				busy: agentLink.phase === 'idle' ? undefined : a.connecting,
				problem: agentLink.problem === null ? undefined : linkProblem(agentLink.problem),
				agent:
					agentSettings.linked === null
						? undefined
						: {
								label: a.connectAgent,
								hint: a.connectAgentHint,
								href: `${agentSettings.linked.site}/account`
							},
				agentAddress:
					agentSettings.linked === null || agentSettings.agentUrl === ''
						? undefined
						: {
								label: a.agentAddress,
								hint: a.agentAddressHint,
								value: agentSettings.agentUrl,
								show: a.show,
								hide: a.hide,
								copy: a.copy,
								copied: a.copied,
								share: a.share
							},
				reconnect: a.reconnect,
				disconnect: a.disconnect,
				ownBridge: a.ownBridge,
				// Open while one is configured — an address alone counts (set-10).
				ownOpen: agentSettings.token.trim() !== '' || agentSettings.url.trim() !== ''
			},
			wallets: {
				title: a.wallets,
				hint: a.walletsHint,
				// The red "turn one on" only once there is an agent to turn one on
				// for: never on a panel the person has not set up.
				none:
					agentSettings.wallets.length === 0 && agentSetUp(agentSettings)
						? a.walletsNone
						: undefined,
				// Every account here, multisigs among them.
				rows: [
					...session.view.accounts.map((row) => ({
						address: row.account.address,
						name: row.account.name
					})),
					...multisig.accounts.map((a) => ({
						address: a.address,
						name: multisigName(a, data.multisigMessages)
					}))
				].map((row) => ({
					id: `agent-wallet-${row.address}`,
					title: row.name,
					subtitle: `${row.address.slice(0, 6)}…${row.address.slice(-4)}`,
					on: agentSettings.wallets.some((w) => w.toLowerCase() === row.address.toLowerCase())
				}))
			}
		};
	});

	/** Why connecting with clientmcp.dev did not work, in the person's words. */
	function linkProblem(problem: LinkProblem): string {
		const a = m.agent;
		switch (problem.kind) {
			case 'refused':
				return fill(a.linkRefused, { reason: problem.detail });
			case 'unavailable':
				return a.linkUnavailable;
			case 'expired':
				return a.linkExpired;
			case 'not_ours':
				return a.linkNotOurs;
			case 'network':
				return a.linkNetwork;
			default:
				return a.linkFailed;
		}
	}

	/** Its row, in 高级 after the service endpoints — the other "which server" row. */
	const agentRow = $derived<SettingsRowModel>({
		id: 'agent',
		icon: 'link-2',
		title: m.agent.title,
		subtitle: m.agent.subtitle,
		value: agentSettings.enabled ? agentStatus.label : m.agent.statusOff,
		trailing: 'chevron'
	});

	/**
	 * dapp-11: Settings › Connected sites — every site holding a grant, each
	 * with Disconnect, beside the AI agent row (the other "who may use this
	 * wallet" row). The extension only: nowhere else holds a grant. The
	 * device-storage page keeps listing them under its connections group.
	 */
	const sitesShown =
		typeof location !== 'undefined' && location.protocol === __VELA_EXTENSION_PROTOCOL__;
	const sitesRow = $derived<SettingsRowModel>({
		id: 'sites',
		icon: 'globe',
		title: m.sites.title,
		subtitle: m.sites.subtitle,
		value: String(grants.length),
		trailing: 'chevron'
	});
	const sitesPanel = $derived<SitesPanelModel | undefined>(
		sitesShown
			? {
					title: m.sites.title,
					subtitle: m.sites.subtitle,
					empty: m.sites.empty,
					group: {
						label: m.sites.listLabel,
						items: grants.map((grant) => ({
							id: grant.origin,
							label: hostLabel(grant.origin),
							meta: shortenAddress(grant.address),
							action: m.storage.disconnectOne,
							destructive: true
						}))
					}
				}
			: undefined
	);

	function withAgentRow(rows: SettingsRowModel[]): SettingsRowModel[] {
		const at = rows.findIndex((row) => row.id === 'endpoints');
		const added = sitesShown ? [agentRow, sitesRow] : [agentRow];
		return at < 0 ? rows : [...rows.slice(0, at + 1), ...added, ...rows.slice(at + 1)];
	}

	/**
	 * Spec 102: where the active account reviews and signs, from the SESSION's
	 * record (the core's reader migrated it), never the raw store.
	 *
	 * A real choice on the web and in the extension (owner, 2026-10-30: the
	 * page, everywhere): every transaction, batch, message and multisig
	 * approval of the account goes where it says (`signingRoute`), so the rows
	 * are an app's (R1 alone decides which can be picked), and the screen says
	 * what a page protects against and what it does not
	 * (`settings.venue.pageProtects` / `pageLimit`). The web saves no signing
	 * pages of its own: the official page, and the account's own.
	 */
	const activeAccount = $derived(view.accounts[view.active_index]?.account);
	const liveVenue = $derived.by(() => {
		if (!activeAccount) return undefined;
		const domain = activeAccount.signing_domain;
		return venueModel(
			{
				domain,
				choices: signingVenueChoices(domain, activeAccount.signing_venue, []),
				honest: true
			},
			m
		);
	});
	/** P2-10: the signing domain, for an account whose keys Vela's sheet cannot reach (R1). */
	const customDomain = $derived.by(() => {
		if (!activeAccount) return undefined;
		const domain = activeAccount.signing_domain;
		return signingVenueBlock(domain, { type: 'in_vela' }) === null ? undefined : domain;
	});

	/**
	 * set-5: the chain this page just added, so the emptied wizard can say
	 * what was added and to which space. Cleared by the next search, or by
	 * opening the wizard again.
	 */
	let justAdded = $state<number | null>(null);

	/** Owner decision 2026-10-28: the pool's word on the person's own nodes. */
	const ownNodeLists = $derived<LiveOwnNodes>({
		fallback: ownNodes.fallback,
		unreached: ownNodes.unreached
	});

	/** Spec 105: the space in front — the network pages show only its networks. */
	const liveSpace = $derived<LiveSpace | undefined>(
		networkSpace.words === null
			? undefined
			: { current: networkSpace.current, words: networkSpace.words }
	);

	/** Fixture base → identity overlay → live network sections (research D7). */
	const liveHome = $derived.by(() => {
		if (identity === null) return data.home;
		let model = withLiveAccounts(homeWithIdentity(data.home, identity), accountsInput, m);
		model = withLiveNetworks(
			model,
			net,
			m,
			selectedNetworkId,
			liveSpace,
			justAdded ?? undefined,
			ownNodeLists
		);
		if (storageReport !== null) model = withLiveStorage(model, storageReport, m);
		model = withLiveCurrency(model, currency.view, currencyCatalog);
		model = withLiveFeeSpeed(model, feeTierPreference.view);
		// After the storage numbers: the connections row is the grants', not a key count.
		model = withLiveConnections(model, grants, m);
		model = withLivePreferences(model, m, languageValue, data.locale, systemLocale);
		// ob-2: the wallets an erase would take before they sync, by name.
		model = withEraseUnsynced(model, m, view.unsynced, data.locale);
		model = withEraseFailure(model, m, eraseFailed);
		model = withFeedback(model, m, deviceFacts);
		model = {
			...model,
			keys: walletKeysModel(walletKeys, backupRow, m, customDomain),
			// Where an account signs is a passkey account's choice; a multisig's
			// members sign where each of them does.
			venue: shownMultisig ? undefined : liveVenue,
			// Spec 102: no Signing pages on the web — it opens none, so a list of
			// pages it trusts would be a list of nothing it does.
			signingPages: undefined
		};
		// The phone's first block (founder, 2026-09-05): 通讯录 is a tab on the
		// bar under this very screen, so the block it made up goes with it. The
		// report (spec 081 FR-016) is in the LAST block, after 关于 — "what is
		// this" and "something is wrong" are one errand — and Community sits
		// right above that block; the fixture already orders them so, the same
		// on all four shells (2026-09-27).
		return {
			...model,
			sections: model.sections
				.map((section) => ({
					...section,
					rows: withAgentRow(
						section.rows.filter((row) => row.id !== 'contacts' && row.id !== 'signing-pages')
					)
				}))
				.filter((section) => section.rows.length > 0 || section.appearanceControls === true)
		};
	});
	const liveDesktop = $derived.by(() => {
		if (identity === null) return data.desktop;
		let model = withLiveAccountsDesktop(
			desktopWithIdentity(data.desktop, identity),
			accountsInput,
			m
		);
		model = withLiveNetworksDesktop(
			model,
			net,
			m,
			selectedNetworkId,
			liveSpace,
			justAdded ?? undefined,
			ownNodeLists
		);
		if (storageReport !== null) model = withLiveStorage(model, storageReport, m);
		// After the storage numbers, as on the phone: the connections row is the
		// grants', not the drawn "4 sites" (spec 028 Phase 9, T485).
		model = withLiveConnections(model, grants, m);
		model = withLiveCurrencyDesktop(model, currency.view, currencyCatalog);
		// The desktop page reuses the phone sheet's rows — one list of tiers,
		// one set of words, whichever layout is showing.
		model = withLiveFeeSpeedDesktop(model, feeTierPreference.view, liveHome.feeSpeedSheet);
		model = {
			...model,
			account: {
				...model.account,
				keys: walletKeysModel(walletKeys, backupRow, m, customDomain),
				// A multisig signs no messages: no venue row for it, as on the phone.
				venue: shownMultisig ? undefined : liveVenue
			},
			// Spec 102: the web opens no signing page — see the phone model above.
			signingPages: undefined,
			nav: model.nav.flatMap((item) => {
				if (item.id === 'signing-pages') return [];
				if (item.id !== 'endpoints') return [item];
				const agentItem = { id: 'agent' as const, icon: 'link-2' as const, label: m.agent.title };
				const sitesItem = { id: 'sites' as const, icon: 'globe' as const, label: m.sites.title };
				// Usage statistics, beside the other "who sees this wallet" entries.
				const privacyItem = {
					id: 'privacy' as const,
					icon: 'eye-off' as const,
					label: m.sections.privacy
				};
				return sitesShown
					? [item, agentItem, sitesItem, privacyItem]
					: [item, agentItem, privacyItem];
			})
		};
		model = withEraseUnsynced(model, m, view.unsynced, data.locale);
		model = withEraseFailure(model, m, eraseFailed);
		// Spec 081: the wide layout's own two dead controls — the danger card
		// with no handler, and a report panel that did not exist.
		model = withEraseFailureDesktop(model, m, eraseFailed);
		model = withFeedbackDesktop(model, m, deviceFacts);
		return withLivePreferencesDesktop(model, m, languageValue, data.locale, systemLocale);
	});

	/**
	 * Usage statistics for the network editor, counted when the core says the
	 * change HELD: an add when the ledger names the chain it added, an RPC
	 * when the chain-id check let it be written (never on a refusal).
	 */
	let addingChain: number | null = null;
	let rpcEdited: number | null = null;
	let rpcSaving = $state<number | null>(null);
	$effect(() => {
		const landed = net.last_added_chain_id;
		if (addingChain === null || landed !== addingChain) return;
		addingChain = null;
		justAdded = landed;
		track('network_added', { chain: landed });
	});
	$effect(() => {
		const chain = rpcSaving;
		if (chain === null) return;
		const row = net.networks.find((candidate) => candidate.chain_id === chain);
		if (row?.rpc_save_deferred === true) return;
		rpcSaving = null;
		if (row !== undefined && row.rpc_chain_mismatch === null) track('network_rpc_set', { chain });
	});

	/**
	 * Check the wizard's candidate again, with whatever RPC is typed — the
	 * panel's "Re-check with this RPC", and its "Retry" over a check that
	 * could not be made.
	 */
	function recheckCandidate(): void {
		const chainId = net.wizard.chain_info?.chain_id;
		if (chainId === undefined) return;
		networkAdmin.dispatch({ type: 'chain_selected', chain_id: chainId, keep_custom_rpc: true });
	}

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
			case 'delete-network': {
				const chain = chainOf(event.id);
				// Through the intents (spec 103), by the chain the row is.
				if (chain !== undefined) {
					void intents.invoke('person', { type: 'remove_network', chain_id: chain });
					track('network_removed', { chain });
				} else networkAdmin.dispatch({ type: 'delete_confirmed', id: event.id });
				return;
			}
			case 'move-network': {
				// Spec 105: a network the person added, to the other space.
				const row = net.networks.find((r) => r.id === event.id);
				if (row !== undefined && row.is_custom)
					networkAdmin.dispatch({
						type: 'custom_space_changed',
						id: row.id,
						testnet: !row.is_testnet
					});
				return;
			}
			case 'switch-space':
				networkSpace.openSwitcher();
				return;
			case 'add-space':
				networkAdmin.dispatch({ type: 'wizard_testnet_chosen', testnet: event.testnet });
				return;
			case 'detail-field': {
				const chainId = selectedNetworkId === undefined ? undefined : chainOf(selectedNetworkId);
				if (chainId !== undefined) {
					networkAdmin.dispatch({
						type: 'override_field_edited',
						chain_id: chainId,
						field: event.field,
						value: event.value
					});
					if (event.field === 'rpc') rpcEdited = chainId;
				}
				return;
			}
			case 'only-mine': {
				const chainId = selectedNetworkId === undefined ? undefined : chainOf(selectedNetworkId);
				if (chainId !== undefined) {
					networkAdmin.dispatch({ type: 'only_mine_changed', chain_id: chainId, on: event.on });
				}
				return;
			}
			case 'detail-blur': {
				const chainId = selectedNetworkId === undefined ? undefined : chainOf(selectedNetworkId);
				if (chainId !== undefined) {
					networkAdmin.dispatch({ type: 'override_blurred', chain_id: chainId });
					if (event.field === 'rpc' && rpcEdited === chainId) rpcSaving = chainId;
					rpcEdited = null;
				}
				return;
			}
			case 'open-add':
				justAdded = null;
				// set-5: what the person entered goes only when they say so
				// (`discard-add`) — left another way, it is there on return.
				if (!addNetworkEntered(net.wizard)) networkAdmin.dispatch({ type: 'wizard_reset' });
				return;
			case 'discard-add':
				justAdded = null;
				networkAdmin.dispatch({ type: 'wizard_reset' });
				return;
			case 'search':
				justAdded = null;
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
				// The panel's primary is "Add network" only when the core says the
				// candidate may be added (`can_add`). Over an inconclusive check it
				// reads "Retry" — and it did nothing: `add_confirmed` is dropped
				// unless the check passed. Retry is the check, made again.
				if (!net.wizard.can_add) {
					recheckCandidate();
					return;
				}
				addingChain = net.wizard.chain_info?.chain_id ?? null;
				// Through the intents (spec 103): the wizard checked this chain
				// already, so only the add is sent, as the button always did.
				if (addingChain !== null)
					void intents.invoke('person', {
						type: 'add_network',
						chain_id: addingChain,
						rpc_url: null
					});
				return;
			case 'recheck':
				recheckCandidate();
				return;
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
				void intents.invoke('person', { type: 'reset_service_endpoints' });
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
		void intents.invoke('person', { type: 'sign_out' });
	}
</script>

<svelte:window ononline={rereadKeys} onfocus={rereadKeys} />
<svelte:document onvisibilitychange={rereadKeys} />

<svelte:head>
	<title>{data.settingsMessages.title}</title>
	<meta name="robots" content="noindex" />
</svelte:head>

{#if identity}
	{#if wide.current}
		<div class="desktop-shell">
			<SettingsDesktop
				bind:this={desktopSettings}
				model={liveDesktop}
				{sidebar}
				onnav={selectTab}
				onsignout={signOut}
				onnetevent={onNetEvent}
				onprefevent={onPrefEvent}
				analytics={analyticsRow}
				agent={agentPanel}
				sites={sitesPanel}
				onaccountselect={selectAccount}
				onaccountcreate={() => void goto(createHref)}
				onaccountsignin={() => void goto(welcome)}
				onaccountremove={removeAccount}
				onaccountsopen={accountsOpen}
				onstorageclear={clearRow}
				onclearcaches={clearCaches}
				onfeedbacksend={(report) => void sendFeedback(report)}
				feedbackSending={reportSend.sending}
				feedbackResult={reportSend.result}
				feedbackDraft={reportSend.draft}
				onfeedbackopen={(open) => reportSend.surface(open)}
				onfeedbackdone={() => reportSend.done()}
				onethereumbackup={startBackup}
				accountMore={multisigRows}
				accountExtra={multisigAdd}
				accountKeys={shownMultisig ? multisigKeys : undefined}
			/>
		</div>
	{:else}
		<main class="page">
			<SettingsHome
				bind:this={phoneSettings}
				model={liveHome}
				destinations={WEB_DESTINATIONS}
				onselecttab={selectTab}
				onsignout={signOut}
				onnetevent={onNetEvent}
				oncurrencyselect={chooseCurrency}
				onstorageclear={clearRow}
				onprefevent={onPrefEvent}
				analytics={analyticsRow}
				agent={agentPanel}
				sites={sitesPanel}
				onaccountselect={selectAccount}
				onaccountcreate={() => void goto(createHref)}
				onaccountsignin={() => void goto(welcome)}
				onaccountremove={removeAccount}
				onaccountsopen={accountsOpen}
				onclearcaches={clearCaches}
				onfeedbacksend={(report) => void sendFeedback(report)}
				feedbackSending={reportSend.sending}
				feedbackResult={reportSend.result}
				feedbackDraft={reportSend.draft}
				onfeedbackopen={(open) => reportSend.surface(open)}
				onfeedbackdone={() => reportSend.done()}
				onethereumbackup={startBackup}
				accountMore={multisigRows}
				accountKeys={shownMultisig ? multisigKeys : undefined}
			/>
		</main>
	{/if}
	<MultisigHost
		m={data.multisigMessages}
		signing={data.signingMessages}
		locale={data.locale}
		wide={wide.current}
		closeLabel={liveHome.closeLabel}
		current={view.address || null}
	/>
{:else}
	<!-- The core has not ruled yet. The shape of a screen, not a fixture account. -->
	<CoreWaiting rows={5} />
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

{#snippet multisigRows()}
	<MultisigRows
		m={data.multisigMessages}
		removeLabel={data.multisigMessages.settings.remove}
		removeNamed={liveHome.accountsSheet.removeNamed}
		cancelLabel={liveHome.accountsSheet.removeCancel ?? ''}
		amount={(address) => accountAmount(accountsInput, address)}
		loading={(address) => accountLoading(accountsInput, address)}
		onselect={(account) => multisig.activate(account.address)}
	/>
{/snippet}

{#snippet multisigAdd()}
	<!-- r4D-19: the two ways to a multisig, here too (the switcher had them alone). -->
	<div class="multisig-add">
		<Button variant="secondary" shape="rounded" onclick={() => multisigUi.open({ kind: 'create' })}
			>{data.multisigMessages.switcher.create}</Button
		>
		<Button variant="secondary" shape="rounded" onclick={() => multisigUi.open({ kind: 'add' })}
			>{data.multisigMessages.switcher.add}</Button
		>
	</div>
{/snippet}

{#snippet multisigKeys()}
	{#if shownMultisig}
		<MultisigSettings
			m={data.multisigMessages}
			account={shownMultisig}
			payer={view.address || null}
			cancelLabel={liveHome.accountsSheet.removeCancel ?? ''}
			removeNamed={liveHome.accountsSheet.removeNamed}
		/>
	{/if}
{/snippet}

<style>
	.multisig-add {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-sm);
		margin-block-start: var(--space-sm);
	}

	.multisig-add > :global(*) {
		flex: 1;
	}

	/* The phone screens are `height: 100%` of whatever holds them, and the
	   root layout only sets a MIN-height — so without a frame the screen
	   stood as tall as its own rows, the DOCUMENT scrolled, and the tab bar
	   scrolled away with it (founder, 2026-09-05). The contacts route frames
	   its phone the same way. */
	.page {
		/* home-10: less the testnet bar, when it shows. */
		height: calc(100dvh - var(--space-bar, 0dvh));
		display: flex;
		flex-direction: column;
		background: var(--color-bg-base);
	}

	/* The three columns are `height: 100%` of whatever holds them, and nothing
	   above this page has a height — the body has a MIN-height — so without a
	   frame they stood as tall as their own rows and the sunken sidebars ended
	   mid-screen. The wallet route frames its desktop the same way. */
	.desktop-shell {
		/* home-10: less the testnet bar, when it shows. */
		height: calc(100dvh - var(--space-bar, 0dvh));
		overflow: hidden;
	}
</style>
