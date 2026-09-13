# Research — 047
- D1 Preferences: shell-side, the web's keys byte-for-byte (`vela.language`
  bare tag or `system`; `vela.localePrefs` JSON `{numberFormat, dateFormat,
  timeFormat, textScale}`; `vela.avatarStyle`; `vela.theme` already).
- D2 Formats: port `locale-format.ts`'s presets (separators per key, date
  shapes, 12/24h); `auto` resolves from the app locale; applied in
  `WalletLive.Money`, the feed's day headers and detail dates, the send's
  figures (one `Formats` object, injected through `Money`).
- D3 Text scale: the six levels' factors multiply the theme's font scale
  (`LocalDensity` with `fontScale × factor`).
- D4 Storage: `KeyValueStore.allKeys()` (DataStore `asMap().keys`); the web's
  `itemOfKey` mapping ported (`vela.transactionHistory` → transactions,
  `vela.contacts*` → contacts, `vela.customTokens|customNetworks` → custom,
  `vela.browserHistory|explore` → browsing, `vela.balanceCache` → balances,
  `vela.fiatRates|fxRates` → rates, `vela.perm.*` → dapps); bytes = the
  value's UTF-8 length; records = the JSON array length where the value is one.
- D5 Erase: enumerate, remove all but the keep-list (account records —
  sign-out's own path removes those), re-enumerate; leftovers → the sheet's
  danger callout; success → `session.signOutConfirmed()` → first run.
- D6 Relayer: `RelayClient.probeTreasury(chainId)` for the current chain →
  address, floor, balance → the panel's hint and address; retry re-probes.
- D7 Feedback: `https://github.com/mondaylabsltd/vela-wallet/issues/new?template=bug.yml&title=…&body=…`
  with the preview lines; opened with ACTION_VIEW.
- D8 Deep links: `singleTop` + `onNewIntent`; `velawallet://pay` and
  `https://wallet.getvela.app/pay` → `PaymentRequest.LinkOpened{…}` on a
  fresh core → `view.pay` → `send.open(locked params)`; `velawallet://open?url=`
  → the browser.
- D9 Offline: `ConnectivityManager.registerDefaultNetworkCallback`; the home
  line uses `settingsModals.home.networkOffline`; reconnect → `wallet.refresh()`.
- D10 Panic: `Thread.setDefaultUncaughtExceptionHandler` writes
  `vela.crashReport` then delegates; next launch reads it, shows the failure
  sheet, clears on dismiss; core faults (`onFault`) write the same record.
- D11 Rulers: two node scripts over the repo (event variants per machine from
  the ts mirrors vs `XEvent.Variant` usages in Android main; view fields per
  machine vs reads in `*Live.kt`), tables into results.md.
