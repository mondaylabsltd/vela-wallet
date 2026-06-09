# Vela Clear Signing — Design Document

## 1. Design Philosophy

**One sentence:** Make every signing request instantly understandable to anyone, regardless of crypto experience.

The user should answer one question in under 3 seconds: **"What am I agreeing to?"**

Everything else is secondary. Technical details exist for verification, not comprehension.

---

## 2. Visual UX Principles

### 2.1 Information Hierarchy (3 layers)

| Layer | What | Visual Treatment | Always Visible |
|-------|------|-----------------|----------------|
| **L1 — Intent** | What action is this? | Large bold word (28px, 800 weight), colored by risk | Yes |
| **L2 — Substance** | What's at stake? (amount, token, recipient) | Token cards with gradients, big numbers (24px) | Yes |
| **L3 — Context** | Technical details (contract, chain, nonce, deadline) | Muted text, collapsible panel | Partially — contract bar always, details collapsed |

### 2.2 Core Rules

1. **Intent first** — The action word (Swap, Send, Approve, Sign) is the largest text on screen
2. **Amount is king** — Token amounts are the second-most prominent element, with USD estimates
3. **Direction is visual** — Arrows and card colors communicate flow (warm=outgoing, cool=incoming)
4. **Risk = color** — Green (safe), neutral (normal), orange (caution), red (danger)
5. **Contract always visible** — Every signing request shows which contract is being interacted with, with copy button
6. **Details on demand** — Nonce, deadline, chain, selector etc. are collapsed by default
7. **No jargon in L1/L2** — L1 and L2 never show hex, selectors, or ABI types. Those go in L3.

### 2.3 Color System for Signing

