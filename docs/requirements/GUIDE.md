# Vela Wallet — Requirements Reading Guide (导读指南)

> **What this is.** A navigation layer over the 100 requirement documents. The
> [README.md](./README.md) is the *index* (the flat list). This is the *guide* (how to read it):
> curated reading paths by role and question, which docs are high-risk, and how to keep the set
> honest. If you only read one thing first, read this, then [A01](./A01-product-vision-scope-non-goals.md).

## 30-second orientation

- **100 PRDs**, IDs `A01`…`O04`, grouped into **15 epics (A–O)** in dependency order
  (foundations → identity → money movement → dApp → polish → ops).
- Each file is **independent** (read one, it stands alone) but the set is **ordered** (read
  top-to-bottom, the product assembles itself).
- Three files aren't PRDs: **[README.md](./README.md)** (index), **[_TEMPLATE.md](./_TEMPLATE.md)**
  (the shape every PRD follows), and **this guide**.
- Every functional claim cites a **source anchor** (`file:line`) or a clue number. Trust the source
  over prose; the repo root `README.md` is stale (see guardrails in the index).

## How to read a single PRD (where to look)

Every doc has the same 10 sections. Jump by intent:

| You want… | Read section |
|---|---|
| The one-paragraph "what & why" | **1. Summary** |
| Why it exists / the problem it solves | **2. Background & context** |
| Who it's for | **3. Users & stories** |
| The testable behaviors | **4. Functional requirements** (`FR-n`) |
| Perf / security / privacy / i18n bounds | **5. Non-functional requirements** (`NFR-n`) |
| Screens, states, copy, haptics | **6. UX / flow notes** |
| Definition of done | **7. Acceptance criteria** (`AC-n`, checkboxes) |
| What it deliberately excludes | **8. Out of scope / non-goals** |
| Upstream deps, cross-repo risks, unknowns | **9. Dependencies, risks & open questions** |
| Exactly where in the code it lives | **10. Source anchors** |

## Reading paths by role

Pick your track. Each is an ordered short list — read these first, then follow their **Depends on** links.

### 🧭 New to Vela — understand the product (≈45 min)
`A01` → `A02` → `B01` → `B06` → `B07` → `C02` → `D01` → `H01` → `I01` → `K01`
*(what it is & why → passkey identity → the Safe/4337 account → same-address-everywhere → create → home → send → clear signing → dApp connect.)*

### 🔐 Security reviewer — the trust surface (start here)
`B05` · `B06` · `B07` · `G02` · `I07` · `I08` · `J03` · `J05` · `K02` · `K06` · `F06` · `A03`
*(passkey compatibility, account model, address derivation, signature encoding, decimals/amounts, risk floor, spoof-resistant simulation, never-unlimited approvals, MITM fingerprint, SIWE domain-binding, read-flood gate, privacy.)* See **High-risk docs** below.

### 🛠️ Implementer — extending or fixing a feature
1. Find the epic in the [index](./README.md); open the matching PRD.
2. Read its **Depends on** (header) and **§9** — those are your prerequisites and landmines.
3. Read **§10 Source anchors** to jump straight into the code.
4. Check the **status tag**: 🚧 means it's the active branch (`feat/contacts-groups-payroll-batch`); expect churn.

