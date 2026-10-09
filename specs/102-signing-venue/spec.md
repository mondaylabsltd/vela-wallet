# 102 — Where you review and sign: in Vela, or on a page you trust

**Status:** draft · 2026-10-09 · owner decisions recorded below
**Supersedes:** the "fourth key method" shape of the Trusted Signer (075 ruling, 2026-09-22)
**Research:** [research.md](research.md) (end-to-end trace, RP IDs, integrity, UX, risks — file:line cited)

## Why

People who tap 可信签名器 see "this device / scan a code / USB security key" again and can't tell what is trusted about it (user feedback, 2026-10-09). The owner's diagnosis: the Trusted Signer is **parallel to in-app signing**, not a fourth place a key lives — the same three key places exist on the page too. The real choice is **where the signing content is previewed and signed**:

- **In Vela** — the app's own sheet, which sits on many third-party dependencies; a library could change how the intent is shown.
- **On a page you trust** — a zero-dependency, self-deployable page that decodes the operation itself (what you see is what you sign), whose published build can be checked against a content hash.

The code already says this (`trusted_signer.rs:4-6` "where the person checks what they sign"); the UI and the data model still say otherwise.

## Owner decisions (2026-10-09)

| # | Question | Decision |
|---|---|---|
| D1 | How is the venue chosen? | **Per account** (on this device). Changeable at any time without touching keys. Not per transaction. |
| D2 | Key isolation? | **Shared domain first.** Keys stay on `getvela.app`; any such key can sign in either venue. The page assures *what you see is what you sign*; it does not cryptographically stop a fully compromised app (a later "vault mode" may add page-only keys). |
| D3 | Self-hosted signing pages | **Customisable, several can be saved, one is active.** Each account **remembers the signing domain (RP ID) its keys live under**, because a passkey created under the person's own domain cannot be reached from Vela's official page (and vice versa). The experience must make that obvious and never let someone pick a page that can't reach their keys. |
| D4 | What the in-app sheet shows when the venue is the page | **A minimal hand-off card**: "Review and sign on your trusted page" + the key you'll confirm with + one integrity line (version · matches the published build list · checked …) + Open. The app does not repeat the transaction preview; the page is the authority. |
| D5 | Old clients | Alpha, no real users: optimise for the new model, but existing accounts migrate automatically and older builds keep working (see [[vela-alpha-no-backcompat]] in memory). |

## The model

Two independent axes:

1. **Key place** (unchanged list of three): this device (platform passkey) · phone or tablet (scan a code) · USB security key.
2. **Signing venue**, per account: **In Vela** | **Trusted page** (a specific saved page).

And one property of every account:

3. **Signing domain** — the RP ID its keys live under: `getvela.app` for every account created in the apps or on the official page; the person's own domain for an account created on a self-hosted page.

Rules (owned by vela-core; shells draw):

