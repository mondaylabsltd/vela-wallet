# Feature Specification: 093 — every dApp interaction in Activity, said in plain words

**Feature Branch**: `093-dapp-activity`
**Created**: 2026-10-02
**Status**: Implemented on the branch (not pushed); see [results.md](results.md)
**Input**: Owner (2026-10-02): every dApp interaction — on-chain transactions (single and batch) and
signatures (personal_sign, typed data incl. Permit / Permit2 / SIWE) — appears in Activity on web +
extension, desktop, iOS and Android: one concise row whose title states the intent in plain words
(「在 PancakeSwap 兑换」, 「在 Uniswap 授权签名」, 「在 app.uniswap.org 登录」) with the money moved or the
allowance on the right, and a detail view with the facts and a collapsed technical section. Details on
tap, not up front. Lead's design brief (audit of `main` @ `ec033f231`) fixes the decisions below.

## User Scenarios & Testing

### User Story 1 — A swap, an approval or a batch reads as what it did (Priority: P1)

A person swaps on a dApp. Activity shows one row: 「在 Uniswap 兑换」, the site and network beneath
(「app.uniswap.org · Ethereum」), and on the right what left (≈ −100 USDC) with what came back.

**Independent Test**: core feed tests (`app_activity_feed.rs`, `app_dapp_activity.rs`); each shell's
row-mapping test; screenshots.

**Acceptance Scenarios**:

1. **Given** a dApp transaction to a contract the wallet knows (Uniswap, PancakeSwap…), **When** the
   row is drawn, **Then** its title is "{intent} on {protocol}" and the site's host is on the second
   line; to a contract it does not know, the title names the site's host and the second line does not
   repeat it. Never the dApp's self-declared name.
2. **Given** a swap whose coin came back in its own "Received" record (same tx hash, same account),
   **Then** that record folds into the dApp row (one operation, one row), which states the received
   amount exactly.
3. **Given** an on-chain approval, **Then** the row is 「在 <spender's protocol or site> 授权」 with the
   allowance on the right — "100 USDC", or 无限额 in the danger tone; a revoke reads 撤销 with no figure.
4. **Given** a `wallet_sendCalls` batch, **Then** the title is the one verb its calls share once the
   approvals serving it are set aside (`[approve, swap]` → 兑换), else 「批量」; a call nobody could read,
   or a best-effort guess, gives no verb.
5. **Given** a pending / failed / unknown transaction, **Then** its status leads the second line.

### User Story 2 — A signature is visible, and says what it allowed (Priority: P1)

**Acceptance Scenarios**:

1. **Given** a Permit / Permit2 signature, **Then** a row 「在 <spender's protocol or site> 授权签名」
   shows the allowance (amount or 无限额, danger tone) — no status chip, no figure of money moved.
2. **Given** a SIWE message whose domain is the site that asked, **Then** 「在 <site> 登录」; a SIWE
   message whose domain is another site is a plain 「签名消息」 row (never "sign in" for somebody
   else's domain).
3. **Given** typed data that grants nothing, or `eth_sign`, **Then** 「签名结构化数据」 / 「盲签」.
4. **Given** a signature, **Then** the disk keeps that it was given — never the signature itself; a
   legacy record holding the signature in `txHash` is never read as a hash.
5. Connect / disconnect / chain switch stay out of Activity (Connections lists grants).

### User Story 3 — Details on tap (Priority: P2)

**Acceptance Scenarios**:

1. **Given** a dApp row, **When** it is opened, **Then** at most six facts show, in the core's order:
   site, network, contract or spender, spending cap + expiry (grants) or balance changes, date; a
   transaction keeps its status chip and explorer link; a signature shows "Off-chain signature —
   nothing was sent on-chain" instead of a chip.
2. A collapsed 「技术细节」 section holds the operation, the stored request (read from the store only
   when opened; 「未记录此请求的内容」 when there is none), the typed-data type, the tx hash and the
   UserOp hash.

### Edge Cases

- A record from before 093 (no summary): a transaction reads by its recorded intent, a signature by its
  kind; old Android / iOS records without `dappUrl` fall back to `dappOrigin` (those shells never sent
  a dApp's own name there) — not on web.
- A request longer than the stored cap: the summary is still read from the WHOLE request; the stored
  copy is clipped to 8 KB, string values cut on a character boundary, JSON shape kept.
- Permit2 is infrastructure: it never names the place (an approval of Permit2 on PancakeSwap is no
  Uniswap action); nor WETH or a smart-wallet factory.
- A finite cap in a token nobody could scale (decimals unknown) states nothing rather than a raw number.

## Requirements

- **FR-001** The core builds a `DappSummary` at approve time from the full final params (action, calls,
  contract, spender, token + symbol + decimals, amount or unlimited, revoke, expires_at, signin_domain
  when matched, primary_type ≤ 32 chars); no message bodies or typed documents.
- **FR-002** `ClearSigningView.record_intent` decides the recorded verb (best-effort rule and batch
  headline in the core); `SignApproveOpts.token_meta` carries the approval surface's token.
- **FR-003** `SignRecord` carries `summary`, `stored_request` (≤ 8 KB, UTF-8 safe) and
  `request_truncated`; a signature record's `result` is empty.
- **FR-004** The feed makes rows of `dapp_tx`, `sign_message`, `sign_typed_data` (never `connect`) and
  decides title verb + place, subtitle (every row), allowance, off-chain, detail facts and technical
  lines; swap receipts fold.
- **FR-005** Every shell sends `record_intent`, `token_meta` and its balance changes at approve, stores
  the summary verbatim and the core's stored request, maps them back, and draws title / subtitle /
  figure / allowance / detail from the core. No shell rule.
- **FR-006** i18n: one new key, `history.dappRowTitle`; everything else reuses existing keys.

## Success Criteria

- **SC-001** Core: the three tests that pinned "signatures are excluded" are rewritten; new tests cover
  a summary from a request > 4 KB (and > 8 KB), legacy rows, protocol vs host, permit limited /
  unlimited, SIWE matched / mismatched, batch headline, swap receive folding.
- **SC-002** All suites green (core, web, desktop, iOS, Android) with each shell's own wiring test.
- **SC-003** ja + en residency within the raised 140,800 budget.
- **SC-004** Screenshots of a swap row, a permit row, a SIWE row and a detail on iOS simulator and web
  (phone + desktop width).
