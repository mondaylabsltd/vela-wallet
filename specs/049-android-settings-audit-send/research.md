# Research — 049 Android Settings Audit and the Send Path

## R1. Why the avatar style does nothing

- `Preferences.setAvatarStyle` writes `vela.avatarStyle`; `SettingsLive.withPreferences`
  ticks the segment from it. No other reader exists: `IdenticonAvatar.kt`
  says "No initial-letter rendering anywhere (FR-006)" — a spec-015 ruling that
  predates the preference (028 T431/T432 on the web).
- Web: `wallet/identicon.ts::avatarSvgForClient(seed, name)` branches on
  `preferences.avatarStyle`; `initialsSvg` = circle `--color-accent-soft`,
  letter `--color-accent-base`, bold, font-size 34 in a 100 viewBox, `V`
  fallback. Every web site (settings, session switcher, wallet, contacts,
  signing) calls it, including the viewer subject.
- **Decision**: `IdenticonImage` reads a `LocalAvatarStyle` composition local
  (provided from `preferences.view` in `MainActivity`) and a new `name`
  parameter; `initials` draws `InitialsDisc`. The viewer host takes
  `(seed, name)`. Nineteen call sites pass the name they already show beside
  the artwork.

## R2. Why the number format looks wrong

- `Formats.current` is a `@Volatile var`; `Preferences.publish` sets it. The
  live builders read it, but: (a) `BalanceDisplay` draws `".$decimals"`
  literally — with `dot_comma` the hero reads `CN¥3` `.63` beside `1.234`-style
  grouping; (b) `WalletLive.trimAmount/signedAmount`, `SendLive.trim/fromBase`,
  `FlowLive.txDetail` print `toPlainString()` — the web's `trimBalance` swaps
  the decimal mark; (c) the flow models are `remember(...)`ed without a
  formats key, so a change under an open flow stays stale until a data key
  moves; (d) the sheets' labels are `NUMBER_SAMPLES` strings from the mocks,
  the web computes `numberFormatOptions()` live; (e) `SigningController.clearKickoff`
  passes `ClearLocale()` (comma_dot/iso/h24/tz 0) — the web does the same
  (`sheet.svelte.ts:98`), recorded as a web gap.
- **Decision**: `Formats.current` becomes Compose `mutableStateOf` (observed
  where read in composition; added to the two `remember` key lists); `Formats.decimalMark()`
  + `Formats.plain(String)` for token amounts; `BalanceModel.decimalMark`;
  live examples in `SettingsLive.withPreferences` with the wire key as the
  row id; `ClearLocale.fromFormats(Formats.current)` at kickoff.

## R3. `auto` resolution

- Web `detectNumber` reads `Intl` for the browser locale — not the app
  language. Android `Formats.resolvedNumber` reads `DecimalFormatSymbols` of
  `Locale.getDefault()`. Same rule; kept. The sheet's 自动 row shows the
  resolved rendering so the person can see what it means on this phone.

## R4. Transaction-detail fiat

- `FlowLive.txDetail` prints `"$" + usd_value`; the web prints
  `moneyText(item.usd_value, currency)` (converted + formatted). The NavHost's
  `liveFlow` already has `currency`; `WalletLive.Money.of(currency)` is the
  converter. **Decision**: pass it through.

## R5. The send on Gnosis

- Fixture Safe `0x88cCA0…6894` holds 0.542 xDAI (RPC read 2026-09-13). A
  transfer of 0.001 + in-band fee ~0.01 is affordable. Recipient: the phone's
  own account `0x7687…D141` (contact 觉得九点半). The 045 script
  (`p45_1.py`) drives pick → form → confirm → receipt; the confirm is a
  button, not a slider (048 gotchas).

## R6. What is not changed

- The web's clear-signing locale constant; the desktop; the gallery fixtures
  (`NUMBER_SAMPLES` stay for the drawn boards).
