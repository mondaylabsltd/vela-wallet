# Audit report — 080

Findings on getvela.app and the root README, verified against `main` at
`dd482131`, the service repositories, and primary external sources (research.md).
Severity: **Fatal** — an expert reader would call it false and stop trusting the
site. **High** — wrong in a way that costs a user money, keys or a decision.
**Medium** — stale, imprecise or inconsistent without direct harm. **Low** —
voice and clarity.

"If we recommended Vela to Ethereum's founder tomorrow" is the Fatal list.

## Fatal

| # | Where | What it said | Why an expert would stop reading | Fix | Status |
|---|---|---|---|---|---|
| F-1 | Whitepaper, threat model | "Compromised Vela server — yields no signing ability; the blast radius is degraded service, not loss of funds." | For a web wallet the server delivers the code that builds the transaction and renders the summary; a compromised delivery asks you to sign something else and the passkey prompt can't tell. That is exactly the Bybit lesson the site's own Bybit page teaches. | Threat model split: backend services (no signing power) vs app delivery (can present a malicious transaction), plus domain takeover; mitigations named honestly. | fixed |
| F-2 | Account contract | "If Vela is gone tomorrow … any Safe-compatible interface can drive it." | A passkey is bound to its rpId (`getvela.app`); Safe{Wallet} or any interface on another origin cannot get an assertion. The claim confuses reading an account with signing for it. | "Safe tools can read it and build transactions; signing needs software that can ask for a `getvela.app` signature" + link to the list of what can. Same correction as a dated note on the blog post that said it. | fixed |
| F-3 | Audits page | SafeWebAuthnSharedSigner "represents exactly one passkey per Safe" while every other page promised up to seven keys; the factory and singleton the second-to-seventh keys use were not listed anywhere. | Irreconcilable to anyone who knows the passkey module; the contract list was incomplete for every multi-key wallet. | Keys 2–7 use SafeWebAuthnSignerFactory `0x1d31…1195` / singleton `0x4E27…CA50`; both added with their audit coverage; address table extended. | fixed |
| F-4 | Audits page | Safe4337Module v0.3.0: "no unresolved findings above informational level." | Certora's August 2026 review found a Medium (M-01), acknowledged and not fixed; Nethermind reviewed it too. The page was out of date on the one contract every operation passes through. | Three reviews listed; M-01 explained under Known issues with Vela's exposure. | fixed |
| F-5 | Bybit page | Vela removes "the upgrade primitive the attack relied on" / "no contract we can upgrade". | The attack used an owner-signed DELEGATECALL; Safe + the 4337 module still allow it, and Vela's own batches use it. Holding no admin role is a different claim. | Reworded: no admin role; the primitive still exists behind a valid signature; the defences are decoding and an independent check. Also states a dApp can request a call to the account itself and Vela decodes but does not block it. | fixed |
| F-6 | Whitepaper vs landing FAQ | Whitepaper: domain loss has "no consumer-grade path". FAQ: "sign from the Vela extension or the clear-signing extension." | Opposite answers to the most important ownership question, one of them wrong in both directions: the extension works (host-permission rpId), the clear-signing extension cannot operate a wallet on its own and is not published. | One answer everywhere, in the self-hosting guide's "if getvela.app disappears" section: extension (any key), self-built apps (phone QR / security key), store apps (depend on association files). | fixed |
| F-7 | Fees page, FAQ, whitepaper, README | A per-network, non-refundable "gas account" activation deposit, sponsored "for new users". | The mechanism no longer exists in the relay or any shell. Describing a deposit that isn't taken, and omitting what is (the in-band fee formula), is the kind of mismatch that reads as not knowing your own product. | Removed everywhere; replaced by the treasury top-up that does exist (voluntary, non-refundable, doesn't pay for your transaction). | fixed |
| F-8 | README, whitepaper, FAQ, landing FAQ | Fee "≈2×", "3× the raw on-chain cost", "no hidden markup". | Code: `3 × padded gas limits × max(wallet price, relay fast-tier price)`, min ≈ $0.01 — usually several times the actual cost. Every multiple in circulation understated it. | Formula stated in plain words on the fees page, whitepaper, README; landing trade-off says "several times the real on-chain cost". The marketing essay's "about twice" is flagged for the founder (below). | fixed (site); flagged (marketing draft) |
| F-9 | Privacy policy | "We do not monitor, log, or analyze your on-chain transactions"; "never sent to Google"; "on-device data is deleted when you log out"; RPC preferences "not sent to our servers"; the public-key index not disclosed at all. | Each is contradicted by code: the relay logs sender + op hash and keeps ops 1 h–14 d; the site loads Google Fonts; sign-out keeps history/contacts/settings; the `X-Rpc-Url` header sends the top RPC URL (possibly with a provider key) to the relay; the index sees every registration and lookup. | Policy rewritten flow by flow, with retention, company number, a privacy contact and a new date. | fixed |
| F-10 | Everywhere | "12 networks" | The wallet has 24 since 17 September; README said 24. | 24 everywhere; a test now compares the site list with vela-core and every locale's heading. | fixed (en, zh; 13 locales in Phase 8) |

