/**
 * Wallet-home message manifest (spec 015, research.md D3).
 *
 * Every string the wallet screens render, keyed into the vela-core corpus.
 * Client-safe: names keys and shapes only — resolution happens in
 * `engine.server.ts` at build time, exactly like the Welcome page.
 * Templated values (`{{name}}`, `{{count}}`…) are resolved server-side too:
 * fixtures are static, so every interpolation is known at prerender time.
 */
import { INTENT_TERMS } from '$lib/signing/terms';

export interface WalletMessages {
	nav: { wallet: string; contacts: string; explore: string; settings: string };
	balance: {
		totalLabel: string;
		liveIndicator: string;
		/**
		 * The hero's quiet line by the key the core names for it
		 * (`BalanceView.checking_key`, `BalanceView.live_key` —
		 * {@link BALANCE_SAID_KEYS}): "Checking…" while the first read is out,
		 * "Live · listening for payments" under a zero every chain answered for.
		 */
		said: Readonly<Record<string, string>>;
		stale: string;
		unpriced: string;
		/** home-11: in the figure's place when nothing held can be priced — "No price yet". */
		unpricedTotal: string;
		/** home-11: the line under it — "None of your tokens could be priced." */
		allUnpriced: string;
		/**
		 * home-13: the chosen currency could not be priced, so the figures are in
		 * US dollars — said, not silently swapped (`{{code}}`).
		 */
		currencyUnavailable: string;
		/**
		 * Owner decision 2026-10-28: the person's node for `{{name}}` is not
		 * answering and public nodes serve it for now — once a session — and the
		 * way to its settings.
		 */
		nodeFallback: string;
		nodeFallbackOpen: string;
		/**
		 * ob-2: under the name of a wallet saved here whose registration has
		 * not landed — "syncing…", then, once it has failed a while, "not
		 * synced yet" with a retry.
		 */
		syncing: string;
		syncStalled: string;
		syncRetry: string;
		/** Spec 038: nothing could be read and nothing is known. */
		unreachable: string;
		noPrice: string;
		a11yHide: string;
		a11yShow: string;
		/** Issue 462: "Updated {{ago}}" under the hero. */
		lastUpdated: string;
		/** Issue 462: what that control says while a read the person asked for is out. */
		updating: string;
		/**
		 * Issue 462: the control's name before any read has settled, when it
		 * draws the glyph alone — "Refresh balance", never "Updating…" for a
		 * control at rest.
		 */
		refreshBalance: string;
		/**
		 * The core's compact relative-time words — "now", "{{n}}m", "{{n}}h",
		 * `{{n}}` unfilled — which the page hands to the core's
		 * `formatRelativeTime` with this device's clock.
		 */
		ago: { now: string; minutes: string; hours: string };
	};
	actions: { receive: string; send: string; scan: string };
	sections: { activity: string; assets: string; all: string; add: string };
	activity: {
		sent: string;
		received: string;
		dapp: string;
		/**
		 * A dApp's transaction nobody decoded (083 H2) — the signing sheet's
		 * own "Contract interaction".
		 */
		contractCall: string;
		/**
		 * A dApp row's headline verb in the reader's words, keyed by
		 * `ClearTerm` (`INTENT_TERMS`) — the recorded intent's word, or the
		 * wallet's own for what the request is (spec 093).
		 */
		intents: Record<string, string>;
		/** Spec 093: "{{intent}} on {{place}}" — a dApp row's title. */
		dappRowTitle: string;
		/** Spec 093: an allowance with no limit, where a figure would be. */
		unlimited: string;
		today: string;
		yesterday: string;
		/** Template with `{{name}}`. */
		toName: string;
		/** Template with `{{name}}`. */
		fromName: string;
		emptyTitle: string;
		emptyCaption: string;
		/**
		 * Spec 082 RG2: a row the tracker has not closed says so first —
		 * "Pending · <site>" / "Failed · <site>".
		 */
		pending: string;
		failed: string;
		/**
		 * send-6: a send that failed moved nothing — "Not sent" is its title,
		 * and its figure carries no "−".
		 */
		notSent: string;
		/** send-20: a sweep's row figure, "{{n}} assets" — a count with its unit. */
		assetsCount: string;
		/**
		 * 087 F04: a pending record nothing will settle (`FeedTxStatus`
		 * `unknown`) — "Unknown · <site>", never "Failed": it may have been sent.
		 */
		unknown: string;
		/** Spec 082 RG5: the home's empty line under a chain filter (`home_empty_key`). */
		emptyTitleNetwork: string;
		/**
		 * Round 16 leftover 5: the networks the scan could not read
		 * (`FeedView.unread_key`) — "Activity can't be read on {{network}}
		 * right now." / "… on {{n}} networks …". Absent (a drawn fixture):
		 * nothing said.
		 */
		unreadOne?: string;
		unreadMany?: string;
		/**
		 * Round 5: a transfer of a token nobody vouches for says so on its row
		 * — "Unverified token", or "Not the real {{symbol}} — …" — the
		 * multisig's words (`home.multisig.token`), one set for every surface.
		 * Absent (a drawn fixture): no note.
		 */
		tokenUnverified?: string;
		tokenLookalike?: string;
	};
	assets: {
		emptyTitle: string;
		emptyCaption: string;
		/**
		 * Spec 092: the home line over networks the wallet cannot reach — the
		 * core names which (`unreachable_key`). `{{name}}` for one network …
		 */
		unreachableOne: string;
		/** … `{{n}}` for several. */
		unreachableMany: string;
		/**
		 * PR 3 note 4: the one network's RPC is fine — its token list could not
		 * be loaded (`{{name}}`, as for the one network).
		 */
		tokenListUnreachable: string;
		/**
		 * Owner decision 2026-10-28: the one network is read through the
		 * person's own node alone, and it did not answer (`{{name}}`).
		 */
		ownNodeUnreachable: string;
		/**
		 * "Your assets there aren't affected — we just can't read them right
		 * now." Under the unreachable line where the Assets list has no rows
		 * to show (home-3).
		 */
		unreachableBody: string;
		/** "Last seen {{amount}}" — a row whose network did not answer. */
		lastSeen: string;
		/**
		 * PR 2 note 11 (issue 483): the line when the last read failed inside
		 * Vela itself, by the key the core names (`BalanceView.internal_key`) —
		 * drawn where the unreachable line goes, never "Can't reach Ethereum".
		 */
		internal: Readonly<Record<string, string>>;
	};
	networkFilter: {
		pillAll: string;
		sheetTitle: string;
		allNetworks: string;
		/** The phone home's line while it is narrowed to one network: "Showing {{network}} only". */
		only: string;
		/** …and its way back to every network. */
		showEvery: string;
	};
	sidebar: { networks: string };
	receive: {
		title: string;
		addressLabel: string;
		copyAddress: string;
		qrCaption: string;
		warningTitle: string;
		warningReminder: string;
		/** Template with `{{count}}`. */
		networksLine: string;
		/** Template with `{{name}}` and `{{id}}`. */
		networkDetail: string;
	};
	assetDetail: {
		send: string;
		receive: string;
		labelName: string;
		labelPrice: string;
		/** Template with `{{symbol}}` and `{{value}}`. */
		priceValue: string;
		labelContract: string;
		labelDecimals: string;
		labelTransactions: string;
		viewOnExplorer: string;
		nativeToken: string;
	};
	/**
	 * The way back out of a signed-in wallet (spec 019).
	 *
	 * `settings.signOut.desc` is deliberately absent, as on all three native
	 * clients: it ends "your passkey stays in Face ID / fingerprint", and
	 * `keeps` says the load-bearing part without naming one platform's
	 * biometric to people on another.
	 */
	signOut: {
		title: string;
		/** Template with `{{count}}` — what a sign-out takes when it is more
		 *  than one wallet (2026-09-23). */
		descMany: string;
		keeps: string;
		warning: string;
		button: string;
		anyway: string;
		cancel: string;
	};
	/**
	 * The identicon, big, with the address that drew it (spec 019 founder
	 * call): the picture is a fingerprint of the address, and the two are only
	 * useful together — seeing them side by side is what builds the habit of
	 * recognising an account by its artwork.
	 */
	identiconViewer: {
		title: string;
		caption: string;
		copyAddress: string;
		copied: string;
		close: string;
		a11yOpen: string;
	};
	close: string;
}

