# 022 — Explore + dApp signing UI · results

What landed, per client, and what is still open. The canon is `data-model.md`;
the mocks are `design/explore/` (60 boards + 4 SPEC sheets).

## Shipped

**One vocabulary, four ports.** Every client got the same two component sets and
the same fixture canon — 33 signing scenarios (CS1–CS33) and seven explore
states (E1–E7 / DE1–DE4) — assembled by the same universal block renderer.

| | web | iOS | Android | desktop |
| --- | --- | --- | --- | --- |
| Explore start page (E1/E2) | ✅ | ✅ | ✅ | ✅ (DE1/DE2) |
| Group manager (E3) | ✅ | ✅ | ✅ | right-click menu (DE2) |
| Browsing + stub page (E4) | ✅ | ✅ | ✅ | ✅ (DE3) |
| Tab switcher (E5) | ✅ | ✅ | ✅ | tab strip (DE1–DE4) |
| Site menu (E6) | ✅ | ✅ | ✅ | ✅ (M3) |
| Connection panel (E7) | ✅ | ✅ | ✅ | ✅ third column |
| Signing sheet / panel | ✅ + desktop panel | ✅ | ✅ | ✅ third column |
| Signing scenarios in fixtures | 33 | 33 | 33 | 33 |
| Reachable in the real app | ✗ by design | ✗ **not shipped** | ✗ **not shipped** | ✗ **not shipped** — see the correction below |

> ## ⚠️ Correction, 2026-09-04 (spec 029)
>
> **The paragraph below was written about work that did not reach `main`, and it
> stood for two days as this repository's record of what shipped.** What 022
> delivered on the three native clients was the *screens* — drawn, translated,
> fixture-complete, and byte-identical to what was intended. What it did not
> deliver was their *routes*.
>
> | Client | What `main` actually had |
> |---|---|
> | desktop | `main.rs` declared neither `mod explore;` nor `mod signing;`. 3,571 lines never reached rustc; 5 `#[test]` never ran; the sidebar 探索 row was built with `None` as its destination. |
> | Android | No `EXPLORE` route in `VelaDestinations`, no `composable`. 4,353 lines compiled and were reached by nothing. |
> | iOS | `PageOverride.Page` had no explore or signing case; `ExploreScreen(` was never instantiated. 1,771 lines. |
>
> **Cause.** The wiring exists in `969bf8fc` on the unmerged `020-intro-carousel`
> branch. `feecb6e4` is a rebase of that work made to resolve an i18n-corpus
> collision with a second session in flight; it kept the 25,976-line corpus half
> and dropped the four wiring files. Nothing caught it because nothing in CI
> built any native client, and because on every platform the *compiler* was
> content — rustc does not read a directory nobody declared, and Kotlin and Swift
> happily compile a screen nothing routes to.
>
> **Repaired by spec 029**, which lands the wiring on all three clients and adds
> the guard (`scripts/check-native-reachability.mjs`) and the three CI jobs that
> would have caught it. The claim below is true as of that feature, not this one.

**Wired, not gallery-only.** Explore is a real destination in the signed-in
shell on the three clients that can have one (iOS `RootView`, Android
`VelaNavHost`, desktop `Section::Explore`), and it shows the SIGNED-IN account,
not the fixture one — a connection panel naming a stranger's account would be
the wallet lying about what it just granted. Business state is still fixtures:
no browser engine, no dApp registry, no chain calls.

**The web has no Explore entry** (founder call, 2026-09-02): a page inside a
browser cannot host a browser, so `[locale]/wallet` renders three tabs and the
desktop-web sidebar three rows. `TabBar`'s new `destinations` prop is where that
choice lives; the gallery boards still draw all four, because they reproduce the
phone mocks. The web's explore + signing libraries stay: they are the design
source for the other three clients, and the signing vocabulary is what a future
web connection path (extension pairing, WalletConnect) would render.

**The renderer is universal.** `SigningModel` is a header, an ordered list of
blocks and a fixed footer (technical details → fee → signer → slide). Nothing in
the renderer knows what "a swap" is, so the six-rung ERC-7730 degradation ladder
is structural: a deeper rung emits more warning blocks and fewer decoded ones.
Two product contracts are enforced in the fixtures and asserted in tests:

- **The slide is the only confirmation.** There is no reject button anywhere;
  dismissing rejects.
- **Never unlimited.** CS5's `requested` chip is disabled and its slide is off
  (`unlimited_approval_cannot_be_confirmed_as_requested`, desktop).

**i18n.** New namespace `explore.json` (54 keys) + 43 additions under
`componentsUi.signing`, all 15 locales, hand-written per locale. ~95% of the
signing copy already existed. Registered in the six places that must agree:
`gen-i18n.mjs`, `lint-i18n-corpus.mjs`, `verify-i18n-parity.mjs`,
`dump-vectors/i18n.dump.mjs`, `resources-generated.test.ts`, and the three Rust
corpus tests.

**Tokens.** 15 web size tokens (`WEB_ADDITIONS`), `ExploreGeometry` (iOS),
`ExploreMetrics` (Android), consts in `explore/components.rs` (desktop) — every
value MEASURED off the PNGs, not estimated (the lesson spec 018 recorded). The
web literal audit now covers `src/lib/explore` and `src/lib/signing`.

## Gates

- web: `pnpm check` (0 errors), `pnpm lint`, `pnpm test:unit` (169), `pnpm build`
  (147 prerendered gallery pages incl. e1–e7, de1–de4, cs1–cs33).
- iOS: `xcodebuild build` green; `audit-literals` clean for spec-022 files (the
  three remaining violations are pre-existing, in Onboarding).
- Android: `:app:compileDebugKotlin` + `:app:testDebugUnitTest` green.
- desktop: `cargo check`, `cargo clippy` (no new warnings), `cargo test` (85).
- i18n: `lint:i18n` and `verify:i18n` were green for this batch when it landed.

## Visually verified

- web: `/zh/gallery/{e1…e7,cs1,cs5,cs11,cs29,cs33,de2,de3,de4}` against the
  mocks (Playwright screenshots, light + dark).
- iOS: simulator, `VELA_PAGE=explore` (E2) and `VELA_PAGE=signing VELA_STATE=cs1/cs5`.
- desktop: `VELA_PAGE=explore` on macOS — DE2 start page.

## Open

1. **Android on-device eyeball.** The OnePlus 5T (e93a3fa) stayed locked during
   the run; the build installs and the unit tests pass, but nobody has looked at
   it on glass. It is also the 1.3×-font baseline, so it is the device that
   matters most for the tile labels.
2. **Desktop browsing + third column** are code-verified and compile, but the
   screenshot pass only covers the start page (synthetic clicks need
   Accessibility permission this host does not grant).
3. **Desktop technical details** renders the collapsed disclosure row only; the
   five-layer panel (CS29) is phone-only so far.
4. **Motion** is the static half of the SPEC boards: sheet rise, tab zoom and
   the slide's spring are implemented; the pull-to-refresh sail, the 250 ms
   start→browsing push and the connection dot's scale-in are not.
5. **Real data** — browser engine, dApp registry, live descriptors and
   simulation — is deliberately out of scope here.