## High

| # | Where | Finding | Fix | Status |
|---|---|---|---|---|
| H-1 | Create wallet, recovery, passkeys, whitepaper, privacy, terms, about | Single passkey, iCloud/Google only, "your face or fingerprint" *is* the passkey. The product creates 1–7 keys of three kinds, and requires a second key when the only one syncs nowhere. | Keys described the same way on every page (claim ledger C-keys-1/2, C-auth-1). | fixed |
| H-2 | Landing trade-off #3 | Said only that Safe is audited; A02's mandated sentence (Vela's own code is not audited, none scheduled) was missing from the landing page. | Title now "The contracts are audited. Vela's own code is not." | fixed |
| H-3 | Landing compare row "An extra check", FAQ | The independent signing page presented as an available extra check. It is built, unpublished, and no app sends it requests. | Row reads "built but not yet connected to the apps", tone changed; docs state status. | fixed |
| H-4 | Signing-page doc | "No network requests"; recommended for every signature today. | It loads token logos from Vela's chain-data host; it can't be used for real signatures until apps route to it. | Corrected; "When to use it" → "Where it fits". | fixed |
| H-5 | Self-hosting (landing fact #4 → README table) | The ownership claim linked to a table of four repos and a false validation claim ("the wallet validates three checks before accepting"). No steps, no requirements, no limits. | New `/docs/self-hosting`: map of every dependency, the rpId limit and escape hatches, per-app endpoint support (incl. the iOS gap), step-by-step for relay / index / chain data / rates / apps, with the undocumented `P256_INDEX_DOMAIN_REGISTRY` and per-chain treasury funding. | fixed |
| H-6 | Clear-signing doc | "Unlimited approvals are blocked" without saying signed permits (EIP-2612 / Permit2) are signed as requested; "verified" implied authentication of fetched descriptors. | Scope of the guard stated; "verified" defined as "a descriptor for this contract was found". | fixed |
| H-7 | Terms | Nothing about fees, store purchases, 1-of-n key risk, unaudited integration; liability clause named a product, not the company; no statutory carve-out, governing law or contact email; date not updated after two edits that bind every user through the onboarding checkbox. | Rewritten; flagged for legal review. | fixed; legal review recommended |
| H-8 | Roadmap | Address book "next" (shipped); DApp Connect without phone (dropped — WalletPair); more networks (shipped); a path for chains without P-256 (contradicts the address derivation); an independent audit (violates the no-audit-scheduled stance); accounts "already follow you through platform backup" on iOS/Android (false for the native apps). | Rewritten from shipped work; upcoming items limited to what is true. | fixed |
| H-9 | Install, introduction, get-started | "Nothing to download, no app store, mobile coming soon"; "built from one codebase"; Mac App Store listed as coming. | Every platform with cost and status; "one wallet, wherever you open it"; Mac App Store removed (blocked by sandboxing). | fixed |
| H-10 | Comparison table | MetaMask "Sponsored gas: not offered", "open source", "EOA"; Base Account naming after the 10 Sep rename; Base "if the service goes away, the wallet goes with it". | Updated to current, sourced facts; the Base criticism restated fairly (closed signing service; no published path for the passkey). | fixed |
| H-11 | `/api/transactions` (site server) | The `network` query parameter went into the Alchemy hostname with the API key in the path — a key-exfiltration bug. | Allowlist of known slugs. Deleting the five dormant routes is recommended. | fixed |

## Medium (selection)

- Gas "1.5–3× an EOA transfer" → measured ~143–168k gas per send vs 21k (on-chain data, Gnosis).
- Security page: Safe bounty tier wording, griefing-vector impact understated and mempool exposure overstated, DefiLlama provenance, pathUSD wording, "most widely tooled" superlative, "we state it in the site header".
- Signers: native NFC not supported; desktop "this device" and Windows less tested; cross-chain reason owners can't change.
- Send & receive: names resolve only from address to name; split/sweep/batch undocumented; activity can miss log-less native deposits.
- Recovery: omitted the Gnosis → Ethereum registry fallback and that two-signature recovery covers single-key wallets only.
- About: "the smart contracts" (Vela doesn't write the account contracts); "biggest cause of lost crypto".
- Licences: "all four services MIT" — p256-index has no licence file.
- Footer linked the legacy index site (biubiu.tools); landing linked a third-party chain-setup tool instead of the site's own page.
- Translated docs: nothing tracked drift (now `source:` fingerprints); zh links convention now tested.

## Low

- AI/marketing voice removed from landing, about and get-started ("not a slogan but the architecture", "no faceless company", "Your keys, your coins").
- Missing full stop in a trade-off; inconsistent "安全钥匙/安全密钥" in zh.

## Not fixed here — product gaps found by the audit

Recorded so the docs can stay honest until they are fixed; each has file
references in research.md §3b and the reports.

1. iOS Settings → Service Endpoints shows fixture data and saves nothing (event field mismatch).
2. Web onboarding ignores a custom passkey index (`setRegistryUrl` never called); desktop ignores it in sessions that start signed in; Android name lookups always use the default.
3. The relay hard-codes `ethereum-data.getvela.app` (`vela-relay/src/utils/rpc.rs:15`).
4. p256-index: no LICENSE; `P256_INDEX_DOMAIN_REGISTRY` undocumented; source Dockerfile misses `p256-replay`. vela-relay source Dockerfile misses workspace members.
5. No guard on dApp-requested calls to the account itself (addOwner / enableModule / setFallbackHandler) — decoded, not blocked.
6. Fetched ERC-7730 descriptors are labelled "verified" without authentication.
7. `X-Rpc-Url` sends the user's top RPC URL (possibly with a provider API key) to the relay, and the relay reads a different header name anyway.
8. Desktop "Erase this device" has no handler; iOS erase leaves browser data; desktop shows the old "fee reserve" copy for the treasury top-up; desktop "Self-hosting guide →" is plain text; the web one links to the repo root (should link `/docs/self-hosting`).
9. Web feedback "Send" does nothing; the site's `/api/bug-report` has no caller.
10. The site loads a third-party-origin analytics script on the same origin as `/chain-setup`, which keeps a funded deployer key in localStorage.
11. Five dormant site API routes (`/api/wallet`, `/api/transactions`, `/api/nft`, `/api/bundler`, `/api/proxy`) forward wallet addresses to Alchemy/Pimlico with no caller.
12. The iOS app has no privacy manifest (App Store blocker per the privacy report).

## For the founder

- **Fee narrative vs code.** `docs/marketing/why-we-charge.md` promises "about twice the raw on-chain cost"; the code charges `3 × padded gas × fast-tier price`. The site now describes the code. One of the two should change — `estimateInBandBasisGas` (unpadded basis) exists in the web shell and is never called.
- **Legal review** of the rewritten terms (statutory carve-out, governing law, store-purchase clause) and privacy policy (controller scope, ICO reference).
- **Analytics operator.** The policy names Rybbit at `tj.appsdata.org`; it no longer claims "self-hosted" because the repo can't confirm who runs that host.
- **Landing nav.** Spec 059 fixed it at two links (Why, FAQ). Docs are reachable from the fact rows and the footer only; a "Docs" link would help the expert reader. Not changed — a founder ruling.
