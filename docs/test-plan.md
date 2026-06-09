# Vela Wallet — Automated Test Plan

Based on [CLAUDE-AUTO-TEST.md](../../walletpair/agent-rules/CLAUDE-AUTO-TEST.md) and [AI-CODING-RULES.md](../../walletpair/agent-rules/AI-CODING-RULES.md).

## Current State

- **Unit tests:** 19 files, 263 tests (250 pass, 13 fail — all network-dependent)
- **E2E tests:** 1 file, 6 smoke tests (Playwright, web only)
- **Frameworks:** Jest (unit), Playwright (E2E)
- **Gaps:** No component tests, no integration tests, no UI interaction tests, no auth-bypass test infrastructure

## Strategy: Test Pages for Auth-Free UI Testing

Many wallet features require a created wallet (passkey auth). Rather than mocking the entire auth flow, we create **dedicated test pages** (`/test-*` routes) that:

1. Render real components with mock data injected via props/context
2. Bypass wallet creation, passkey auth, and network calls
3. Are only accessible in development mode
4. Allow Playwright E2E tests to exercise the full UI without auth

### Test Page Architecture

```
src/app/test-ui.tsx           → Test page entry (dev-only route)
src/screens/test/TestUIPage.tsx → Renders all testable UI sections

Sections:
├── Onboarding flow (mock wallet state)
├── Home screen (mock balances, tokens)
├── Send flow (mock token list, address validation)
├── Connect flow (mock dApp connection states)
├── Signing modals (all 16 clear-signing scenarios)
├── Settings (all settings views)
├── Transaction history (mock tx list)
└── Error states (network errors, signing errors)
```

## Test Priority (per agent-rules: core logic > data > API > utilities > UI)

### Priority 1: Core Logic (Unit Tests)

| Module | File | Status | Gap |
|--------|------|--------|-----|
| EIP-712 hashing | eip712.test.ts | ✅ Pass | — |
| Eth crypto (keccak, sign) | eth-crypto.test.ts | ✅ Pass | — |
| Safe address derivation | safe-address.test.ts | ✅ Pass | — |
| Safe transaction building | safe-transaction.test.ts | ✅ Pass | — |
| Hex utilities | hex.test.ts | ✅ Pass | — |
| ABI encoding/decoding | abi.test.ts | ✅ Pass | — |
| **Clear signing service** | — | ❌ Missing | Field resolution, risk assessment, format types |
| **ABI decoder (generic)** | — | ❌ Missing | decodeCalldata, matchSelector |
| **Wallet state reducer** | — | ❌ Missing | CREATE_WALLET, ADD_ACCOUNT, SWITCH_ACCOUNT |

### Priority 2: Data Layer (Unit Tests)

| Module | File | Status | Gap |
|--------|------|--------|-----|
| Storage service | storage.test.ts | ✅ Pass | — |
| Network model | network.test.ts | ✅ Pass | — |
| Types/formatting | types.test.ts | ✅ Pass | — |
| Send helpers | send-helpers.test.ts | ✅ Pass | — |
| **Token discovery** | — | ❌ Missing | chain-tokens.ts |
| **Balance cache** | — | ❌ Missing | balance-cache.ts |

### Priority 3: API/Service Layer (Unit + Integration Tests)

| Module | File | Status | Gap |
|--------|------|--------|-----|
| Bundler service | bundler-service.test.ts | ✅ Pass | — |
| RPC pool | rpc-pool.test.ts | ⚠️ Network fails | Mock RPC calls |
| DApp signing | dapp-signing.test.ts | ✅ Pass | — |
| Attestation parser | attestation-parser.test.ts | ✅ Pass | — |
| **WalletPair transport** | — | ❌ Missing | connect, disconnect, message routing |
| **Remote inject transport** | — | ❌ Missing | SSE/POST bridge |

### Priority 4: UI/UX (E2E Tests via Test Pages)

| Flow | Current | Target |
|------|---------|--------|
| Onboarding screen renders | ✅ Basic | Visual elements, button states |
| Create wallet flow | ❌ | Mock passkey, verify state transitions |
| Home screen (with wallet) | ❌ | Balance display, token list, pull-to-refresh |
| Send flow | ❌ | Address input, amount input, review, confirmation |
| Connect flow | ❌ | QR scanner open/close, paste URI, connection states |
| **Clear Signing — all 16 scenarios** | ❌ | Intent display, token cards, contract bars, copy, approve/reject |
| Settings | ✅ Basic | Theme toggle, network editor, account switcher |
| Transaction history | ❌ | Tx list rendering, status badges, receipt modal |
| Error states | ❌ | Network error, signing error, invalid input |

## E2E Test File Structure

```
e2e/
├── smoke.spec.ts              (existing - basic load tests)
├── clear-signing.spec.ts      (new - all 16 signing scenarios)
├── wallet-ui.spec.ts          (new - home, send, receive, history)
├── connect.spec.ts            (new - connect flow states)
├── settings.spec.ts           (new - settings interactions)
└── helpers/
    └── test-utils.ts          (shared navigation, wait helpers)
```

## Test Page Design

The test page (`/test-ui`) renders isolated UI sections with mock data, each wrapped in a collapsible card. Playwright navigates to `http://localhost:8081/test-ui#section-name` and tests specific sections.

Key mock data includes:
- `MockWalletProvider` — provides wallet state without passkey auth
- `MockDAppConnectionProvider` — simulates dApp connection states
- `MockSigningRequest` — triggers signing modals with predefined scenarios
- `MockTokenList` — provides balance/token data without API calls

## Run Commands

```bash
# Unit tests
npm test

# E2E tests (auto-starts dev server)
npm run test:e2e

# E2E headed (for debugging)
npm run test:e2e:headed

# Stability verification (30 cycles)
./scripts/test-stability.sh 30
```