### 📊 PM / evaluator / partner — scope & posture
`A01` (scope & non-goals) · `A02` (voice, honesty, audit stance) · `A03` (privacy/no-token) · `O04` (roadmap, alpha, on-chain verification) · `O02` (how it's built) · `G05` (fee/business model).

### ⚙️ Protocol / crypto deep-dive
`B06` → `B07` → `G01` → `G02` → `I06` → `K05` → `G10`
*(contract set → CREATE2 address → UserOp/deploy → SafeOp hash & WebAuthn sig → ABI/EIP-712 decode → EIP-1271 SafeMessage → Tempo stablecoin-gas.)*

## Reading paths by question

| "How does Vela…" | Docs |
|---|---|
| …work without a seed phrase? | `B01`, `B07`, `B09` |
| …have the same address on every chain? | `B07` (+ `B06`, `G01`) |
| …let me receive before deploying? | `B07`, `G01`, `H09` |
| …decide gas / fees? | `G03`, `G04`, `G05` (+ `G06`, `G07`) |
| …pay gas in a stablecoin (Tempo)? | `G10` |
| …stay up when RPCs fail or rate-limit? | `F03`, `F04`, `F05`, `F08` |
| …price tokens without a price API? | `E01`, `E02`, `E03`, `E04` |
| …show my balance in my currency? | `E05`, `E06` (+ `M02`, `M03`) |
| …avoid blind signing? | `I01`, `I02`, `I06`, `I08` |
| …stop unlimited approvals? | `J05` (+ `I08`) |
| …preview what a transaction does? | `J01`, `J02`, `J03` |
| …connect to a dApp (no Bluetooth)? | `K01`, `K02`, `K03`, `K04` |
| …batch multiple calls atomically? | `K07`, `H07` |
| …pay many people at once (payroll)? | `H06`, `H07`, `H08` (+ `E07`) |
| …resist address poisoning / phishing? | `H03`, `H02`, `K06` |
| …keep my data private? | `A03`, `N04`, `N05` |
| …let me self-host everything? | `A06`, `N02` (+ `F03`, `F07`, `E05`, `B08`) |
| …recover a lost device? | `B09`, `C04`, `B04` |

## High-risk docs (a bug here can strand funds or break the trust model)

Per the engineering rules (`O02`), any change touching these is **auto-High-risk** and must pass
golden vectors / red-team review. Read them with extra care:

`B05` · `B06` · `B07` · `B08` · `G01` · `G02` · `G05` · `I07` · `I08` · `J03` · `J05` · `K05` · `K06`

Rule of thumb: **anything that derives an address, builds/signs a UserOp or message, sets a
spending limit, or renders an amount/risk color** is on the critical path.

## Status tags — how to interpret

| Tag | Read it as | For planning |
|---|---|---|
| ✅ | Shipped; the doc describes current behavior | Safe to rely on |
| 🚧 | Active branch or roadmap "now"; behavior may still move | Expect change; confirm against code |
| 🔜 | Committed-ish "next" | Not built yet; treat FRs as intent |
| 🧭 | Exploring / "later" | Directional only, no timeline |

Roadmap items that don't have their own PRD live as **forward-FRs** inside an existing doc — see the
"Roadmap items tracked as future requirements" section in the [index](./README.md).

## Conventions (quick reference)

- **IDs** are stable: `<Epic><nn>`. Cross-references use the ID (e.g. "see `G05`") so files stay
  independent — never a file path in prose.
- Inside a doc: **`FR-n`** functional, **`NFR-n`** non-functional, **`AC-n`** acceptance.
- **Owner** is Shelchin across the set unless a doc says otherwise.
- **Source anchors** cite `file:line`; when a line number has drifted, trust the **symbol name**.

## How this set relates to the rest of the repo

- **Code** is the ground truth; PRDs point *into* it via §10 anchors.
- **[docs/CONTENT-SOURCE-100-CLUES.md](../CONTENT-SOURCE-100-CLUES.md)** is the *fact bank* (for
  marketing/SEO/docs). PRDs cite it, but a clue ≠ a requirement — one says "what to say," the other
  "what to build."
- **`getvela.app/`** holds the public whitepaper, roadmap, and site copy; treated as canonical over
  the stale root `README.md`.
- **`agent-rules/`** defines the process these PRDs are built under (`O02`).

## Maintaining this set

- **Adding a requirement:** copy [_TEMPLATE.md](./_TEMPLATE.md), pick the smallest free ID in the
  right epic (or append), fill every section, and add a row to the [index](./README.md) table + the
  epic count. Keep §10 anchors real.
- **Updating status:** change the tag in *both* the PRD header and the index row. When a 🚧/🔜 item
  ships, flip it to ✅ and reconcile any forward-FR that referenced it.
- **When code moves:** update the `file:line` anchors (or at least the symbol names) in §10 so the
  doc stays verifiable. A PRD whose anchors don't resolve is a bug.
- **Guardrails are non-negotiable:** no "Bluetooth," no "audit planned," no "beta/tolerate-bugs"
  banner, 12 networks (not 8), ≈2×/~3× fees (not 60%). See the index guardrails before writing.

---

*Companion to [README.md](./README.md) (the index) and [_TEMPLATE.md](./_TEMPLATE.md) (the shape).
Start any exploration from a role track above, then follow each doc's **Depends on** links.*