/**
 * Every line the core can name for a balance read that failed inside Vela
 * (`BalanceView.internal_key`, PR 2 note 11): the fee's own sentence for the
 * same fault (`fee_policy::REASON_INTERNAL_KEY`).
 */
export const BALANCE_INTERNAL_KEYS = ['componentsUi.gas.reasonInternal'] as const;

/**
 * Every quiet line the core can name for the hero's status slot
 * (`balance_dashboard`: `CHECKING` — `BalanceView.checking_key`, `LIVE_ZERO` —
 * `BalanceView.live_key`). `live.test.ts` holds the list to the core's source.
 */
export const BALANCE_SAID_KEYS = ['componentsUi.funding.checking', 'home.liveIndicator'] as const;

/** Every corpus key the wallet screens consume (tests iterate this). */
export const WALLET_KEYS = [
	'componentsUi.mainNav.wallet',
	'componentsUi.mainNav.contacts',
	'componentsUi.mainNav.explore',
	'componentsUi.mainNav.settings',
	'home.totalBalance',
	'home.liveIndicator',
	'componentsUi.funding.checking',
	'home.balanceStale',
	'home.balanceUnpriced',
	// home-11 / home-13 / the person's own node (2026-10-28).
	'home.totalUnpriced',
	'home.allUnpriced',
	'home.currencyUnavailable',
	'home.nodeFallbackNotice',
	'home.nodeFallbackOpen',
	'home.syncingLine',
	'home.syncStalledLine',
	'home.balanceDetailRetry',
	'home.balanceDetailNoPrice',
	'home.a11yHideBalance',
	'home.a11yShowBalance',
	// Issue 462: the hero's refresh control, and the age it reads.
	'home.lastUpdated',
	'home.updating',
	'home.refreshBalance',
	'time.now',
	'time.minutesShort',
	'time.hoursShort',
	'componentsUi.dock.receive',
	'componentsUi.dock.send',
	'componentsUi.dock.scan',
	'home.tabActivity',
	'assets.sectionTitle',
	'history.filterAll',
	'assets.addToken',
	'history.labelSent',
	'history.labelReceived',
	'history.txLabelDappTx',
	// 083 H2, spec 093: a dApp row is titled by what it did, and where
	// (`intentContractCall` and the signatures' verbs are among the terms).
	...INTENT_TERMS.map((term) => `componentsUi.signing.${term}` as const),
	'history.dappRowTitle',
	'componentsUi.signingApprove.unlimitedValue',
	'componentsUi.dayGroup.today',
	'componentsUi.dayGroup.yesterday',
	'history.toName',
	'history.fromName',
	'home.emptyNoActivity',
	'home.emptyNoActivityNetwork',
	// Round 16 leftover 5: Activity names the networks its scan could not read.
	'home.activityUnreadOne',
	'home.activityUnreadMany',
	'home.emptySubtitle',
	'componentsTx.detail.statusPending',
	'componentsTx.detail.statusFailed',
	// 087 F04: a pending record nothing will settle.
	'componentsUi.signing.intentUnknown',
	'assets.emptyTitle',
	'assets.emptySubtext',
	'assets.unreachableOne',
	'assets.unreachableMany',
	// PR 3 note 4: a token list that can't be loaded is not a network out of reach.
	'assets.tokenListUnreachable',
	'assets.ownNodeUnreachable',
	// home-3: the list that could not be read, and a row last read elsewhere.
	'assets.unreachableBody',
	'assets.lastSeen',
	// PR 2 note 11: a read that failed inside Vela, said as that.
	...BALANCE_INTERNAL_KEYS,
	// Spec 038: the home's sentence for a first launch with no network.
	'onboarding.common.networkBody',
	'componentsUi.networkFilter.pillAll',
	'componentsUi.networkFilter.selectChain',
	'componentsUi.networkFilter.allNetworks',
	'home.networkFilterOnly',
	'componentsUi.networkFilter.showEvery',
	'settingsModals.network.modalTitle',
	'receive.title',
	'receive.addressLabel',
	'componentsUi.identiconViewer.copyAddress',
	'componentsUi.qrPlaceholder.caption',
	'receive.warningTitle',
	'receive.warningReminder',
	'receive.networksLine',
	'receive.networkDetail',
	'tokenDetail.send',
	'tokenDetail.receive',
	'tokenDetail.labelName',
	'tokenDetail.labelPrice',
	'tokenDetail.priceValue',
	'tokenDetail.labelContract',
	'tokenDetail.labelDecimals',
	'tokenDetail.labelTransactions',
	'tokenDetail.viewOnExplorer',
	'settings.signOut.title',
	'settings.signOut.keeps',
	'settings.signOut.warning',
	'settings.signOut.button',
	'settings.signOut.anyway',
	'settings.signOut.cancel',
	'componentsUi.identiconViewer.title',
	'componentsUi.identiconViewer.caption',
	'componentsUi.identiconViewer.copied',
	'componentsUi.identiconViewer.a11yOpen',
	'addToken.labelNativeToken',
	'componentsUi.identiconViewer.close'
] as const;

/**
 * `{{var}}` interpolation for the handful of templated wallet strings.
 * Build-time only, over corpus-linted templates with known vars — not a
 * parallel i18n engine (spec 015 research.md D3).
 */
export function fill(template: string, vars: Record<string, string | number>): string {
	return template.replace(/\{\{\s*(\w+)\s*\}\}/g, (match, name: string) =>
		name in vars ? String(vars[name]) : match
	);
}