| Risk Level | Intent Color | Card Gradient | Button | When |
|-----------|-------------|---------------|--------|------|
| **Safe** | `success` (#22a456) | green tint | Green | Stake, deposit, claim |
| **Normal** | `accent` (#c35a2a) | warm/cool tint | Accent | Swap, send, transfer |
| **Caution** | `warning` (#d4890a) | yellow tint | Accent | Approve, permit (limited) |
| **Danger** | `danger` (#d43a2a) | red tint | Red | Unlimited approve, blind sign, unknown |
| **Signature** | `purple` (#6c5ce7) | purple tint | Purple | personal_sign, EIP-712 (no asset risk) |

### 2.4 Button Labels

Buttons should reflect the action, not be generic:
- Swap → "Confirm Swap"
- Send → "Confirm Send"
- Approve → "Approve"
- Stake → "Confirm Stake"
- personal_sign → "Sign"
- EIP-712 → "Sign"
- Blind sign → "Sign Anyway" (red button)

---

## 3. Signing Scenarios

### 3.1 `eth_sendTransaction` — Clear Signed (descriptor found)

**Resolution flow:**
1. Extract `to`, `data`, `value` from params
2. Fetch `erc7730/calldata/eip155-{chainId}/{to}.json`
3. Fallback to `erc7730/ercs/calldata-erc20-tokens.json`, `calldata-erc721-nfts.json`, `calldata-erc4626-vaults.json`
4. Match function selector → get `intent` + `fields[]`
5. Decode calldata with ABI, resolve field paths, format values

**UI layout:**
```
┌─────────────────────────────────┐
│  [dApp logo]  dApp Name         │  ← dApp banner (logo, name, domain, E2E badge)
│               domain.com        │
├─────────────────────────────────┤
│           {Intent}              │  ← L1: Large colored word
│                                 │
│  ┌────────────────────────────┐ │
│  │ [token] 1,500 USDC        │ │  ← L2: "You pay" token card (warm gradient)
│  │         You pay · ≈$1,500  │ │
│  └────────────────────────────┘ │
│            ↓  (flow arrow)      │
│  ┌────────────────────────────┐ │
│  │ [token] 0.584 WETH        │ │  ← L2: "You receive" token card (cool gradient)
│  │         min · ≈$1,485      │ │
│  └────────────────────────────┘ │
│                                 │
│  📄 Interacting with            │  ← Contract bar (name + address + copy + verified)
│     Uniswap V3 Router          │
│     0x3fC9...b537  [Copy] ✓    │
│                                 │
│  ▼ Details                      │  ← Collapsed: slippage, deadline, chain, account
│                                 │
│  [Reject]  [Confirm Swap]       │  ← Buttons
└─────────────────────────────────┘
```

**Rendering by intent:**

| Intent | L2 Layout | Notes |
|--------|-----------|-------|
| Swap/Exchange | Two token cards with ↓ arrow | "You pay" → "You receive (min)" |
| Send/Transfer | One token card + recipient bar | Show ENS if available |
| Approve | One token card + spender bar | Warn if unlimited (threshold check) |
| Stake/Deposit | Token card + receive card (stToken) | Show what you get back |
| Withdraw/Redeem/Claim | Token card you receive | Success color |
| Other (Buy, Create, etc.) | Generic: show all `visible: always` fields as rows | Fallback |

### 3.2 `eth_sendTransaction` — Plain ETH Transfer (no calldata)

When `data` is empty/`0x` — native ETH transfer.

**UI:**
- Intent: **"Send"**
- One token card showing amount + native symbol
- Recipient bar with address (+ ENS lookup if available)
- No "Details" section needed
- Button: "Confirm Send"

### 3.3 `eth_sendTransaction` — Blind Sign (no descriptor)

When no ERC-7730 descriptor matches.

**UI:**
- Intent: **"Unknown"** (red)
- Token card showing ETH value (if any) with danger gradient
- Contract bar marked as **"Unverified"** (no green check, warning icon)
- Red warning banner: "Unable to decode — no ERC-7730 descriptor"
- Collapsed raw calldata (hex + byte count + selector)
- Red button: **"Sign Anyway"**

### 3.4 `personal_sign` — Message Signing

**No ERC-7730 lookup needed.** This is a plain text message (no asset risk).

**UI:**
- Intent: **"Sign Message"** (purple — signature, no asset movement)
- Message displayed in a bubble/card:
  - If printable UTF-8: show as readable text, centered
  - If hex (non-printable): show truncated hex with warning
- Tag: `personal_sign · No gas fee`
- Context strip: chain + account
- Purple button: **"Sign"**

**Visual principle:** Keep it light and safe-feeling. Purple color communicates "this is a signature, not a transaction." No asset-related UI elements (no token cards, no amounts).

### 3.5 `eth_signTypedData_v4` — Clear Signed (descriptor found)

**Resolution flow:**
1. Parse typed data JSON from params
2. Extract `domain.verifyingContract`
3. Compute `typeHash` = keccak256(encodeType(primaryType))
4. Fetch `erc7730/eip712/eip155-{chainId}/{contract}.json`
5. Match by typeHash → get `intent` + `fields[]`
6. Fallback to `erc7730/ercs/eip712-erc2612-permit.json`
7. Resolve fields from `message` object

**UI layout:**
- Same structure as calldata clear signing
- Intent word from descriptor (e.g., "Sign Permit", "UniswapX Dutch Order")
- Token card if `tokenAmount` field exists
- Spender/recipient bar if `addressName` field exists
- **Key difference from personal_sign:** Shows structured fields, may involve asset risk
- Button color: Purple for pure signatures, accent if involves value transfer

**Distinguishing from personal_sign:**
- personal_sign: message bubble, purple, "No gas fee"
- EIP-712 clear: structured fields with labels, may show token amounts, contract verified badge
- EIP-712 blind: same as below (3.6)

### 3.6 `eth_signTypedData_v4` — Blind Sign (no descriptor)

When no ERC-7730 descriptor matches the typeHash.

**UI:**
- Intent: **"Sign Typed Data"** (orange — unknown risk)
- Show domain info: `domain.name`, `domain.verifyingContract` (with copy)
- Show `primaryType` name
- Show message fields as raw key-value pairs (max 3-4, rest collapsed)
- Orange warning: "This typed data could not be decoded with a known descriptor."
- Button: **"Sign Anyway"** (orange, not red — less severe than unknown transaction)

---

## 4. Component Architecture

### 4.1 Data Flow

```
SigningRequestModal
  │
  ├─ useEffect: resolveTransaction() or resolveTypedData()
  │     │
  │     ├─ clear-signing.ts: fetch descriptor from registry
  │     ├─ abi-decode.ts: decode calldata
  │     └─ returns ClearSignResult | null
  │
  ├─ if ClearSignResult → <ClearSignView />
  ├─ if null + personal_sign → <MessageSignView />
  ├─ if null + signTypedData → <BlindTypedDataView />
  └─ if null + sendTransaction → <BlindTransactionView />
```

### 4.2 New Component Structure

```
SigningRequestModal.tsx (orchestrator)
  ├── DAppBanner          — dApp logo, name, domain, E2E badge
  ├── ClearSignView       — descriptor-driven rendering
  │   ├── IntentHeader    — large colored intent word
  │   ├── TokenCard       — amount + token + gradient
  │   ├── FlowArrow       — directional arrow between cards
  │   ├── ContractBar     — contract/spender/recipient + address + copy + verified
  │   ├── DetailsPanel    — collapsible secondary fields
  │   └── ActionButtons   — contextual reject/confirm
  ├── MessageSignView     — personal_sign bubble
  ├── BlindTypedDataView  — EIP-712 without descriptor
  └── BlindTransactionView — transaction without descriptor
```

### 4.3 ClearSignResult Extension

Current `ClearSignResult`:
```typescript
interface ClearSignResult {
  intent: string;
  contractName?: string;
  owner?: string;
  fields: ClearSignField[];
}
```

**Proposed additions:**
```typescript
interface ClearSignResult {
  intent: string;
  contractName?: string;
  owner?: string;
  fields: ClearSignField[];

  // New fields for enhanced rendering:
  /** Risk level derived from intent + fields */
  risk: 'safe' | 'normal' | 'caution' | 'danger';
  /** Contract address being interacted with */
  contractAddress?: string;
  /** Whether the contract has a verified descriptor */
  verified: boolean;
  /** Signing type for button/color decisions */
  type: 'transaction' | 'signature';
}
```

### 4.4 Enhanced ClearSignField

```typescript
interface ClearSignField {
  label: string;
  value: string;
  format: string;
  tokenAddress?: string;
  warning?: boolean;

  // New fields:
  /** Role hint for layout decisions */
  role?: 'send-amount' | 'receive-amount' | 'recipient' | 'spender' | 'generic';
  /** Whether this field should be in the collapsed details panel */
  detail?: boolean;
}
```

The `role` field enables the modal to decide layout:
- `send-amount` → warm gradient token card
- `receive-amount` → cool gradient token card
- `recipient` / `spender` → contract bar with copy button
- `generic` → standard label/value row

---

## 5. ERC-7730 Registry Integration

### 5.1 Lookup Priority

**For `eth_sendTransaction`:**
1. `erc7730/calldata/eip155-{chainId}/{to}.json` — contract-specific
2. `erc7730/ercs/calldata-erc20-tokens.json` — ERC-20 universal
3. `erc7730/ercs/calldata-erc721-nfts.json` — ERC-721 universal
4. `erc7730/ercs/calldata-erc4626-vaults.json` — ERC-4626 universal
5. `erc7730/ercs/calldata-erc7540Deposit-vaults.json`
6. `erc7730/ercs/calldata-erc7540Redeem-vaults.json`
7. **No match → blind sign**

**For `eth_signTypedData`:**
1. `erc7730/eip712/eip155-{chainId}/{verifyingContract}.json` — contract-specific (keyed by typeHash)
2. `erc7730/ercs/eip712-erc2612-permit.json` — ERC-2612 universal
3. **No match → blind typed data sign**

**For `personal_sign`:**
- No ERC-7730 lookup — always rendered as message bubble

### 5.2 Supported Format Types

All format types from the registry must be rendered:

| Format | Current Support | Rendering |
|--------|----------------|-----------|
| `tokenAmount` | ✅ | Big number + token symbol + USD estimate |
| `addressName` | ✅ | Short address + ENS/contract name if known |
| `amount` | ✅ | Native currency amount (ETH, MATIC, etc.) |
| `raw` | ✅ | Truncated hex or string |
| `date` | ✅ | Human-readable date (e.g., "Jun 16, 2026") |
| `duration` | ✅ | Human-readable duration (e.g., "30m", "24h") |
| `enum` | ✅ | Resolved enum label from metadata |
| `unit` | ✅ | Numeric with unit suffix/prefix |
| `calldata` | ✅ | Truncated hex (nested calldata) |
| `nftName` | ✅ | NFT name string |

### 5.3 Coverage

Current registry:
- **57 chains** supported
- **242+ calldata descriptors** (Ethereum mainnet)
- **38+ EIP-712 descriptors** (Ethereum mainnet)
- **6 universal ERC standards** (ERC-20, ERC-721, ERC-4626, ERC-2612, ERC-7540)

---

## 6. Implementation Plan

### Phase 1: Restructure Modal Components
- Extract `DAppBanner`, `TokenCard`, `FlowArrow`, `ContractBar`, `DetailsPanel` as reusable components
- Add `risk` and `role` fields to `ClearSignResult` / `ClearSignField`
- Implement risk-level → color mapping

### Phase 2: Redesign Clear Sign View
- Implement new token card with gradient backgrounds
- Add contract bar with copy-to-clipboard
- Add collapsible details panel
- Contextual button labels and colors

### Phase 3: Differentiate Message Types
- `personal_sign` → dedicated `MessageSignView` (purple, bubble, lightweight)
- `eth_signTypedData` blind → dedicated `BlindTypedDataView` (show domain + primary type + raw fields)
- Clear distinction in visual weight and risk communication

### Phase 4: Polish
- Smooth modal open/close animations
- Loading skeleton while descriptor is being fetched
- Haptic feedback on approve/reject
- Copy-to-clipboard toast notification

---

## 7. Risk Assessment Logic

```
function assessRisk(intent, fields, method):
  if method === 'personal_sign':
    return 'safe'  // no asset risk

  if no descriptor found:
    if method includes 'signTypedData':
      return 'caution'  // unknown signature
    return 'danger'  // unknown transaction

  // Check for unlimited approvals
  if any field has warning === true:
    return 'danger'

  // Check by intent
  switch intent.toLowerCase():
    'approve', 'permit', 'authorize': return 'caution'
    'stake', 'deposit', 'claim', 'supply': return 'safe'
    'swap', 'send', 'transfer', 'buy': return 'normal'
    'withdraw', 'redeem', 'unstake': return 'normal'
    default: return 'normal'
```

---

## 8. Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| personal_sign color | Purple | Communicates "signature only, no asset risk" |
| Blind sign button | Red "Sign Anyway" | Explicit friction for dangerous actions |
| Contract address | Always visible + copyable | Security requirement — users must verify |
| Details panel | Collapsed by default | Reduces cognitive load; power users can expand |
| Token amounts | Big numbers, no hex | Most users don't understand wei or hex values |
| Unlimited approvals | Red highlight + warning text | Critical security signal |
| EIP-712 vs personal_sign | Different components | Different risk profiles need different UIs |
