# Contract: i18n / SSR / Routes

## URL contract

| URL | Behavior |
|---|---|
| `/{locale}` for the 15 corpus tags | Prerendered Welcome page, fully localized HTML |
| `/` | Runtime 307 → `/{negotiated}`; headers: `Location`, `Vary: Accept-Language`, `Cache-Control: private, no-store`. No cookies (data-model.md). A language pinned in Settings lives in `localStorage` (`vela.language`): the locale page reads it before its first paint (inline in `app.html`) and, only when it names another supported tag, replaces itself with that locale's page — one hop, the page hidden meanwhile (set-12) |
| `/{locale}/create`, `/{locale}/import`, `/{locale}/settings/passkey-index` | Prerendered placeholder destinations (minimal, tokenized, localized titles) |
| unknown `/{segment}` | 404 |

Locale tags in URLs are the corpus tags verbatim (`es-MX`, `pt-BR`, `zh-TW`,
`zh-HK` keep their case; matching is case-insensitive with redirect to
canonical case).

## Negotiation contract (root redirect)

Parse `Accept-Language` per RFC 9110 quality ordering; for each candidate in
q-order: exact tag match (case-insensitive) → base-language map → next
candidate; exhausted → `en`. Base map (copied verbatim from RN
`src/i18n/shared.ts` `detectSystemLanguage`): `zh` distinguishes Simplified vs
Traditional — script `Hant` or region `TW/HK/MO` ⇒ traditional, then region
`HK`/`MO` ⇒ `zh-HK`, else `zh-TW`; all other `zh` ⇒ `zh`; `id`/legacy `in` ⇒
`id`; `es-*` ⇒ `es-MX`; `pt-*` ⇒ `pt-BR`; any other supported base language
matches itself regardless of region (`fr-CA` → `fr`).

## Head/document contract (every locale page)

- `<html lang="{tag}" dir="{engine.dir()}">` via `hooks.server.ts` transform.
- `<link rel="alternate" hreflang="{tag}" href="{origin}/{tag}">` × 15
  + `<link rel="alternate" hreflang="x-default" href="{origin}/en">`.
- `<link rel="canonical" href="{origin}/{tag}">`.
- `<meta name="color-scheme" content="dark light">`.
- `<title>` and `<meta name="description">` localized (new corpus keys
  `onboarding.welcomeWeb.meta.title|description`).

## Message-key contract (new corpus keys, all 15 locales)

Namespace file: `rust/crates/vela-core/i18n/locales/<locale>/onboarding.json`,
under top-level `onboarding.welcomeWeb`:

```
meta.title, meta.description, tagline, passkeyIndexLink,
features.noSeedPhrase.title, features.noSeedPhrase.description,
features.oneAddress.*, features.openSource.*, features.keysInPasswordManager.*,
features.safeContracts.*, features.stablecoinGas.*
```

Rules: camelCase segments; no `{{...}}` needed by any of these; zh-HK in the
corpus's spoken-Cantonese register; no translator notes in values; en + zh
authored from the design mocks (zh is the design source language), the other
13 best-effort and flagged for the standing human-review sweep. Reused keys
(`onboarding.welcome.createWallet|alreadyHaveWallet`) are NOT modified.

After editing: `npm run gen:i18n && npm run lint:i18n && npm run verify:i18n`
at repo root must pass with no unexpected diff beyond the new keys.

## Engine contract (build-time only)

`src/lib/i18n/engine.server.ts` may be imported only from `.server.ts`
modules; it `initSync`s `rust/pkg-web` once per build process, constructs
`I18n` with the `en` aggregate bytes, `loadCatalog`s the requested locale from
`rust/crates/vela-core/i18n/locales/{locale}.json`, and exposes
`resolveWelcomeMessages(locale): PageMessages`. It must never appear in the
client bundle nor in the runtime worker bundle for `/` (verified by asserting
the built `_worker.js` contains no `WASM_BASE64` marker).
