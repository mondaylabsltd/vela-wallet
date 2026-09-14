# Results — 056 iOS Port Completion

## Baselines

| | at 056's start | now |
|---|---|---|
| hermetic test run | 523 in 66 suites | **558 in 72 suites** |
| XCUITest methods | 35 | 35 |
| literal-audit violations | 35 | **35** |
| `vela_core_uniffi.swift` bytes | 353,772 | untouched |
| event-parity strong diffs | — | **2** (from 19 on the first run) |
| dropped judgements | — | **21** carried-but-unread, 7 not on the wire |

## What each phase did

**0 · Preferences and formats.** Five storage keys whose SPELLINGS are the
contract — web and Android write exactly these, so a person restoring a backup
onto another of their own devices finds their choices intact. Four number
presets, five date presets, two clocks, ported from web's `locale-format.ts`:
explicit rules, never the platform's formatter, because a wallet that renders a
figure by the device's idea of a locale reads two ways on two machines. The
bigint rule has a test: 2^256−1 grouped digit by digit, never through a
`Double`.

**1 · The presets reach the screens.** Hero, asset rows, feed days, the send
form's fiat line, the transaction detail, the address book's dates, and the
clear-signing panel. The chosen SIZE multiplies a screen's own scale rather than
replacing it. The signing panel is handed the RESOLVED preset, never "auto" —
only a shell can turn that into a convention.

**2 · The settings page acts.** Every control. The three format sheets are
rebuilt from the presets themselves, because the drawn ones key their rows by
position over hardcoded strings — fine for a picture, useless for a choice.
全部清除 clears what can be looked up again and nothing else. 通讯录 and 反馈
came off the home, per the founder's ruling.

**3 · Deep links.** `CFBundleURLTypes` declared (there was none) and
`applinks:getvela.app` finally has something behind it. The shell tokenises, the
CORE validates. `open` takes a web page and nothing else.

**4–5 · Two rulers, and the nineteen differences they found.** See the commit;
the residue is below with a reason for each.

## Success criteria

| | claim | verdict |
|---|---|---|
| SC-001 | a chosen currency changes the hero, the rows and the detail | **device-adjacent** — simulator screenshot shows the hero at ¥3.28 with the CNY preset and the asset row agreeing; the machinery is `FormatsReachTheScreensTests` |
| SC-002 | a chosen number format changes every figure | **test-only** — asserted where each figure SHOWS, not on the settings page |
| SC-003 | a chosen language takes effect without a relaunch | **test-only** — the key is written and read; the app's own `Loc` re-resolution on a live change is **owed** (see below) |
| SC-004 | the RPC and endpoint pages drive the core | **test-only** — the parity ruler is the evidence: `NetEvent` went from 8 of 19 dispatched to 19 of 19 |
| SC-005 | storage reports real bytes and clearing changes them | **not done** — see below |
| SC-006 | a pay-link opens a prefilled send | **half device-verified** — `simctl openurl` raises the system's "Open in Vela Wallet?", so the scheme is registered and the link reaches the app; the prefill is `PayLinkVerdictTests` |
| SC-007 | every row acts, or is not drawn | **test-only** — `SettingsAliveTests` |
| SC-008 | both rulers report zero strong differences | **2 remain, both with reasons** (below) |

## The residue, with reasons

### Events (2 strong)

| | why it is not dispatched |
|---|---|
| `contacts::GroupSave` | Creating or renaming a group has **no drawn form** on this client — the C6 menu's 编辑分组 opens nothing. Inventing one is what the founder's draw-first ruling forbids. Needs a drawing. |
| `dapp_permissions::PopupRequest` | A browser POPUP WINDOW. There is no such thing in a `WKWebView` tab; the request arrives as a normal one. Web-only by construction. |

### Fields the core computes and no screen reads (21)

| View · field | why |
|---|---|
| `FeedView.newItemId`, `.toast` | The receipt toast and the row glow. **Needs a drawing** — there is no toast surface on this client at all. The notification and the receipt screen both already say a transfer landed. |
| `BatchView.totalToken`, `.totalFiat` | The importer's total. The drawn SD2c has no total line; the CTA carries the count. Recorded rather than added. |
| `BatchView.applied` | A one-shot the shell acts on by seeding the split; reading the flag as well would be a second source for one fact. |
| `SendView.splitOverBalance` | The per-row problem line already says which row is over. A screen-level repeat of the same refusal is what 054 deliberately replaced. |
| `SendReceiptView.submittedAtMs` | The receipt shows a status, not a clock. The feed's row carries the time. |
| `ClearMessageView.isHex` | The core has already DECODED the payload; the shell shows the decoded text or the binary preview, and the flag only says which branch was taken. |
| `ClearSigningView.dangerHaptic` | The haptic policy is 056 US4's `VelaHaptic`, which this cut did not reach. **Owed.** |
| `GuardBatchView.anyToOwnToken` | A batch approval where one leg targets the wallet's own token. Drawn nowhere; the guard's own blocks carry the refusal. |
| `ContactRecipientView.verified` | The green check for a saved AND starred recipient. The contact page shows the star itself; the send's recipient row does not draw a badge. Needs a drawing. |
| `DpermConsentView.methods` | The methods a consent grants. The drawn consent panel names the site and the account, not the method list. |
| `DpermView.connectedAddress` | The connection panel shows the site; the address is the signed-in one by construction on this client (one account per browser session). |
| `ExploreView.tabsFull` | Whether the tab strip is at its cap. Nothing draws a cap. |
| `NetEndpointView.defaultValue` | What an endpoint would revert to. 恢复默认 resets them all; a per-field revert is not drawn. |
| `PaymentRequestView.qrValue`, `.copyPayload`, `.hasAmount` | The REQUEST-BUILDING half of that machine — the mode toggle, the amount, the shareable link. No drawn home on this client. **Needs a drawing**; 056 wired only the `/pay` reading half. |
| `SignView.reconcilePending` | Whether a submitted operation is still being reconciled. The receipt's own stage says the same thing. |
| `BalanceView.bannerChainIds`, `.holdingsLoading` | The per-chain failure banner and the holdings spinner. The status line already says a chain failed; a banner is a second surface for it. |

### Fields not on the Swift wire (7)

`BalanceView.unreachable`, `DpermPopupView.granted`, `DpermView.popup`,
`ReceiveWatchView.deposits`, and `RpcPoolView.failedChains` /
`.rateLimitedChains` / `.banned`. Each is a subset the drift gate allows and
each has the same shape of reason: a popup that cannot exist here, a pool
diagnostic no screen draws, a deposit list the watcher answers through a
callback instead.

## Owed

1. **A live language change does not re-resolve `Loc`.** The key is written and
   the next launch reads it; the current process keeps the language it started
   with. SC-003 is half.
2. **Storage (ST13) reports drawn numbers**, not real bytes. SC-005 not done.
3. **`VelaHaptic`** — the touch policy (one gesture, one buzz; nothing on
   navigation, scrolling, tabs or typing) is not written.
4. **051's unreachable list** is not cleared: the native transaction detail, the
   native-coin token tab, the asset-limited receive, the identicon viewer from
   every avatar, and the category chips.
5. **`docs/ios/`** — the install-verify loop and the program's own handover.

These are 057's to pick up or to record as the program's remaining debt.
