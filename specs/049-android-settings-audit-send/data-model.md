# Data Model — 049

## Preferences (unchanged storage)

| Key | Shape | Readers after 049 |
|---|---|---|
| `vela.avatarStyle` | `initials` \| `identicon` | `LocalAvatarStyle` → `IdenticonImage` (every avatar), the viewer |
| `vela.localePrefs.numberFormat` | wire key | `Formats.current` (Compose state) → hero mark+grouping, fiat, token amounts, sheet examples, clear-signing locale |
| `vela.localePrefs.dateFormat` | wire key | day headers, detail facts, sheet examples, clear-signing locale |
| `vela.localePrefs.timeFormat` | wire key | detail facts, sheet examples, clear-signing locale |

## View-model additions

- `BalanceModel.decimalMark: String = "."` — the mark between integer and
  decimals; `WalletLive.home` sets it from the preset.
- `RecipientFieldModel.name: String?` — the identity's name for the initials
  disc (the `note` slot stays the sweep's note).
- `FactLead.Identicon(seed, name: String? = null)`.
- `SelectRowModel.id` for the three format sheets = the wire key (was the
  row index).

## Formats

- `Formats.current`: `MutableState<Formats>` in the companion — set by
  `Preferences.publish` BEFORE the view is published; read in composition it
  subscribes the reader.
- `decimalMark()`: the resolved preset's decimal separator.
- `plain(text)`: a plain decimal string with `.` swapped for the mark (no
  grouping — the web's `trimBalance`).
- `example()`: `number(1234567.89, 2, 2)`; `dateExample()/timeExample()` on
  2026-06-13 13:45 local (the web's `FORMAT_SAMPLE`).

## Clear-signing locale

`ClearLocale.fromFormats(f)`: number/date/time from `f.resolved*()`,
`tz_offset_minutes = TimeZone.getDefault().getOffset(now) / 60000`.
