# Vela Wallet — 100 Clues (Content & SEO Source Document)

> **用途 / Purpose.** 这是一份"事实库"母文档，用来派生大量 SEO 文章、用户指南、开发者文档、对比页、落地页文案等。
> 每条线索都来自真实源代码 / 官方文档（附文件路径），**不是**对钱包的泛泛假设。写任何对外内容时，从这里取事实，不要凭空发挥。
> This is a grounded "fact bank" master document used to spin off SEO articles, user guides, developer docs, comparison pages, and landing-page copy. Every clue is sourced from real code or official docs (path cited) — never generic wallet assumptions.
>
> **排序 / Ordering.** 100 条按**重要程度从高到低**排序（1 = 最重要、受众最广、SEO 价值最高）。每条带一个分类标签，便于按主题筛选。
>
> **派生内容 / How to use.** 见文末 [Content Mapping](#content-mapping-clue-clusters--content-pieces)：把线索聚成簇，每簇对应一篇可写的文档，附目标关键词与受众。

---

## ⚠️ Content-Accuracy Guardrails (read before writing anything)

These come straight from the codebase and override intuition. Violating them produces factually wrong content.

- **The main repo `README.md` is STALE. Treat `getvela.app/` docs + `docs/store-submission/` as canonical.** The README still says **"8 EVM networks"**, a **"~60% markup (gasPrice × 1.6)"**, a **"one-time" gas deposit**, and **"DApp Connect over Bluetooth (BLE)"**. The current facts are **12 networks**, **~2× cost with a ~3× cap**, a **non-refundable + re-activatable** gas account, and **dApp Connect over a WalletPair WebSocket relay — Bluetooth was dropped entirely.**
- **There is NO Bluetooth.** Source headers say so explicitly ([src/services/walletpair-transport.ts](../src/services/walletpair-transport.ts):5-9). Legacy type names like `BLEIncomingRequest` are artifacts. Never write "pair over Bluetooth."
- **Audit phrasing is mandated.** There is **no third-party audit and none is scheduled.** Never write "audit planned/coming." Allowed framing: Safe contracts are independently audited; Vela's *own integration* is not, and review = open source + community + AI-assisted, explicitly *not* equivalent to a professional audit. (`getvela.app/src/content/docs/whitepaper.md`, `docs/store-submission/privacy-and-review.md`)
- **"No token, ever."** No airdrop, no farming, nothing to buy.
- **Vela is in alpha** — say so honestly; do **not** add scary "tolerate bugs / beta" banners (brand stance is trust > disclaimers).
- **`src/services/deployer-api.ts` is a mock/placeholder.** Its address-derivation and deployment flow are simulated — never present them as production behavior. Production bundler facts come from [bundler-service.ts](../src/services/bundler-service.ts).
- **Typeface is Plus Jakarta Sans**, even though the theme export is still named `inter` ([src/constants/theme.ts](../src/constants/theme.ts)). Don't write "uses Inter."
- **Canonical URLs:** `getvela.app` (site) and `wallet.getvela.app` (wallet) only — flagged as an anti-phishing fact.

---

## Tier 1 — Foundational Identity (what Vela *is*) · clues 1–15
*Broadest audience, highest SEO priority. These seed the homepage, "what is Vela," and every top-level intent page.*

**1. A wallet with no seed phrase — the founding thesis.** Each wallet is controlled by a WebAuthn passkey (P-256) held in the device secure enclave and unlocked by Face ID / Touch ID / fingerprint. "There is no secret you can type," so there is nothing to phish. Source: `getvela.app/src/content/docs/whitepaper.md`, [src/modules/passkey/](../src/modules/passkey/). **Keywords:** seedless wallet, no seed phrase, passkey wallet, wallet without recovery phrase.

**2. Positioning & taglines (use verbatim).** Brand line: **"Your keys. Your face."** Product: **"an open-source, self-custodial wallet for ETH and ERC-20s."** Secondary: **"A wallet that does less — on purpose."** Source: `getvela.app/src/routes/+page.svelte`. **Never improvise alternative taglines.**

**3. The trust model — "we architecturally can't."** "We can't access your keys. Not 'we promise not to' — we architecturally can't." Signing uses OS-managed WebAuthn keys; Vela never has the private key. Source: whitepaper.md, [README.md](../README.md) Security Model. This "verify, don't trust" framing is the single most distinctive differentiator — lead with it.

**4. The account model is concrete and standards-based.** Every wallet is an unmodified **Safe v1.4.1 smart account**, operated via **ERC-4337 account abstraction (EntryPoint v0.7)**, with the **Safe 4337 Module** and a **WebAuthn signer (SafeWebAuthnSharedSigner)** as the sole owner (threshold 1). Source: [src/services/safe-address.ts](../src/services/safe-address.ts):21-28, 106-188; whitepaper.md. **Keywords:** Safe smart account, ERC-4337, account abstraction, smart contract wallet.

**5. Biometric on every transaction.** Each wallet is bound to a passkey credential; every transaction requires a fresh biometric verification (Face ID / fingerprint). No "unlock once, sign freely" session. Source: README Security Model, [src/hooks/use-dapp-signing.ts](../src/hooks/use-dapp-signing.ts).

**6. One address on every chain, and you can receive before it exists.** The address is computed deterministically from the passkey public key via CREATE2 *before* deployment (counterfactual). chainId is **not** an input, so the address is **identical on every network**; the contract self-deploys (paid from its own balance) on the first transaction. Source: [src/services/safe-address.ts](../src/services/safe-address.ts):220, [src/services/eth-crypto.ts](../src/services/eth-crypto.ts):231. **Keywords:** counterfactual wallet, same address every chain, deterministic address.

**7. Recovery = your platform passkey sync, stated honestly.** Recovery is the passkey synced by **iCloud Keychain (Apple)** or **Google Password Manager (Android)** — no seed phrase, no social recovery, no guardians. Honest caveat repeated everywhere: lose **both** your device **and** your cloud-synced passkey and the account is **unrecoverable by design.** Source: whitepaper.md, `getvela.app/src/content/docs/recovery.md`.

**8. No token, ever.** "Vela has no token and no plans for one. There is nothing to buy, farm, or speculate on." Source: whitepaper.md. Strong anti-scam / trust signal; counter-positions against airdrop-bait wallets.

**9. Alpha status — honesty as positioning.** Status is shown openly as **"alpha · v0.1"** in the site top bar. Brand voice = candor over hype ("Honest about trade-offs… we write down what, and why"). Source: `getvela.app/src/content/blog/vela-is-in-alpha.md` (2026-06-16), `about/+page.svelte`.

**10. Audit status (mandated phrasing).** Safe contracts are independently audited; **Vela's own integration has NOT had a third-party audit and none is scheduled** — "a goal for when the project can fund one, not a commitment with a date." Source: whitepaper.md, `docs/store-submission/privacy-and-review.md`. See guardrails above.

**11. Fully open source and self-hostable.** App + all three backend services are **MIT-licensed and self-hostable** via Settings → Advanced → Service Endpoints. Source: whitepaper.md, [README.md](../README.md). This "self-hostable wallet" claim is uniquely defensible — competitors can't match it.

**12. Clear signing, not blind signing (ERC-7730).** Vela decodes both calldata *and* EIP-712 typed data into human-readable **Intent / Substance / Details**, color-coded by risk. Undecodable calls get an explicit blind-sign warning instead of a fake summary. Source: [src/services/clear-signing.ts](../src/services/clear-signing.ts), `docs/clear-signing.md`. **Keywords:** clear signing, no blind signing, ERC-7730, readable transactions.

**13. "Does less on purpose" — deliberate minimalism.** No NFT gallery, no built-in swaps, no DeFi dashboard, no in-app dApp browser. "Fewer paths to attack, fewer moving parts to audit." dApp connection is delegated to WalletPair rather than an embedded browser. Source: `+page.svelte`. Turns a feature gap into a security virtue.

**14. 12 EVM networks + custom networks.** Ethereum, BNB Chain, Polygon, Arbitrum, Optimism, Base, Avalanche, Gnosis, **Unichain, Tempo, Monad, World Chain** — plus user-added custom chains. Source: [src/models/chains.ts](../src/models/chains.ts):42-119. (README's "8 networks" is stale.)

**15. Web-first, MIT-licensed, native apps coming.** Runs in the browser with nothing to download (web wallet free); native iOS/Android share one React Native + Expo codebase and are in device testing ahead of store release. License: MIT. Source: [README.md](../README.md), `roadmap/+page.svelte`. **Keywords:** browser wallet, no download, open source crypto wallet.

---

## Tier 2 — Key Differentiators & Trust Mechanics · clues 16–35
*These seed comparison pages, "how it works," security explainers, and pricing/fees content.*

**16. "Don't trust us — verify."** Every wallet is on-chain, every line is on GitHub, every claim is checkable. The homepage even shows a **live on-chain "wallets created" counter** read from the Passkey Index contract on Gnosis (`0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3`) through an auto-failover pool of public RPCs — the page demonstrates the architecture. Source: `getvela.app/src/routes/+page.svelte`:92-206.

**17. Transparent, capped fee model.** Gas is paid from **your own wallet's native-token balance** (no paymaster). The **bundler is the single source of truth for gas price** (wallet never marks it up); you pay roughly **2× the raw on-chain cost** (network fee + relayer fee, shown split before you confirm), and the wallet **refuses any quote above ~3× the network rate** (`GasQuoteTooHighError`, `MAX_QUOTE_VS_CHAIN_MULTIPLE = 3n`). Source: [src/services/safe-transaction.ts](../src/services/safe-transaction.ts):1305,1373-1378; whitepaper.md. (README's "60% markup" is stale.)

**18. Per-Safe, per-chain "gas account" (relayer EOA).** The first tx on each network activates a dedicated relayer account. New users may get **Free Activation (sponsored)**; the deposit is **non-refundable**, tops up from gas refunds, and can run down / need re-activation. Sponsorship is capped per wallet, depends on Vela's per-chain treasury, and is **never offered on custom/test networks** (server-gated on nonce ≤ 3 + WebAuthn registration + treasury balance). Source: [src/services/bundler-service.ts](../src/services/bundler-service.ts):118-227.

**19. Battle card vs named competitors.** The homepage compares Vela against **MetaMask, Rabby, Base Account, and Clave** on: where the signing key lives, key ever exposed to the app, fully open source (app + backend), **self-host the bundler & services (Yes — uniquely)**, audited Safe vs proprietary account, survives vendor shutdown, no NFT/DeFi bloat, no seed phrase. Source: `+page.svelte`:858-937. Ready-made comparison-page content.

**20. Self-host every service, validated on entry.** Configure custom endpoints in Settings → Advanced. The wallet validates a candidate before accepting it: HTTPS only, reachable within 10s, and `/api/health` returns the correct `service` id + `status:"ok"`. Source: [README.md](../README.md), [src/services/network-checker.ts](../src/services/network-checker.ts). Services: Chain Data Index (`atshelchin/ethereum-data`), Passkey Index, Bundler (`atshelchin/vela-bundler`), plus FX = Frankfurter.

**21. WebAuthn proxy extension for domain-loss disaster recovery.** WebAuthn binds passkeys to the rpId domain `getvela.app`. The included **open-source Chrome extension** (`chrome-ext-webauthn-proxy/`, v1.3.0) proxies `navigator.credentials` so localhost/preview domains share the `getvela.app` rpId — both a dev tool and a "if the domain ever changes" escape hatch. Source: [README.md](../README.md):163-219, extension `manifest.json`. Pre-empts the "what if you shut down" objection.

**22. rpId resolution keeps passkeys consistent across subdomains.** Native uses `getvela.app`; web reduces to the registrable domain so `wallet.getvela.app` and `getvela.app` share passkeys. The proxy extension can override via `window.__VELA_WEBAUTHN_PROXY_RPID__`. Source: [src/modules/passkey/index.ts](../src/modules/passkey/index.ts):33-54.

**23. The "never unlimited approval" guard.** A descriptor-independent, **unbypassable submit-time guard** (`enforceNoUnlimited`) re-scans the outgoing request and throws on unbounded ERC-20 / Permit2 approvals — it reads raw calldata/typed-data, not a resolved descriptor, precisely because descriptor lookup is what fails on novel drainer contracts. The UI offers no "Max/Unlimited" preset and lets you cap an approval to a finite amount. Source: [src/services/approval-guard.ts](../src/services/approval-guard.ts), [use-dapp-signing.ts](../src/hooks/use-dapp-signing.ts):322,367. **Keywords:** unlimited approval protection, token approval safety, revoke approvals.

**24. Asymmetric trust model in simulation (the standout security idea).** Simulation logs are unauthenticated, so a hostile contract can emit a fake `Transfer(_, you, big)` and spoof a green "+1,000,000 USDC." Vela renders a *received* amount with confidence **only** if the token is curated/known, a chain stable/wrapped, or one you already hold; otherwise it degrades to "unverified" (direction + caution, no attacker-controlled amount). An *outflow* can't be understated, so sent amounts always render. Source: [src/services/tx-simulation.ts](../src/services/tx-simulation.ts):257.

**25. Transaction simulation with a 3-tier engine cascade.** `simulateAssetChanges` tries (1) `eth_simulateV1` via the user's own RPC, (2) an optional local Tevm fork, (3) a single `eth_call` revert pre-check — no third-party simulation service. Drives a **balance-change preview** before signing; `null` always means "no info," never a false "will fail." Source: [src/services/tx-simulation.ts](../src/services/tx-simulation.ts):162. **Keywords:** transaction preview, simulate before sign, what will this transaction do.

**26. Recipient identity resolution — on-chain, no third-party name API.** Resolves a recipient address to a human name across name services in parallel, first match by priority: Vela Passkey Index → `.bnb` (BSC) → `.arb` (Arbitrum) → `.g` (Gravity) → Basename (Base, ENSIP-19) → ENS (mainnet). All via direct RPC; only positive results cached (24h). Source: [src/services/recipient-identity.ts](../src/services/recipient-identity.ts).

**27. Recipient risk checks defend against address poisoning.** Two cheap on-chain signals before you send: **first-interaction** (no prior outgoing tx to this address → counters look-alike poisoning) and **is-contract** (`eth_getCode` → catches sending to the token contract itself). Best-effort: unreachable → no false alarm. Source: [src/services/recipient-risk.ts](../src/services/recipient-risk.ts).

**28. On-chain pricing — no third-party price API.** All crypto USD prices are derived on-chain: DEX swap quotes first, then Chainlink feeds, then `null`. No CoinGecko/CMC/Moralis. Source: [src/services/price-service.ts](../src/services/price-service.ts):1-11, [src/services/wallet-api.ts](../src/services/wallet-api.ts):364-406. **Keywords:** on-chain prices, no price API, decentralized pricing.

**29. Resilient multi-source RPC pool with auto-failover.** Each chain auto-discovers ~15–25 RPC endpoints, scored by source tier + measured latency + reliability, with automatic failover, exponential-cooldown banning, and self-healing (if everything is banned, it clears and rebuilds). No single hardcoded RPC. Source: [src/services/rpc-pool.ts](../src/services/rpc-pool.ts).

**30. Privacy: no accounts, no email, no KYC, no tracking.** Servers store only your passkey public key + chosen account name (published on-chain on Gnosis by design). The **app ships with zero analytics/crash/tracking SDK**, `NSPrivacyTracking=false`, and the only app permission is **Camera (QR)**. The website uses cookieless self-hosted analytics (Rybbit). Source: whitepaper.md, `docs/store-submission/privacy-and-review.md`, [package.json](../package.json). **Keywords:** no KYC wallet, privacy wallet, no tracking.

**31. One passkey per wallet (current constraint), backup is the OS sync.** A second backup passkey is not yet supported (a signer-module constraint); the redundancy is iCloud/Google passkey sync. Source: `+page.svelte`:1191-1198. State this honestly in recovery content.

**32. dApp Connect uses WalletPair over a WebSocket relay — NOT Bluetooth.** Pairing is a relay URI carried in a QR code; the channel is end-to-end encrypted with out-of-band fingerprint verification. Source: [src/services/walletpair-transport.ts](../src/services/walletpair-transport.ts):5-9, 288-320. **Correct any "Bluetooth pairing" claim.**

**33. SIWE phishing defense via domain-binding.** The Sign-In-With-Ethereum (EIP-4361) parser is conservative (requires the canonical anchor line, rejects userinfo spoofs like `uniswap.org@evil.com`), and `checkSiweDomainBinding` compares the message domain to the request origin → green "Sign in to {domain}" on match, **danger banner on mismatch.** Source: [src/services/siwe.ts](../src/services/siwe.ts), [SigningRequestModal.tsx](../src/components/SigningRequestModal.tsx):1019-1061.

**34. EIP-5792 batch transactions (`wallet_sendCalls`).** Multiple calls batch into a **single Safe `multiSend` UserOp** (one atomic transaction, one signature). The returned batch id equals the userOpHash for receipt lookup. Source: [use-dapp-signing.ts](../src/hooks/use-dapp-signing.ts):341-435, [safe-transaction.ts](../src/services/safe-transaction.ts):172-191. **Keywords:** batch transactions, atomic transactions, EIP-5792.

**35. Business model: web free, mobile paid, "paying for convenience, not access."** Web wallet is free; the mobile app is a paid download (region-priced) funding a small independent team — but it's open source so you can build from source for free. Source: `+page.svelte`:1073-1116. Honest monetization narrative for pricing pages.

---

## Tier 3 — Technical Architecture Depth · clues 36–65
*Developer SEO, "how it's built" deep-dives, integration guides. High keyword value for technical search.*

**36. Canonical contract set, identical on every chain.** One deployment used on all chains: SafeProxyFactory `0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67`, Safe Singleton `0x29fcB43b46531BcA003ddC8FCB67FFE91900C762`, FallbackHandler `0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99`, EntryPoint `0x0000000071727De22E5E9d8BAf0edAc6f37da032`, Safe4337Module `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226`, SafeModuleSetup `0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47`, WebAuthn shared signer `0x94a4F6affBd8975951142c3999aEAB7ecee555c2`, MultiSend `0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526`. Source: [src/services/safe-address.ts](../src/services/safe-address.ts):21-28.

**37. EntryPoint is ERC-4337 v0.7.** Confirmed by both the address above and [deployer-api.ts](../src/services/deployer-api.ts):140. UserOps use the v0.7 split form (`factory` + `factoryData`, `verificationGasLimit`/`callGasLimit` as uint128). **Keywords:** EntryPoint v0.7, ERC-4337 v0.7.

**38. CREATE2 salt is derived purely from the P-256 public key.** `saltNonce = keccak256(abi.encode(x, y))`; the outer salt is `keccak256(abi.encode(keccak256(setupData), saltNonce))`; `initCodeHash = keccak256(proxyCreationCode ++ abi.encode(singleton))`. No chainId → same address everywhere. Golden cross-platform vector locks parity with iOS Swift + Android Kotlin. Source: [src/services/safe-address.ts](../src/services/safe-address.ts):75-208, `safe-address.test.ts`.

**39. Counterfactual deployment is bundled into the first transaction.** When not yet deployed, `buildInitCode` prepends `SafeProxyFactory.createProxyWithNonce(...)` so the first UserOp atomically deploys + executes. Undeployed ops use a 2,000,000-gas verification floor. Source: [safe-transaction.ts](../src/services/safe-transaction.ts):835-863.

**40. The signed hash is a SafeOp EIP-712 typed-data hash bound to the 4337 module.** `calculateSafeOpHash` hashes a `SafeOp(...)` struct with domain `{chainId, verifyingContract = Safe4337Module}`, not a raw userOpHash. `validAfter`/`validUntil` are 0 (no expiry window). Source: [safe-transaction.ts](../src/services/safe-transaction.ts):869-916.

**41. WebAuthn DER→raw conversion with mandatory low-s normalization.** `derSignatureToRaw` parses DER, left-pads r/s to 32 bytes, then normalizes `s` to the low half (`s ≤ n/2`) because the RIP-7212 precompile rejects high-s signatures. Source: [src/services/attestation-parser.ts](../src/services/attestation-parser.ts):51-122.

**42. On-chain signature is a Safe "contract signature" wrapping ABI-encoded WebAuthn fields.** Layout: `validAfter(6) ++ validUntil(6) ++ r(32) ++ s(32) ++ v=0x00(1) ++ dataLen(32) ++ abi.encode(authenticatorData, clientDataFields, sigR, sigS)`. Source: [safe-transaction.ts](../src/services/safe-transaction.ts):1016-1123.

**43. EIP-1271 dApp signing double-wraps as a SafeMessage.** `computeSafeMessageHash` wraps the dApp's hash: `keccak256(0x1901 ++ domainSep{chainId, verifyingContract = the Safe itself} ++ keccak256(SAFE_MSG_TYPEHASH ++ keccak256(abi.encode(originalHash))))`, and omits the 12-byte validity prefix. Source: [safe-transaction.ts](../src/services/safe-transaction.ts):941-1043.

**44. `clientDataFields` is extracted, not reconstructed.** The verifier contract templates the JSON, so Vela takes only the trailing fields after the challenge (e.g. `"origin":"https://getvela.app","crossOrigin":false`). Source: [safe-transaction.ts](../src/services/safe-transaction.ts):982-1007.

**45. On-chain P-256 verification via the RIP-7212 precompile.** The WebAuthn signer is configured with `verifiers = 0x100`, selecting the RIP-7212 P-256 precompile. Adding any custom network requires this precompile to exist (validated two ways). Source: [safe-address.ts](../src/services/safe-address.ts):120, [network-checker.ts](../src/services/network-checker.ts):191-212. **Keywords:** RIP-7212, P-256 precompile, on-chain passkey verification.

**46. Hand-rolled crypto shared across iOS / Android / Web.** Bespoke Keccak-256 (0x01 padding, not SHA-3's 0x06) and a minimal CBOR parser extract the COSE P-256 key (`{1:2, 3:-7, -1:1, -2:x, -3:y}`) from the attestation — no native crypto dependency, identical address derivation on three platforms, locked by golden test vectors. Source: [eth-crypto.ts](../src/services/eth-crypto.ts):110-146, [attestation-parser.ts](../src/services/attestation-parser.ts):23-45.

**47. Counterfactual accounts answer `eth_getCode` with real runtime bytecode.** `SAFE_PROXY_RUNTIME_CODE` is sliced at runtime from the proxy creation code so a not-yet-deployed Vela account reports non-empty code — making dApps detect EIP-1271 smart-wallet behavior instead of an EOA. WalletPair advertises this bytecode to the dApp side too. Source: [safe-address.ts](../src/services/safe-address.ts):34-54, [walletpair-transport.ts](../src/services/walletpair-transport.ts):86-130.

**48. Incompatible passkey providers are rejected at registration.** Providers that reorder `clientDataJSON` fields (the code names **Xiaomi Password Manager**) are detected by a strict prefix check and rejected with a non-retryable `PasskeyIncompatibleError` before anything is saved. Source: [public-key-upload.ts](../src/services/public-key-upload.ts):36-59, [webauthn-verify.ts](../src/services/webauthn-verify.ts):29-65.

**49. Public-key upload is idempotent; the server signs the on-chain index tx.** Upload uses verify-after-write (server record is source of truth, byte-for-byte checked), an `Idempotency-Key`, and silent retry on launch with no biometric prompt. The Passkey Index stores keys on **Gnosis Chain** and signs the storing transaction itself (no client challenge/signature needed). Source: [public-key-upload.ts](../src/services/public-key-upload.ts):69-147, [public-key-index.ts](../src/services/public-key-index.ts):1-6.

**50. Gas estimation inflates 1.5× with hard floors and refuses doomed ops.** Estimated limits are multiplied with floors (300k deployed / 2M undeployed verification, 100k call, +10k preVerificationGas). If estimation fails and calldata > 1024 bytes, the send is **refused** (retryable) rather than submitting an op likely to silently fail. Static L2 bumps: +600k for Arbitrum, +150k for OP-Stack. Source: [safe-transaction.ts](../src/services/safe-transaction.ts):421-546.

**51. Named gas tiers + bundler as price oracle.** Tiers: slow ×1.1, standard ×1.2, rapid ×1.5, fast ×2.0. Authoritative pricing comes from `pimlico_getUserOperationGasPrice`; Vela-specific `networkFeePerGas`/`relayerFeePerGas` fields are read when present. Source: [safe-transaction.ts](../src/services/safe-transaction.ts):225-230,1336-1381.

**52. Optimistic nonce caching + "already pending" recovery.** Nonce is cached 10s and optimistically incremented to prevent concurrent-send collisions; if the bundler rejects because a prior op is in-flight, Vela extracts the existing hash and polls that op's receipt instead of failing. Source: [safe-transaction.ts](../src/services/safe-transaction.ts):585-597,1226-1233.

**53. Receipt polling distinguishes "unconfirmed" from "bundler unreachable."** Adaptive 1s→3s backoff, 120s default timeout, `success===false` treated as terminal "dropped," and different messages depending on whether the bundler was ever reachable — never implying failure for an op that may still land. Source: [safe-transaction.ts](../src/services/safe-transaction.ts):1459-1532.

**54. Tempo: a stablecoin-gas chain on the same Safe + passkey stack.** Tempo (chainId 4217) has no native coin; gas is paid in TIP-20 stablecoins (default `pathUSD` at the reserved `0x20c0…0000`, 6 decimals). Vela signs the UserOp with `maxFeePerGas = 0` and batches a `pathUSD.transfer(bundlerEOA, reimbursement)` (priced ~2× realistic gas) into the MultiSend; the bundler submits `handleOps` inside a native Tempo tx paying gas in the fee token. Same EntryPoint + Safe + passkey + cross-chain address consistency. Source: [src/services/tempo.ts](../src/services/tempo.ts), [safe-transaction.ts](../src/services/safe-transaction.ts):626-737. **Keywords:** pay gas in stablecoin, gasless-feeling, Tempo chain.

**55. RPC pool — six-tier source priority + EMA latency scoring.** Endpoints are ranked `user > provider (Alchemy/dRPC/Ankr) > default > public > builtin > fallback`, blended with a measured-latency penalty (EMA `0.7·avg + 0.3·new`), reliability bonus, and exponential cooldown on failure. Measured latency wins after warm-up. Source: [rpc-pool.ts](../src/services/rpc-pool.ts):255-508.

**56. RPC pool — two-tier banning, self-heal, getLogs range-cap handling.** Temporary bans (rate-limit/401/403, 1h) vs permanent (0 successes + ≥6 failures, auto-expires 24h), persisted to AsyncStorage. `eth_getLogs` range-cap errors are parsed and **returned** (so the caller splits the range), never failed-over or banned. Source: [rpc-pool.ts](../src/services/rpc-pool.ts):63-488.

**57. Bundler calls pick the fastest RPC and forward it.** `poolBundlerCall` races all endpoints with a 3s `eth_chainId` ping, caches the winner per chain (1h), and passes it to the Vela bundler via an `X-Rpc-Url` header so the bundler reaches the chain correctly (critical for Tempo's in-band reimbursement to the right EOA). Source: [rpc-pool.ts](../src/services/rpc-pool.ts):587-630,747-870.

**58. Adding a custom network requires the full contract suite + precompile.** The wallet checks (via the fastest reachable HTTPS RPC) for all 11 contracts — Deterministic Deployment Proxy, Safe Singleton Factory, **Multicall3** (`0xcA11bde05977b3631167028862bE2a173976CA11`), EntryPoint v0.7, Safe L2, Proxy Factory, 4337 Module, Module Setup, WebAuthn Signer, Fallback Handler, MultiSend — **and** the RIP-7212 precompile, before persisting the network. Source: [network-checker.ts](../src/services/network-checker.ts):20-32, [add-network.ts](../src/services/add-network.ts):42-53. A companion "Chain Setup" tool can deploy these on chains that lack them.

**59. Read-only dApp RPC gate (defense-in-depth).** At most 6 concurrent reads, up to 512 queued, identical concurrent keys collapse to one execution (never cached across time); excess rejected with retryable `-32005`. **Signing requests bypass the gate** so a read-flood can't starve a user's confirmation. Source: [src/services/readonly-rpc-gate.ts](../src/services/readonly-rpc-gate.ts).

**60. One Multicall3 `eth_call` per chain for the whole portfolio.** Balances + `decimals` + per-token DEX quotes + the chain's Chainlink native/USD feed are packed into a single `aggregate3` call, bounded per chain by an 18s `Promise.race`. Token-metadata batches 40 tokens/call. Source: [wallet-api.ts](../src/services/wallet-api.ts):251-518.

**61. DEX pricing supports two protocol families with hardcoded per-chain contracts.** Uniswap-V3-style (`QuoterV2.quoteExactInputSingle`, fee tiers 500/3000/2500/10000) on most chains; PancakeSwap V3 on BSC; Aerodrome solidly router (`getAmountsOut`, volatile+stable) on Base; SushiSwap V3 on Gnosis. Built-in DEX overrides beat the remote API. Source: [chain-tokens.ts](../src/services/chain-tokens.ts):48-104, [wallet-api.ts](../src/services/wallet-api.ts):514-526.

**62. Native price uses a 3-tier fallback with a Chainlink sanity guard.** DEX price is preferred only if within 0.5×–2.0× of Chainlink (a >50% deviation is treated as low-liquidity and discarded); fallback order DEX → on-chain Chainlink → Ethereum-mainnet Chainlink. Stablecoins are hard-pegged to $1. Source: [wallet-api.ts](../src/services/wallet-api.ts):385-447.

**63. Per-chain Chainlink native/USD feeds are hardcoded (8-decimal answer).** ETH/BNB/AVAX/DAI feeds per chain; Polygon deliberately omitted (MATIC→POL migration broke its feed; DEX covers it). Source: [wallet-api.ts](../src/services/wallet-api.ts):40-49.

**64. Received-transfer detection is client-side log polling + EIP-7708, with a spam allowlist.** One `eth_getLogs` per chain on the ERC-20 `Transfer` topic with the wallet as recipient; EIP-7708 means native ETH transfers emit the same event, so one query catches both (native recognized by sentinel emitters). Filtered to a per-chain **allowlist** (known stablecoins + user-added tokens + native sentinels) so airdrop spam can't slip in; logs are re-validated client-side. Source: [src/services/transfer-monitor.ts](../src/services/transfer-monitor.ts):38-221. **Keywords:** deposit detection, incoming transfer notification.

**65. 7-day balance history via on-chain block-time estimation + archive-RPC discovery.** Estimates avg block time by sampling `latest` vs `latest−1000`, converts past midnights to block numbers, and auto-discovers an archive-capable RPC by test-querying ~1-day-old balances. Each point validated within ±1h of target. Source: [src/services/balance-history.ts](../src/services/balance-history.ts).

---

## Tier 4 — Clear-Signing, Decoding, Fiat & UX Depth · clues 66–85
*Feature deep-dives, security explainers, design/UX content, localization SEO.*

**66. ERC-7730 descriptor cascade (richest-first).** For `eth_sendTransaction`: built-in local descriptor → contract-specific ERC-7730 (`/erc7730/calldata/eip155-{chainId}/{to}.json`) → ERC-165-disambiguated standard token methods → ERC fallbacks (erc20/721/4626) → 4-byte best-effort. Only if all fail does it blind-sign. Source: [clear-signing.ts](../src/services/clear-signing.ts):293.

**67. 4-byte selector registry merges three databases.** Sourcify 4byte + OpenChain (spam-filtered, most-likely-first) lead, then `4byte.directory` (canonical order) fills gaps, 6s timeout. Unknown calls decode as `bestEffort:true` with `risk:'caution'` and raw params hidden under "Advanced — view raw data." Source: [src/services/selector-registry.ts](../src/services/selector-registry.ts).

**68. ERC-165 standard detection avoids amount/tokenId confusion.** `transferFrom`/`approve` selectors collide between ERC-20 and ERC-721; `detectTokenStandard` probes `supportsInterface(0x80ac58cd)` / `0xd9b67a26` in parallel and caches only *definitive* verdicts (a transient RPC `null` re-probes next time). Source: [clear-signing.ts](../src/services/clear-signing.ts):187-208.

**69. Token decimals are always fetched on-chain, never assumed 18.** A 6-decimal token rendered as 18 would overstate an amount by 1e12 on a security surface. Unresolved decimals fall back to 18 but flag `unverified`, flooring risk at caution. BigInt math used because `10**decimals` as a JS number loses precision past ~23 decimals. Source: [clear-signing.ts](../src/services/clear-signing.ts):365,1179.

**70. No "Max/Unlimited" preset anywhere in approval UI.** `EditableApproveCard` forces an unbounded incoming request into custom mode with the confirm button disabled until a finite amount is chosen; boolean grants (`setApprovalForAll`, DAI permit) render a danger card defaulting to no selection (a deliberate "Grant all anyway" tap). Source: [src/components/EditableApproveCard.tsx](../src/components/EditableApproveCard.tsx). Approval re-encoding is byte-surgical and self-verifying (`assertOnlyWordChanged`).

**71. Layered risk scoring that floors uncertainty at caution.** Any warning field (unlimited approval) → danger; intent base (approve/permit → caution, stake/deposit/claim → safe); then any partial/unverified/expired field floors a safe/normal result at caution. Uncertainty can never read as "safe." Source: [clear-signing.ts](../src/services/clear-signing.ts):1236.

**72. Dependency-free ABI decoder handling nested dynamic types.** Parses Solidity signatures (tuples/arrays), recomputes selectors via keccak256, and decodes with correct relative-offset handling and negative-index/byte-slice path resolution (`path.-1`, `params.path[-20:]`) needed to read Uniswap V2/V3 swap routes. Caps dynamic arrays at 200 elements. Source: [src/services/abi-decode.ts](../src/services/abi-decode.ts), [clear-signing.ts](../src/services/clear-signing.ts):826.

**73. Revert reasons decoded from `Error(string)` and `Panic(uint256)`.** Bounds-checked length parsing; a bare "execution reverted" is suppressed (UI already says "expected to fail"). Source: [src/services/sim-assets.ts](../src/services/sim-assets.ts):181.

**74. Contract deployment renders calmly, not as a scary "Unknown."** A deploy shows as "Deploy contract" with a predicted CREATE2 address for the Arachnid deployer `0x4e59…956c`; "no expiry" Permit2 sentinels are omitted (not "Invalid Date"); past deadlines render `(expired)`. Source: [clear-signing.ts](../src/services/clear-signing.ts):128.

**75. Fiat conversion via a configurable, no-API-key FX endpoint.** Default Frankfurter v2 (`?base=USD`, FOSS, Docker-self-hostable, ~160 currencies incl. VND). Contract: `rate(code)` = units per 1 USD, `displayed = usdAmount × rate`. The selectable-currency list is **data-driven** — whatever the endpoint returns "just appears." Cache keyed by endpoint URL (swap provider → instant refetch). Source: [src/services/fiat-fx.ts](../src/services/fiat-fx.ts), [src/services/currency.ts](../src/services/currency.ts), `docs/fiat-price.md`.

**76. Optional decentralized fiat rates via Chainlink ENS-addressed feeds.** Resolves `<ccy>-usd.data.eth` through the ENS registry on mainnet, reads `latestRoundData()`; 16 codes (EUR, GBP, JPY, CNY, AUD, …). Per-feed `decimals()` read (PHP uses 18, not 8). Source: [src/services/fiat-rates.ts](../src/services/fiat-rates.ts).

**77. Display formatting drops decimals for large sums and zero-decimal currencies.** No cents when `|value| ≥ 100,000` or for JPY/KRW/IDR/VND/etc. — e.g. `¥259,770`. Source: [currency.ts](../src/services/currency.ts):98-100.

**78. Robust on-chain token metadata.** Resolves `symbol()`/`decimals()` via 3-layer cache (static known-tokens → memory → AsyncStorage → on-chain), decodes legacy `bytes32` symbols (MKR) and multibyte UTF-8 ("USD₮0"); negative lookups are session-only. Exists because an unknown 6-decimal stablecoin once rendered as "+0 tokens." Source: [src/services/token-metadata.ts](../src/services/token-metadata.ts), [src/services/abi.ts](../src/services/abi.ts):268-307.

**79. `AmountText` atomic-number display (Apple Wallet / Cash App-style).** Renders money as one never-wrapping unit via a 3-step cascade: fit-to-width shrink → compact-notation floor (`$1,234,567.89 → $1.23M` once shrinking would go illegible) → two-tier typography (large integer head, subordinated decimal tail). Width is *estimated* (`width / (len·0.6em)`), not left to flaky `adjustsFontSizeToFit`. Source: [src/components/ui/AmountText.tsx](../src/components/ui/AmountText.tsx), `docs/dynamic-amount-display.md`. **Keywords:** balance display, responsive currency text.

**80. Intl-free locale formatting (works around Hermes' incomplete ICU).** Explicit presets: 4 number formats (comma_dot, dot_comma, space_comma, indian — real lakh/crore grouping), 5 date, 2 time, each with an `auto` device-detection path. Compact suffixes use universal K/M/B/T (CJK myriad avoided on purpose). Source: [src/services/locale-format.ts](../src/services/locale-format.ts), `docs/localization.md`.

**81. 15 UI locales, ~857 keys, near-100% parity, type-checked.** en, zh, zh-TW, zh-HK, ja, ko, vi, id, tr, ru, es-MX, pt-BR, fr, it, de. zh / zh-TW / zh-HK maintained as genuinely distinct. Keys are TypeScript-typed against English (`typeof en`) so a bad `t()` key is a compile error. Language switching is restart-free (provider above a keyed `<Stack>`). Source: [src/i18n/resources.ts](../src/i18n/resources.ts), `i18next.d.ts`. **Keywords:** multilingual wallet, [language] crypto wallet (×15).

**82. A single platform-abstraction seam.** [src/services/platform.ts](../src/services/platform.ts) is the one file branching on `Platform.OS` (alert, clipboard, openURL, browser, app-active), so screens never scatter platform checks; web degrades gracefully (in-app modal alert, `navigator.clipboard`, `window.open`). **Keywords:** cross-platform wallet, one codebase iOS Android web.

**83. Plus Jakarta Sans typeface + a 3-second font boot gate.** The theme loads Plus Jakarta Sans (weights 400/500/600/700) though the export is still named `inter`; a hung font fetch releases the splash after 3000ms because "a wallet must always boot." Mono = Menlo/monospace. Source: [src/constants/theme.ts](../src/constants/theme.ts), [src/app/_layout.tsx](../src/app/_layout.tsx).

**84. Theme = mutable design tokens, never key-remounted; 6-level text scale; WCAG palette.** Tokens (4px-grid `space`, `text`, `radius`, `color`, `shadow`, `motion`) are rebuilt in place and gated by a `_styleVersion` (no flicker/state-loss). Text scale: 0.82×–1.35× across 6 levels. Palette documents exact contrast remediations (e.g. light `fg.muted #6E6B62` ≥4.5:1); single accent `#E8572A`. Design language = "depth through shadow, not glass" (no BlurView anywhere). Source: [src/constants/theme.ts](../src/constants/theme.ts), [src/constants/text-scale.ts](../src/constants/text-scale.ts), `docs/text-scale-architecture.md`.

**85. Bespoke "big-tech" UI primitives.** Custom PanResponder/Reanimated components built to work identically on Expo web: `SlideToConfirmButton`, `VelaRefresh` (branded pull-to-refresh that "draws" an accent arc with the pull), `WaveDock` (home action bar with a concave wave cradle + emerging Scan FAB), and `AppModal` (three implementations: native pageSheet on iOS, custom drag-to-dismiss on Android, CSS slide-up portal on web). Five semantic haptics, no-op on web. Source: [src/components/ui/](../src/components/ui/).

---

## Tier 5 — Connection Activity, Process, Meta & Niche · clues 86–100
*Power-user features, "how it's built" credibility, founder story, and content-ops notes.*

**86. dApp transactions are persisted as `pending` at submit-time, before the receipt wait.** The moment the bundler accepts an op, a `pending` record is written (then patched in-place to confirmed/failed) so nothing is lost on sheet-close or reload; a mount-time effect re-polls any still-pending op newer than 24h. Source: [src/models/dapp-connection.tsx](../src/models/dapp-connection.tsx):497-543,715-734.

**87. Every approved dApp operation is a replayable history record, rendered by the *live* signing sheet.** `dapp-history` captures the original request (bounded to 24KB, progressively clipping calldata) plus the sign-time simulation; the Connections panel replays it through the same `<SigningSheet>` in read-only mode (intent, fields, "what moved") — no re-simulation. Source: [src/services/dapp-history.ts](../src/services/dapp-history.ts), [src/components/ui/SigningReplaySheet.tsx](../src/components/ui/SigningReplaySheet.tsx).

**88. WalletPair advertises rich capabilities to the dApp.** At session creation it declares signing + 22 read-only RPC methods, `accountsChanged`/`chainChanged`/`disconnect` events, **per-chain EIP-5792 `atomic: supported`**, the RPC URLs for every configured network, and the Safe proxy runtime bytecode (so the dApp can answer `eth_getCode` for a counterfactual account). A method map translates WalletPair object-params ↔ Ethereum JSON-RPC array-params. Source: [walletpair-transport.ts](../src/services/walletpair-transport.ts):54-202.

**89. Mobile-aware connection resilience.** WebSocket heartbeat pings every 25s (tuned to Cloudflare's 30s idle timeout); on app-foreground after ≥20s background, it forces `session.reconnect()`; bounded deadlines (`CONFIRM_JOIN 30s`, `RECONNECT_MAX 60s`, provider `reconnectStuck 45s`) stop the UI hanging on a silent relay. Source: [walletpair-transport.ts](../src/services/walletpair-transport.ts):513-587.

**90. Single-session model with MITM-resistant pairing.** Exactly one dApp connected at a time; pairing requires visual **fingerprint** confirmation before joining the E2E-encrypted channel (connected card shows a green "E2E" lock badge). Sessions auto-restore on mount (remote-inject first, then WalletPair, signed snapshot). Source: [dapp-connection.tsx](../src/models/dapp-connection.tsx), [src/components/ConnectionFlowStates.tsx](../src/components/ConnectionFlowStates.tsx).

**91. getvela.app tech stack + SEO scaffolding.** Marketing/docs site = **SvelteKit + Svelte 5 + TypeScript (bun)**, deployed on **Cloudflare**, with schema.org Organization + WebSite JSON-LD binding the "Vela Wallet" entity to the domain (for branded sitelinks). Accent `#e8572a` on near-black `#0f0e0c`. The wallet app is React Native + Expo Router, web built to `dist/` → Cloudflare Pages. Source: `getvela.app/README.md`, `+page.svelte`.

**92. Engineering culture (substantiates "built carefully, in the open").** Codified rules in `agent-rules/`: AI-coding accountability ("AI can produce code, but not accountability") with a Low/Medium/High risk workflow where any crypto/key/auth change is auto-High; a red-team security-audit prompt ("prove this code is unsafe until you can't"); a pre-launch hardening rulebook; and a build→test→fix loop with 30-cycle stability verification. Source: `agent-rules/AI-CODING-RULES.md`, `SECURITY-AUDIT.md`, `LAUNCH_AUDIT.md`, `CLAUDE-AUTO-TEST.md`.

**93. Solo founder, fully in the open.** One person — **"Shelchin," Founder & Engineer** (GitHub `atshelchin`, X `@atshelchin`) — builds the wallet, the contracts, and the site. Legal entity **MONDAY LABS LTD (UK)**; brand socials X `@realvelawallet`, Telegram `t.me/velawallet`; repo `github.com/mondaylabsltd/vela-wallet`. Source: `about/+page.svelte`, `seo.ts`. Useful for schema.org Organization markup and indie-founder angle.

**94. The origin story (quotable for About/PR/launch posts).** Vela was born from "Where are you supposed to keep twelve words?" and from daily use of **Base Account** hitting its limits (a browser-generated recovery key you "just had to trust," no custom networks, no self-hosting, "if the service disappeared, the wallet disappeared with it"). Source: `+page.svelte`:792-855.

**95. Built-in resilience tooling, no telemetry backend.** A fault-injection harness exposes `vela.*` web-console commands (simulate RPC down/slow/null-price) to validate failure-state UX; in-memory `metrics.ts` counters + a 25-entry failure ring buffer attach (sanitized — strips keys/sigs/calldata) to one-click bug reports. No data leaves the device unless the user files a report. Source: [src/services/metrics.ts](../src/services/metrics.ts), `src/services/dev/fault-injection`.

**96. One-click bug report with diagnostics.** An in-app report button posts to `getvela.app/api/bug-report` (PAT server-side only) with sanitized metrics + failed-chain context, falling back to a prefilled GitHub issue URL. Source: [src/services/bug-report.ts](../src/services/bug-report.ts), [src/services/feedback.ts](../src/services/feedback.ts).

**97. Cross-repo coupling: bundler "underfunded" detection.** `parseBundlerUnderfunded` matches the bundler's error by stable signal (e.g. `/dedicated bundler (gas account|EOA)/` + `Deposit to:` + `required:`), not exact wording, and must stay in sync with the `vela-bundler` repo's handlers. Drives the gas-account top-up modal. Source: [bundler-service.ts](../src/services/bundler-service.ts):367-385.

**98. Advanced send modes: split & sweep.** Split (1 token → N recipients) and sweep (N tokens → 1 address) are mutually exclusive, each executed as one MultiSend UserOp via `sendBatchCalls`; max-send reserves native gas (EntryPoint prefund) so a sweep can't revert with AA21. Source: [src/services/batch-send.ts](../src/services/batch-send.ts), [src/components/MultiRecipientEditor.tsx](../src/components/MultiRecipientEditor.tsx).

**99. Contacts & known-recipient UX.** On-device contacts with avatars and a "known contact" badge surfaced during send/signing, reducing wrong-address risk. Source: [src/components/contacts/](../src/components/contacts/), [src/services/contacts.ts](../src/services/contacts.ts).

**100. Content-ops note — single source of truth for tokens & duplication.** `KNOWN_TOKENS` ([src/services/tokens.ts](../src/services/tokens.ts)) is the consolidated source for well-known ERC-20 symbol/decimals (previously duplicated in three places); explorer-URL, wei→ETH, and address-validation logic were likewise consolidated. When writing dev docs, point readers at these canonical modules rather than the old scattered copies.

---

## Content Mapping (Clue Clusters → Content Pieces)

Each cluster below is a writeable document. Pull facts only from the cited clues.

| # | Content piece | Type | Audience | Primary clues | Target keywords |
|---|---------------|------|----------|---------------|-----------------|
| A | **What is Vela Wallet?** (pillar) | SEO pillar | All | 1–4, 8, 11, 13–15 | seedless wallet, smart contract wallet, self-custodial |
| B | **No seed phrase: how passkey wallets work** | SEO explainer | Crypto-curious | 1, 3, 5, 7, 31, 46 | no seed phrase, passkey wallet, WebAuthn wallet |
| C | **Vela vs MetaMask / Rabby / Base Account / Clave** | Comparison | Evaluators | 19, 11, 13, 32, 30 | metamask alternative, rabby alternative, best self-custody wallet |
| D | **Your keys, our architecture can't touch them** | Security/trust | Skeptics | 3, 16, 10, 30, 92 | non-custodial proof, verify don't trust |
| E | **Recovery without a seed phrase (and its honest limits)** | Guide | Users | 7, 31, 21, 22 | wallet recovery, icloud passkey, lost device crypto |
| F | **Fees & gas explained** | Guide | Users | 17, 18, 54, 50–53 | crypto wallet fees, account abstraction gas, pay gas in stablecoin |
| G | **Clear signing: never blind-sign again** | Security feature | Power users | 12, 23, 24, 25, 66–74 | clear signing, no blind signing, unlimited approval protection |
| H | **Build on Vela: the AA + WebAuthn architecture** | Dev deep-dive | Developers | 4, 36–49, 58 | ERC-4337 v0.7, Safe 4337 module, RIP-7212, counterfactual address |
| I | **Self-host everything** | Dev/ops guide | Self-hosters | 11, 20, 55–58, 75 | self-hosted wallet, run your own bundler, ethereum-data index |
| J | **Multi-chain done right: RPC resilience & pricing** | Dev deep-dive | Developers | 28, 29, 55–65 | on-chain pricing, RPC failover, Multicall3 portfolio |
| K | **Connect a dApp with Vela (WalletPair)** | Guide | Users/devs | 32, 33, 34, 88–90 | walletconnect alternative, dApp signing, EIP-5792 batch |
| L | **Designed for everyone: cross-platform + 15 languages** | Design/i18n | Users | 79–85, 82 | multilingual wallet, [lang] crypto wallet, accessible wallet |
| M | **The Vela story / about** | Brand | All | 8, 9, 93, 94 | indie wallet, open source wallet founder |
| N | **Why a wallet that does less is safer** | Thought leadership | All | 13, 24, 30, 92 | minimal wallet, attack surface, secure by design |
| O | **Supported networks (incl. Tempo, Monad, World Chain, Unichain)** | Reference/pSEO | SEO long-tail | 14, 54, 58, 63 | [chain] wallet, stablecoin gas wallet |

### Reusable canonical assets
- **Taglines:** "Your keys. Your face." · "A wallet that does less — on purpose." · "We can't access your keys. Not 'we promise not to' — we architecturally can't." · "You're paying for convenience, not access." · "Don't trust us — verify."
- **One-line ICP:** *"For people who want real self-custody without the footgun of seed-phrase management — if you can unlock your phone, you can use Vela safely."* (`docs/introduction.md`)
- **High-value technical keyword set:** Safe smart account, ERC-4337 (EntryPoint v0.7), WebAuthn passkey, P-256 / RIP-7212, EIP-1271, ERC-7730 clear signing, EIP-5792 batch calls, counterfactual / CREATE2 address, EIP-7708.

---

*Generated 2026-06-30 from a full read of the vela-wallet source + getvela.app docs. Every clue cites its source file; when in doubt, re-read the source before publishing — the README is stale and must not be used as a fact source (see Guardrails).*