- R1. A venue is **reachable** for an account only if it can use the account's keys: In Vela ⇔ signing domain = `getvela.app`; a trusted page ⇔ the page's RP ID = the account's signing domain. Unreachable choices are shown disabled with the reason ("This page's domain can't reach this account's keys").
- R2. An account on a custom signing domain is **locked to a page on that domain** (the apps can't use those keys natively). If several saved pages share that domain, the person picks which one is active for the account.
- R3. Key ceremonies (create, add key, sign-in, member proof) run in the app for `getvela.app` accounts — the challenge is random/derived, there is nothing to preview. For a custom-domain account they run on its page (only that page can mint/use those keys).
- R4. The venue applies to transactions and messages (user operations, personal_sign, typed data).
- R5. The page is told **which key to use** (credential id + transports + hints), so the browser goes straight to it — no generic "where is your passkey" chooser.
- R6. The integrity check is **enforced** before the word "trusted" is used anywhere: the URL that is opened is the version that was checked (one core rule for both), every platform checks it (phones included), a failed check refuses to open, and the result is shown on the hand-off card. Wording: "matches Vela's published build list · checked <time>", never "certified untampered".
- R7. The page only answers signatures to `velawallet://` (closes the open-signing hole; see Phase 0) and cannot be framed.

## User-facing surfaces

- **Create / sign-in chooser:** three key places only. An "Advanced: use my own signing page" entry creates or signs into a custom-domain wallet (asks for / picks a saved page, shows its domain and integrity line).
- **Settings → Signing pages:** the list of saved pages (official always first; custom ones added by URL, each showing its domain and integrity status); add / remove / rename; no free-text "trusted signer URL" field any more.
- **Account settings → Where you review and sign:** In Vela | Trusted page (with the active page). Shows the account's signing domain. Disabled options carry the reason (R1).
- **Signing sheet, venue = page:** the hand-off card (D4). Waiting / closed / refused / mismatch / timeout states as today, reworded.
- **Keys list:** keys are captioned by where they live (this device / phone / USB), never "Trusted Signer"; a custom-domain account shows its domain.
- **The page:** shows the intent first; uses the key route so the browser asks for the right key only; 15 languages.

## Phases

- **Phase 0 (urgent, separate PR):** the page refuses signature answers to anything but `velawallet://` (a test-only exception for the desktop loopback harness), `frame-ancestors 'none'`; rebuild → new hash at the front of `BUILD_ALLOWED`; the official page must be deployed before an app that launches the new hash ships.
- **Phase 1 — core:** venue + signing domain + saved pages in the account/settings model; migration of today's records; reachability rule (R1/R2); key route in the request (R5); one launch-URL-and-check rule + enforcement (R6); retire `KeyMethod::TrustedSigner` for new records (reader kept); i18n (new venue/hand-off/integrity strings; stale strings fixed).
- **Phase 2 — shells (web, desktop, Android, iOS):** choosers, Settings → Signing pages, account venue setting, hand-off card, phone integrity checks, keys-list captions.
- **Phase 3 — the page:** key route + hints, intent-first layout, 15 languages, rebuild + hash + deploy.
- **Phase 4 — verification:** every platform on devices (Xiaomi, iPhone), including a self-hosted page on a test domain.

## Migration

- `signer_origin` empty or the official origin → signing domain `getvela.app`, venue = trusted page (official), key route from the key's stored transports.
- `signer_origin` = a custom origin → signing domain = that host, venue locked to that page; the page is added to the saved list.
- `signer_origin` keeps being written so older builds still route to the page. New fields default safely when an older build reads them. Nothing changes in the registry or on-chain.

## Owner rulings after Phase 1–3 (2026-10-09)

- **Naming (D6).** "My own signing page" is only for a page the person deployed. A trusted signing page is either **Vela's official page** or **one you deployed yourself**. Wording: the venue choice is 「在可信签名页预览并签名」 / "Review and sign on a trusted signing page"; the create / sign-in entry is 「使用可信签名页」 / "Use a trusted signing page", whose list shows 「Vela 官方签名页」 / "Vela's official signing page" and 「自己部署的签名页 · <domain>」 / "Self-hosted · <domain>"; adding one is 「添加自己部署的签名页」 / "Add a self-hosted signing page". The setting itself: 「在哪里预览并签名」 / "Where you review and sign".
- **The page looks like the app's signing sheet (D7).** Same palettes (light/dark), type, rows, fee block, pill confirm button; one accent (the primary on the confirm button); no chain chips or tinted badges; warnings in the app's styles. "Too colourful" was the owner's verdict on the Phase 3 build.

## Core round after Phase 2 (gaps the shells hit)

1. Record trust in an unknown version of a self-hosted page ("Trust this version on this device?" — event + strings).
2. `checked <time>` wording/format from the core (one rule for every shell).
3. `key_label()` — the "Confirm with {key}" name.
4. A stale check shows "checking" while it re-runs; checks refresh in the background before they expire.
5. The hand-off card keeps a compact fee + speed row (the fee is chosen in the app before the hand-off).
6. The launch URL carries the app's language (`lang=`).
7. A corpus reason for venues the web cannot use; a `venue_blocked` failure kind for a sign-time refusal (translated).
8. plan.md: on the web, P2-08 is not wired and P2-09 is read-only.
9. The renaming in D6 (15 locales).
10. Words for Settings → Signing pages: trust a version, rename, remove, the page's name (the phones borrowed other screens' keys).
11. The signing plan carries the key's display name (item 3), so no shell reads the account record for it.
12. A key ceremony on a self-hosted page has its own title (create / sign in / confirm), not the hand-off card's.
13. A freshness check for an admitted page, across the bindings.
14. The check fetches what a browser gets: the core names the request headers (a host rewrote HTML for navigations only).
15. A page address is normalised (full-width forms a CJK keyboard types) and validated (scheme, ASCII host, port), or refused with a translated reason.
16. The ceremony-request binding's doc no longer says `method = trusted_signer`.
17. The key label never repeats the "Signing account" row: a key named after the wallet is named by its place.

All seventeen are done in the core — see plan.md, "Core round after Phase 2" and "Phase 2b".

## Open items

- iOS subdomain RP-ID behaviour for a future vault mode (device check needed) — out of scope here.
- Public monitor that fetches every `/b/*` and compares to `dist/` — nice to have, not blocking.
