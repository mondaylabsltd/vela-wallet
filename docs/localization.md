# Localization (i18n / L10n) Architecture

How Vela Wallet localizes everything the user sees. Written as a **reference** —
reusable in other React Native / Expo projects, and a maintenance guide for ours.

Localization here is split into **four independent concerns**. They do not depend
on each other: a user can run the UI in English, group numbers `1.234.567,89`,
show dates as `dd.mm.yyyy`, and display balances in KRW — all at once.

| Concern | What it controls | Lives in | Stored at |
|---|---|---|---|
| **1. UI language** | Translated strings (buttons, labels, messages) | `src/i18n/` | `vela.language` |
| **2. Regional format** | Number / date / time formatting (the "区域格式" screen) | `src/services/locale-format.ts` | `vela.localePrefs` |
| **3. Display currency** | Which fiat balances are shown in + the rate | `src/services/currency.ts` (+ `fiat-*`) | `vela.currency` |
| **4. Relative time** | "4m" / "16分钟前" in the activity feed | `src/services/activity.ts` (uses #1) | — |

> Why split? Language ≠ format ≠ currency. A German speaker in Japan may want
> the UI in German, JPY balances, and `1,234.56` grouping. Keeping them
> independent is the whole point.

---

## Table of contents

- [1. UI language (react-i18next)](#1-ui-language-react-i18next)
  - [Stack & file layout](#stack--file-layout)
  - [Single `translation` namespace](#single-translation-namespace)
  - [Typed keys](#typed-keys)
  - [System-language detection](#system-language-detection)
  - [Preference & persistence](#preference--persistence)
  - [Instant switch (no restart)](#instant-switch-no-restart)
  - [Using translations](#using-translations)
  - [Conventions & pitfalls](#conventions--pitfalls)
  - [Add a string](#add-a-string) · [Add a language](#add-a-language)
- [2. Regional format (number / date / time)](#2-regional-format-number--date--time)
- [3. Display currency (fiat)](#3-display-currency-fiat)
- [4. Relative time](#4-relative-time)
- [Maintenance](#maintenance)
- [Reusing this in another project](#reusing-this-in-another-project)

---

## 1. UI language (react-i18next)

### Stack & file layout

`i18next` + `react-i18next` + `expo-localization`.

```
src/i18n/
  index.ts          # init, system detection, preference cache, load/save, native names
  language.tsx      # LanguageProvider + useLanguagePreference() hook
  resources.ts      # aggregates every locale file into the i18next `resources` map
  i18next.d.ts      # makes t() keys type-checked (derives from the English file)
  locales/
    en.json         # CORE namespaces: common / language / settings
    en/             # one file per screen/area:
      home.json     #   { "home": { … } }
      send.json     #   { "send": { … } }
      …             #   14 area files total
    zh.json  zh/…   # …one folder per language
    ja.json  ja/…
    …
```

- **13 languages**: `en`, `zh` (Simplified), `zh-TW`, `zh-HK`, `ja`, `ko`, `vi`,
  `id`, `tr`, `es-MX`, `pt-BR`, `fr`, `de`.
- **14 area namespaces** + core (`common`/`language`/`settings`): `home`, `send`,
  `receive`, `assets`, `addToken`, `tokenDetail`, `history`, `onboarding`,
  `connect`, `about`, `clearSigning`, `componentsTx`, `componentsUi`,
  `settingsModals`.
- **~600 keys per language** (kept at 100% parity — see [Maintenance](#maintenance) for the exact, always-current count).

### Single `translation` namespace

Every per-area file is merged into **one** i18next namespace (`translation`), so
callers just write `t('home.title')`. `resources.ts` does the merge:

```ts
// resources.ts (generated-style; add a language = add its imports + spread)
import enCore from './locales/en.json';
import enHome from './locales/en/home.json';
// …
export const en = { ...enCore, ...enHome, /* …all 14 files */ };   // also the TS key source
const zh = { ...zhCore, ...zhHome, /* … */ };
export const resources = {
  en: { translation: en },
  zh: { translation: zh },
  // …13 languages
};
```

**Why per-file, not one big JSON?** It lets many people (or many agents) edit
different screens without merge conflicts, while callers still get the ergonomic
flat `t('area.key')`.

> ⚠️ Because there is only one namespace, **never** call `useTranslation('home')`.
> Use `useTranslation()` and `t('home.x')`. (Namespace-scoped hooks look for a
> `home` namespace that doesn't exist → keys won't resolve.)

### Typed keys

`i18next.d.ts` augments i18next so `t()` keys are autocompleted and type-checked
against the **English** file. A typo (`t('home.titel')`) is a compile error; a
missing key fails `tsc`. This is also our cheapest correctness net — a clean
`tsc --noEmit` proves every `t()` call resolves.

```ts
// i18next.d.ts
import type { en } from './resources';
declare module 'i18next' {
  interface CustomTypeOptions {
    defaultNS: 'translation';
    resources: { translation: typeof en };
  }
}
```

### System-language detection

`detectSystemLanguage()` (in `index.ts`) maps the device locale to a shipped
language, **script/region-aware**:

- `zh` + `Hant`/`TW`/`HK`/`MO` → `zh-TW` or `zh-HK`; otherwise `zh` (Simplified)
- `es` → `es-MX`, `pt` → `pt-BR` (only those variants ship)
- `id`/`in` (legacy Android code) → `id`
- everything unsupported → `en`

Powered by `expo-localization` (`getLocales()`).

> ⚠️ `expo-localization` is a **native module**: after install you must rebuild
> the dev client (`expo run:ios|android`). Auto-detection is wrapped in
> `try/catch` and falls back to English, so manual selection always works.

### Preference & persistence

- The user's choice is `'auto' | <AppLanguage>`, cached in a module var
  `_preference`, persisted at **`vela.language`**, default `'auto'`.
- `loadLanguage()` runs once at startup (inside the `Promise.all` in
  `app/_layout.tsx`) **before** the first render.

> 🐞 **Persistence pitfall (we hit this).** Persist the choice **independently of
> `i18n.changeLanguage()`**. If you `await changeLanguage()` *before* writing to
> storage and that await ever rejects/hangs, the write is skipped — the in-session
> UI switches but the choice is lost on next launch and the picker silently
> reverts to "follow system". Order it: write first, then change language.

```ts
export async function setLanguagePreference(pref: LanguagePreference) {
  _preference = pref;
  AsyncStorage.setItem('vela.language', pref).catch(() => {}); // ← persist FIRST
  await i18n.changeLanguage(resolveLanguage(pref));            // ← then apply
}
```

Covered by `src/__tests__/i18n/language-persistence.test.ts` (persists even when
`changeLanguage` rejects/hangs; `loadLanguage` restores it).

### Instant switch (no restart)

Two mechanisms, belt-and-suspenders:

1. **react-i18next** re-renders every component using `useTranslation()` when
   `changeLanguage()` fires.
2. **Stack remount** — `app/_layout.tsx` keys the navigator by language:
   ```tsx
   <Stack key={`${colorScheme}-${language}`} />
   ```
   Changing language flips the key → the tree remounts → effects re-run (this is
   what re-localizes service-derived strings like the activity feed; see #4).

`LanguageProvider` (`language.tsx`) sits **above** the `Stack`, so its state
survives the remount. `useLanguagePreference()` exposes
`{ preference, resolved, systemLanguage, setPreference }`.

### Using translations

**In components** — the hook:

```tsx
import { useTranslation } from 'react-i18next';
const { t } = useTranslation();
<Text>{t('send.confirm')}</Text>
<Text>{t('home.toastReceived', { amount, token })}</Text>   // interpolation
```

**In non-React code (services)** — the singleton. Safe because it reads the
*current* language; call it at render time so it follows switches:

```ts
import i18n from '@/i18n';
return i18n.t('time.minutesShort', { n });
```

### Conventions & pitfalls

- **One namespace** → `t('area.key')`, never `useTranslation('area')`.
- **Dynamic keys** (`t(someVar)`) break typed-keys. Pass a `defaultValue` so the
  overload resolves and you get a sensible fallback:
  ```ts
  t(scenario.labelKey, { defaultValue: scenario.labelKey })
  ```
- **Module-level constants** with display strings can't call `t()` at module
  scope. Store a stable `labelKey` and translate at render:
  ```ts
  const THEME_OPTIONS = [{ key: 'light', labelKey: 'settings.appearance.themeLight' }];
  // …in the component: t(opt.labelKey)
  ```
- **Service-generated display strings** are the classic leak (titles, "to/from",
  relative time, status labels). Localize them via the `i18n` singleton **at
  render**, or carry a semantic field (e.g. `direction: 'in'|'out'`) and translate
  in the component. See #4.
- **Do NOT translate**: brand (`Vela`, `Vela Wallet`), tickers (`ETH`, `USDC`),
  technical identifiers (`Gas`, `UserOp`, `dApp`, `RPC`, `EVM`, `ERC-*`/`EIP-*`,
  `QR`, `WebAuthn`, `Face ID`), URLs, addresses/hashes, currency codes, numeric
  examples, and anything inside `{{ }}`.
- **Locale quality bar**: translations must read like a native product team wrote
  them. zh-TW ≠ zh-HK ≠ zh (real vocabulary differences); es = Mexican (`tú`,
  `billetera`), pt = Brazilian (`você`); ja/ko use the right politeness register.

### Add a string

1. Add the key to **`locales/en/<area>.json`** (this is the type source — `tsc`
   now requires it everywhere).
2. Add the same key to the other 12 `<lang>/<area>.json` files (translated).
3. Use `t('<area>.<key>')`.
4. Run the [parity check](#maintenance) + `tsc`.

### Add a language

1. Copy the English files as the structural base (guarantees key parity):
   ```bash
   L=it
   cp src/i18n/locales/en.json src/i18n/locales/$L.json
   mkdir src/i18n/locales/$L
   for f in src/i18n/locales/en/*.json; do cp "$f" "src/i18n/locales/$L/$(basename "$f")"; done
   ```
2. Register it in `index.ts`: add to the `AppLanguage` union, `SUPPORTED_LANGUAGES`,
   `LANGUAGE_NATIVE_NAMES`, and a `detectSystemLanguage()` branch.
3. Add its imports + spread + `resources` entry in `resources.ts`.
4. Translate the values (keep keys + `{{placeholders}}` identical).
5. Parity check + `tsc`. The picker (`SettingsScreen`) lists languages from
   `SUPPORTED_LANGUAGES` automatically.

---

## 2. Regional format (number / date / time)

This is the **"区域格式"** screen. It lives in `src/services/locale-format.ts` and
is **format-based, not country-based**: a short set of explicit presets the user
picks by live example, not an opaque "locale".

> **Why not `Intl`?** Hermes ships incomplete `Intl`/ICU data, so
> `Intl.NumberFormat`/`DateTimeFormat` are unreliable on-device. We format with
> explicit separators + patterns instead, so output is identical everywhere.

**Presets** (`src/models/types.ts`):

```ts
type NumberFormatKey = 'auto' | 'comma_dot' | 'dot_comma' | 'space_comma' | 'indian';
type DateFormatKey   = 'auto' | 'ymd_slash' | 'mdy_slash' | 'dmy_slash' | 'dmy_dot' | 'iso';
type TimeFormatKey   = 'auto' | 'h24' | 'h12';
interface LocalePrefs { numberFormat; dateFormat; timeFormat }   // default: all 'auto'
```

**`'auto'`** best-effort detects the device convention via
`Intl.*.formatToParts` (when available) and maps it to a preset; if `Intl` is
missing/locked it degrades to sensible defaults. Resolution is cached.

**Storage**: `LocalePrefs` at **`vela.localePrefs`**, with a synchronous in-memory
cache (`getLocalePrefs()`) warmed by `loadLocalePrefs()` at startup (so formatting
is sync and allocation-free in hot render paths).

**API** (all honor the current preset, or an explicit `key`):

```ts
formatNumber(1234567.89, { minimumFractionDigits: 2 })   // "1,234,567.89"
formatCompact(4.5e9)                                     // "4.5B"  (K/M/B/T)
formatTokenAmount(n, { compact })                        // token-amount display
formatDate(ts)            // "2026/06/13" | "06/13/2026" | "2026-06-13" | …
formatTime(ts)            // "13:45" | "1:45 PM"
formatDateTime(ts)        // date + time
numberSeparators()        // { group, decimal } for the active preset
```

**The picker** (`SettingsScreen`'s `FormatPickerModal`) shows each option as a
**live example** plus a translated note. Options come from
`numberFormatOptions()` / `dateFormatOptions()` / `timeFormatOptions()`, which
return `{ key, example, noteKey }`. The `noteKey` (`'system'|'indian'|'h24'|'h12'`)
is a **semantic token** the UI translates via `t('settings.formatNote.<noteKey>')`
— the service stays React-free.

> Note: number/date/time **format** is deliberately decoupled from UI **language**
> (and AM/PM is left in English as a near-universal time token).

---

## 3. Display currency (fiat)

Which fiat the user's balances are shown in — independent of language and format.
Full details in [`docs/fiat-price.md`](./fiat-price.md); the localization-relevant
surface:

- **`useDisplayCurrency()`** (`src/hooks/use-display-currency.ts`) → `{ code,
  symbol, rate, fmt }`. `fmt(usd)` converts a USD value into the selected currency
  and formats it **using the regional number preset** (concern #2). Refreshes on
  screen focus so changing the currency anywhere reflects everywhere.
- **`formatFiat(value, code, symbol)`** (`src/services/currency.ts`) — the actual
  string builder.
- **Rates**: Chainlink FX feeds on Ethereum mainnet (`fiat-rates.ts`), with
  `fiat-fx.ts` normalizing rate shapes; **`CURRENCY_CATALOG`**
  (`currency-catalog.ts`) provides each currency's name + symbol.
- Preference stored at `vela.currency`.

So a single amount flows through both currency and format concerns:
`usd → ×rate (currency) → grouped/decimal'd (format) → "₩1,234,567"`.

---

## 4. Relative time

The activity feed's `relativeTime()` (`src/services/activity.ts`) is a service
function, so it uses the **`i18n` singleton** and is called **at render** (so it
follows language switches):

```ts
if (diff < 45)    return i18n.t('time.now');                       // "now" / "刚刚" / "방금"
if (diff < 3600)  return i18n.t('time.minutesShort', { n });       // "16m" / "16分钟前" / "16분 전"
if (diff < 86400) return i18n.t('time.hoursShort',   { n });       // "4h"  / "4小时前"
// < 7d → localized weekday via d.toLocaleDateString(i18n.language, { weekday:'short' })
// else → formatDate(d)  (regional preset, concern #2)
```

The feed's **titles** (`Sent`/`Received`) and **`to`/`from`** subtitles are built
in `HomeScreen` from the item's **semantic** `direction`/`address` fields via
`t('activity.*')` — *not* from the English strings the service stores. This is the
general fix for "service strings won't translate": **carry semantics, translate at
render.** Keys live under `time.*` and `activity.*` in the core locale files.

---

## Maintenance

**Parity check** — every language must have the same keys + the same
`{{placeholders}}` as English. Run this before committing locale changes:

```bash
node -e '
const fs=require("fs");
const NS=["home","send","receive","assets","addToken","tokenDetail","history","onboarding","connect","about","clearSigning","componentsTx","componentsUi","settingsModals"];
const LANGS=["en","zh","zh-TW","zh-HK","ja","ko","vi","id","tr","es-MX","pt-BR","fr","de"];
const flat=(o,p="")=>Object.entries(o).flatMap(([k,v])=>v&&typeof v==="object"?flat(v,p+k+"."):[[p+k,v]]);
const ph=s=>(String(s).match(/{{\s*\w+\s*}}/g)||[]).sort().join(",");
const load=L=>{let o={...JSON.parse(fs.readFileSync(`src/i18n/locales/${L}.json`))};for(const n of NS)Object.assign(o,JSON.parse(fs.readFileSync(`src/i18n/locales/${L}/${n}.json`)));return Object.fromEntries(flat(o));};
const EN=load("en"),EK=Object.keys(EN);let bad=0;
for(const L of LANGS){const M=load(L),K=Object.keys(M);
  const miss=EK.filter(k=>!(k in M)),extra=K.filter(k=>!(k in EN)),phb=EK.filter(k=>k in M&&ph(EN[k])!==ph(M[k]));
  if(miss.length||extra.length||phb.length){bad++;console.log("FAIL",L,{miss:miss.slice(0,4),extra:extra.slice(0,4),phb:phb.slice(0,4)});}}
console.log(bad?bad+" FAILED":`ALL ${LANGS.length}: parity OK (${EK.length} keys/lang)`);'
```

Other gates:

- **`npx tsc --noEmit`** — typed keys mean this proves every `t()` call resolves.
- **`npx jest src/__tests__/i18n`** — language-persistence regression tests.
- **Residual-English scan** when adding screens (catches strings that skipped i18n):
  ```bash
  grep -rnE '<Text[^>]*>[A-Z][a-z]|(title|subtitle|label|placeholder)="[A-Z]' \
    src/screens src/components --include='*.tsx' | grep -vE "t\('|styles\."
  ```
- **Translation feedback** — in-app entry + `.github/ISSUE_TEMPLATE/translation.yml`
  let native speakers report wording that reads wrong.

---

## Reusing this in another project

The portable pieces (drop-in for any Expo / React Native app):

1. **`src/i18n/`** — the whole pattern: per-area files merged into one namespace,
   typed keys, `LanguageProvider`, system detection, **persist-before-apply**.
2. **The instant-switch trick** — `<Stack key={language}>` + react-i18next.
3. **`locale-format.ts`** — Hermes-safe, format-based number/date/time presets
   (no `Intl` dependency). Self-contained.
4. **The parity-check script** + persistence tests above.

Golden rules carried over from our bugs:

- Persist the preference **before** applying it (don't gate the write on an async).
- Keep **language / format / currency** independent.
- For service-layer strings: **carry semantics, translate at render** via the
  `i18n` singleton — don't store display English.
- One `translation` namespace; typed keys; `{defaultValue}` for dynamic keys.
