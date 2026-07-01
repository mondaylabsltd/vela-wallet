# Vela Wallet — Requirements Documents (Master Index)

> **Purpose.** This directory decomposes the *entire* Vela Wallet product into **100 independent
> requirement documents (PRDs)**, ordered so that each doc's dependencies come before it
> (foundations → identity → money movement → dApp → polish → ops). Read top-to-bottom and the
> product assembles itself; read any single file and it stands alone.
>
> **Not the same as `docs/CONTENT-SOURCE-100-CLUES.md`.** That file is a *content/SEO fact bank*
> ("what to say about Vela"). These are *requirements* ("what the product must do"): goal, user
> stories, functional + non-functional requirements, acceptance criteria, source anchors. The
> clues file is cited as a fact source; it is not a substitute for a PRD.

## Can 100 documents describe Vela?

Yes — comfortably, with a little headroom. Vela is a *deliberately minimal* wallet ("a wallet that
does less — on purpose"): no NFT gallery, no swaps, no DeFi dashboard, no in-app browser. Its depth
is in **correctness of a narrow surface** (seedless passkey identity, ERC-4337 account abstraction,
clear signing, multi-chain RPC resilience), not in feature sprawl. 100 PRDs cover that surface at
roughly one requirement per meaningful behavior, and leave room for the roadmap items still landing.

## How to read

- **New here? Start with the [Reading Guide (导读指南)](./GUIDE.md)** — role-based reading paths,
  a "how does X work?" map, and which docs are high-risk. This README is the flat index; the guide
  is how to navigate it.
- Docs are grouped into **15 epics (A–O)**. IDs are stable: `<Epic><nn>` (e.g. `G04`).
- Each doc follows [`_TEMPLATE.md`](./_TEMPLATE.md). Filenames: `<ID>-<slug>.md`.
- **Every functional claim must cite a source anchor** (`file:line`). When a fact and this index
  disagree, re-read the source — the root `README.md` is **stale** (see guardrails below).

### Status legend

| | Meaning |
|---|---|
| ✅ | Shipped (in `main` / alpha build) |
| 🚧 | In progress (active branch `feat/contacts-groups-payroll-batch`, or roadmap "now") |
| 🔜 | Next (planned, roadmap "next") |
| 🧭 | Exploring (roadmap "later" — directional, not committed) |

### Accuracy guardrails (inherited from the fact bank — do not violate)

- **No Bluetooth.** dApp pairing is WalletPair over a WebSocket relay. "BLE" type names are legacy artifacts.
- **Audit posture:** Safe contracts are independently audited; **Vela's own integration is not, and none is scheduled.** Never write "audit planned."
- **No token, ever.** No airdrop, no farming.
- **Alpha, stated honestly** — no scary "tolerate bugs" banners (trust > disclaimers).
- **Fees:** ≈2× raw on-chain cost, refused above ~3× (`MAX_QUOTE_VS_CHAIN_MULTIPLE = 3n`). The README's "60% markup / 8 networks" is stale — it's **12 networks**.
- **`deployer-api.ts` is a mock.** Production bundler facts come from `bundler-service.ts`.

---

## The 100 requirements

### Epic A — Product Foundations & Cross-Cutting (6)
| ID | Title | Status | Deps |
|---|---|---|---|
| A01 | Product Vision, Scope & Non-Goals ("does less on purpose") | ✅ | — |
| A02 | Brand, Voice, Honesty & Alpha/Audit Posture | ✅ | A01 |
| A03 | Privacy, No-Token & No-Tracking Principles | ✅ | A01 |
| A04 | Cross-Platform Runtime & Platform-Abstraction Seam | ✅ | — |
| A05 | App Bootstrap, Splash & Font-Boot Gate | ✅ | A04 |
| A06 | Local Persistence & Storage Model | ✅ | A04 |

### Epic B — Identity: Passkeys & Account Model (9)
| ID | Title | Status | Deps |
|---|---|---|---|
| B01 | Seedless Thesis & WebAuthn P-256 Passkey Registration | ✅ | A04 |
| B02 | Biometric-Per-Transaction (No Persistent Unlock Session) | ✅ | B01 |
| B03 | rpId Resolution Across Native / Web / Subdomains | ✅ | B01 |
| B04 | WebAuthn Proxy Extension (Domain-Loss / Dev Passkeys) | ✅ | B03 |
| B05 | Incompatible Passkey Provider Rejection | ✅ | B01 |
| B06 | Safe v1.4.1 Account Model & Canonical Contract Set | ✅ | B01 |
| B07 | Counterfactual CREATE2 Address Derivation (Same Address Every Chain) | ✅ | B01, B06 |
| B08 | Public-Key Index: Upload + On-Chain Publish (Gnosis) | ✅ | B07 |
| B09 | Recovery Model (OS Passkey Sync) & Honest Limits | ✅ | B01 |

### Epic C — Onboarding & Wallet Lifecycle (5)
| ID | Title | Status | Deps |
|---|---|---|---|
| C01 | Welcome & Value-Prop Onboarding | ✅ | A01 |
| C02 | Create Wallet (Passkey Enrollment → Address) | ✅ | B01, B07 |
| C03 | Account Naming & On-Chain Name Publish | ✅ | B08 |
| C04 | Restore Existing Wallet via Passkey Sync | ✅ | B09 |
| C05 | Multiple Accounts & Account Switcher | ✅ | C02 |

### Epic D — Balances, Portfolio & Activity (8)
| ID | Title | Status | Deps |
|---|---|---|---|
| D01 | Home Screen IA (Activity-First) | ✅ | C02 |
| D02 | Portfolio Aggregation (Multicall3 Per Chain) | ✅ | F03 |
| D03 | Token List & Add/Remove Custom Tokens | ✅ | D04 |
| D04 | Token Metadata & Known-Tokens Registry | ✅ | F03 |
| D05 | Received-Transfer Detection (Log Polling + EIP-7708 + Allowlist) | ✅ / 🚧 | F03 |
| D06 | 7-Day Balance History (Block-Time Estimation + Archive RPC) | ✅ | F03 |
| D07 | Token Detail Screen | ✅ | D02 |
| D08 | Activity Feed & Transaction Reconciliation | ✅ | G09 |

### Epic E — Pricing & Fiat (7)
| ID | Title | Status | Deps |
|---|---|---|---|
| E01 | On-Chain USD Pricing Engine (DEX → Chainlink → null) | ✅ | F03 |
| E02 | DEX Price Adapters (Uniswap / Pancake / Aerodrome / Sushi) | ✅ | E01 |
| E03 | Native Price 3-Tier Fallback + Chainlink Sanity Guard | ✅ | E01, E04 |
| E04 | Per-Chain Chainlink Feed Registry | ✅ | F01 |
| E05 | Fiat FX Conversion (Configurable Endpoint + Optional Chainlink ENS) | ✅ | A06 |
| E06 | Display-Currency Selection & Formatting Rules | ✅ | E05 |
| E07 | Fiat-Denominated Amount Entry | 🚧 | E05, E06 |

### Epic F — Networks & RPC Infrastructure (8)
| ID | Title | Status | Deps |
|---|---|---|---|
| F01 | Supported Networks Registry (12 Chains + Custom) | ✅ | — |
| F02 | Custom Network Add — Contract-Suite + Precompile Validation | ✅ | F01, B06 |
| F03 | RPC Pool Auto-Discovery & 6-Tier Scoring | ✅ | F01 |
| F04 | RPC Failover, Banning & Self-Heal | ✅ | F03 |
| F05 | `eth_getLogs` Range-Cap Handling | ✅ | F03 |
| F06 | Read-Only dApp RPC Gate (Concurrency / Queue) | ✅ | F03 |
| F07 | Bundler RPC Selection & `X-Rpc-Url` Forwarding | ✅ | F03, G01 |
| F08 | Rate-Limit UX & Graceful Degradation | ✅ | F04 |

### Epic G — Transaction Engine (ERC-4337) (10)
| ID | Title | Status | Deps |
|---|---|---|---|
| G01 | UserOp Construction & Counterfactual Deployment (v0.7) | ✅ | B06, B07 |
| G02 | SafeOp Hashing & WebAuthn Signature Encoding | ✅ | G01, B01 |
| G03 | Gas Estimation (Inflation, Floors, Refuse-Doomed Ops) | ✅ | G01 |
| G04 | Gas Price Oracle, Tiers & Wallet↔Bundler Parity | ✅ | F07 |
| G05 | Fee Model (≈2× Cost, ~3× Cap Guard) | ✅ | G04 |
| G06 | Gas Account (Relayer EOA) & Sponsored Activation | ✅ | F07 |
| G07 | Underfunded Detection & Top-Up Modal (Cross-Repo Coupling) | ✅ | G06 |
| G08 | Nonce Caching & Already-Pending Recovery | ✅ | G01 |
| G09 | Receipt Polling (Unconfirmed vs Bundler-Unreachable) | ✅ | G01 |
| G10 | Tempo Stablecoin-Gas Transaction Path | ✅ | G01, F07 |

### Epic H — Send & Receive (9)
| ID | Title | Status | Deps |
|---|---|---|---|
| H01 | Send Flow & Max-Send (Recipient → Token → Amount → Review) | ✅ | G01, G03 |
| H02 | Recipient Identity Resolution (Name Services) | ✅ | F03 |
| H03 | Recipient Risk Checks (Address-Poisoning Defense) | ✅ | F03 |
| H04 | Address Input, QR Scan & EIP-681 Parsing | ✅ | A04 |
| H05 | Contacts (On-Device Address Book) & Known-Contact Badge | 🚧 | A06 |
| H06 | Contact Groups (e.g. Payroll) | 🚧 | H05 |
| H07 | Advanced Send: Split (1→N) & Sweep (N→1) | ✅ | G01, K07 |
| H08 | Payroll Batch Importer (Recipient Table Paste / CSV) | 🚧 | H06, H07 |
| H09 | Receive Screen, QR & Share Card | ✅ | B07 |

### Epic I — Clear Signing & Decoding (8)
| ID | Title | Status | Deps |
|---|---|---|---|
| I01 | Clear-Signing Model (Intent / Substance / Details, Risk Color) | ✅ | — |
| I02 | ERC-7730 Descriptor Cascade (Richest-First) | ✅ / 🚧 | I01 |
| I03 | Local Built-In Descriptors | ✅ | I02 |
| I04 | 4-Byte Selector Registry (3 DBs Merged) | ✅ | I01 |
| I05 | ERC-165 Standard Detection (ERC-20 vs ERC-721) | ✅ | I01 |
| I06 | Dependency-Free ABI & EIP-712 Decoding (Nested Dynamic Types) | ✅ | I01 |
| I07 | On-Chain Decimals & BigInt Amount Rendering | ✅ | D04 |
| I08 | Layered Risk Scoring (Floor Uncertainty at Caution) | ✅ | I01 |

### Epic J — Simulation & Safety Guards (5)
| ID | Title | Status | Deps |
|---|---|---|---|
| J01 | Simulation Engine Cascade (`eth_simulateV1` → Tevm → `eth_call`) | ✅ | F03 |
| J02 | Asset-Change Preview (`BalanceChangePreview`) | ✅ | J01 |
| J03 | Asymmetric Trust Model (Received vs Sent Confidence) | ✅ | J02 |
| J04 | Revert-Reason Decoding | ✅ | J01 |
| J05 | Never-Unlimited Approval Guard & Editing UX | ✅ | I06 |

### Epic K — dApp Connect (WalletPair) (8)
| ID | Title | Status | Deps |
|---|---|---|---|
| K01 | WalletPair Pairing Over WebSocket Relay (QR, No BLE) | ✅ | — |
| K02 | MITM-Resistant Fingerprint Verification & E2E Badge | ✅ | K01 |
| K03 | Single-Session Model & Auto-Restore | ✅ | K01 |
| K04 | Capability Advertisement & JSON-RPC Method Map | ✅ | K01, B07 |
| K05 | EIP-1271 Message Signing (SafeMessage Double-Wrap) | ✅ | G02 |
| K06 | SIWE Parsing & Domain-Binding Defense | ✅ | K05 |
| K07 | EIP-5792 Batch Calls (`wallet_sendCalls` → `multiSend`) | ✅ | G01 |
| K08 | Connection Resilience (Heartbeat, Reconnect, Deadlines) | ✅ | K01 |

### Epic L — Connection Activity & History (3)
| ID | Title | Status | Deps |
|---|---|---|---|
| L01 | dApp Tx Persisted Pending-At-Submit | ✅ | K07 |
| L02 | Replayable dApp History Record (Bounded 24KB) | ✅ | L01 |
| L03 | Read-Only Signing Replay Sheet | ✅ | L02, I01 |

### Epic M — Design System & UI Primitives (5)
| ID | Title | Status | Deps |
|---|---|---|---|
| M01 | Theme Tokens (Mutable, No-Remount) & WCAG Palette | ✅ | A04 |
| M02 | 6-Level Text Scale & Intl-Free Locale Formatting | ✅ | M01 |
| M03 | `AmountText` Atomic-Number Display | ✅ | M02 |
| M04 | Bespoke UI Primitives & Semantic Haptics | ✅ | M01 |
| M05 | i18n: 15 Locales, Typed Keys, Restart-Free Switch | ✅ | A04 |

### Epic N — Settings, Self-Host & Diagnostics (5)
| ID | Title | Status | Deps |
|---|---|---|---|
| N01 | Settings Screen & About | ✅ | A01 |
| N02 | Service Endpoints Configuration & On-Entry Validation | ✅ | A06 |
| N03 | Fault-Injection Harness (`vela.*` Console) | ✅ | A04 |
| N04 | One-Click Bug Report with Sanitized Diagnostics | ✅ | N05 |
| N05 | In-Memory Metrics & Failure Ring Buffer (No Telemetry Backend) | ✅ | A03 |

### Epic O — Ops, Testing, Store & Meta (4)
| ID | Title | Status | Deps |
|---|---|---|---|
| O01 | Parallel-Space Test Environment | ✅ | — |
| O02 | Engineering Rules & AI-Coding Accountability (`agent-rules/`) | ✅ | — |
| O03 | Store Launch Readiness (iOS / Android Submission) | 🚧 | — |
| O04 | Roadmap, Alpha Status & Public On-Chain Verification | ✅ | A02 |

---

## Roadmap items tracked as future requirements

These appear on `getvela.app/roadmap` and are captured as *forward requirements* inside the docs above
(marked 🚧/🔜/🧭), not as separate PRDs:

- **"See every coin you receive"** (🚧 now) — internal-call / native deposit tracing → forward FR in **D05**.
- **Wider clear-signing coverage** (🚧 now) → forward FR in **I02**.
- **Native iOS & Android apps** (🔜 next) → **O03**.
- **Cross-device sync + saved address book** (🔜 next) → forward FR in **H05 / C04 / E06**.
- **Independent audit, desktop dApp connect, non-precompile-chain signing** (🧭 exploring) → **A02 / K01 / F02**.

## Conventions

- **Owner** across all docs: Shelchin (solo founder/engineer) unless a doc says otherwise.
- **Source anchors** use repo-relative paths; when a line number drifts, trust the symbol name.
- Requirement IDs inside a doc: `FR-n` (functional), `NFR-n` (non-functional), `AC-n` (acceptance).
- Cross-doc references use the requirement ID (e.g. "see G05") so files stay independent.

*Backbone generated 2026-07-01 from a full read of the vela-wallet source, `docs/CONTENT-SOURCE-100-CLUES.md`, and `getvela.app/roadmap`. Individual PRDs are filled in per the template.*
