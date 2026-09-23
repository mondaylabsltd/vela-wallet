# Dead controls — the sweep of 2026-09-23

A device run found a settings row that looked tappable, had a chevron, and did
nothing: `SettingsNetworkRow` owns an `.onTapGesture` calling an `onTap` that
defaults to a no-op, and the call site omitted it and wrapped the row in an
outer gesture the inner one swallows. It compiles, it renders, it has a hit
area, it never acts.

That is a class, not an incident, so all four shells were swept for it: **a
visible control whose action, on the live path, is a defaulted no-op.** Twenty
findings. This file is the worklist; each one is fixed on branch
`fix/081-followups` or carries a reason why not.

Severity is what the person loses: money or safety first, then a feature that
cannot be reached at all, then a control that has another route.

| # | Shell | Where | What is dead | Status |
| --- | --- | --- | --- | --- |
| 1 | iOS | `FlowBlocks.swift:168` ← `FlowHost.swift:334` | The ⇄ toggle under the send amount. `SendStore.toggleFiatInput()` has **no call site in the whole iOS tree**; the other three shells all dispatch it. |**fixed** — `FlowHost.onDenom` seam, `RootView` sends `toggleFiatInput()` |
| 2 | iOS | `FlowBodies.swift:1308` ← `FlowHost.swift:388` | "View on explorer" on a **confirmed** send receipt. Doubly dead: `RootView.explorerLink(for:)` also returns `nil` for that state. |**fixed** — body gets `onExplorer`, and `explorerLink(for:)` now answers for `.sd4a/b/c` off the token's chain |
| 3 | Android | `FlowScreens.kt:929` ← `FlowHost.kt:231` | The MAX pill on every sweep row — `onMax` is `() -> Unit`, the row index is discarded, and the core has no per-row max event. |open — Android; needs a device run (sweep, 2+ tokens, MAX on row two) |
| 4 | Desktop | `page.rs:3187-3207` | Contact detail's 转账 / 收款 / 二维码 pills. 二维码 has **no other path at all**. |**fixed** — 转账 prefills the send form, 收款 opens receive, 二维码 opens a new dialog with a real code |
| 5 | Desktop | `page.rs:2994`, `contacts/components.rs:408` | The group view's accent-filled 群发转账 button — accent means "this moves money" in this codebase. |**fixed** — one member prefills, two or more append as split recipients |
| 6 | Web | `FeeRow.svelte:61` ← `SigningHost.svelte:43` | The signing sheet's fee row, on a chain with exactly one fee coin. |**fixed** — `tappable` decided in the live builder; with one coin the row is a statement, on web, iOS, desktop and Android |
| 7 | Desktop | `page.rs:7020-7025` | The currency select in Region & formats. `Event::UserChose` is never dispatched: **there is no way to change display currency on desktop at all.** |**fixed** — the menu lists what the rate endpoint can price, with a sample in each |
| 8 | iOS + Android | `WalletKeysBlock.swift:82`, `SettingsRows.kt:80` | The Ethereum-backup row taps in all four states but only `notBackedUp` carries a `call` — including "could not check", which is exactly when a person taps to retry. Both shells gate the chevron and forget the click. |open — **founder question**: should "could not check" retry, or should the row stop being tappable? |
| 9 | iOS | `ConnectionPanelView.swift:58` ← `ExploreScreen.swift:577` | "Switch account" in the dApp connection sheet, chevron and all. Android draws the same row with no click either; web wires it. |**built** (founder, 2026-09-23) — iOS + Android open the switcher and the core re-pins the grant. iOS was also never telling the browser a switch had happened at all |
| 10 | iOS | `FlowBodies.swift:424-436` ← `FlowHost.swift:538` | The add-token sheet's ERC-20 / native segmented control and its network row. FlowHost has no seam for either, and `FlowsLive.addToken` falls back to the **fixture's** network, so the dead chevron shows a fixture chain name. |**fixed** — 原生 goes to 添加网络 (Android's route), the fixture-network fallback is gone, the row is a label |
| 11 | Desktop | `settings/components.rs:873`, `page.rs:7793` | Settings → Storage: every per-row 清除 and 清除全部缓存 are plain `div`s. (Same block: only the total is live, every per-row meta is fixture.) |**fixed** — `executor/device_storage.rs` measures it, and each clear asks first |
| 12 | iOS + Web | `SettingsScreen.swift:526`, `AddNetworkPanel.svelte:87` | "Open chain setup tool" on an **incompatible** chain — the one thing the wallet just told you to do. Desktop does it right; iOS has no such URL constant anywhere. |**iOS fixed** (`ExternalLinks.chainSetup`); web open |
| 13 | iOS + Desktop | `SettingsRows.swift:383`, `settings/components.rs:890` | Settings → About: getvela.app, the GitHub repo, safe.global, each wearing an external-link icon. Android wires them. |**both fixed** — the row carries the URL beside the host it draws |
| 14 | Desktop | `page.rs:3017` | Group members: each row has a pointer cursor and a hover wash and neither click works. From a group you cannot open, pay or edit a member. |**fixed** — left-click opens the member, right-click menus them, both by address→index |
| 15 | Web | `ActivityRow.svelte:23` ← `ContactsHome.svelte:216`, `ContactDetailPanel.svelte:86` | A contact's recent-activity rows press in under your finger and do nothing. |**fixed** — the row is a button only when it is given an onclick |
| 16 | Desktop | `page.rs:9756` | Explore's "+ 添加" favourites tile. The `site_tile` beside it chains both click and right-click. |**fixed** — asks for an address, refuses one that is not, dispatches `FavoriteAdded` |
| 17 | Desktop | `page.rs:10491` | The signing sheet's "Advanced — view raw data" chevron: the only route from the decoded summary to the raw payload before signing. Reads as not-yet-built, but it ships drawn in the live sheet. |**fixed** — it opens the REQUEST: the method, each call's destination and calldata, or the payload |
| 18 | Web | `TxDetail.svelte:59`, `AssetDetailPanel.svelte:88`, `TokenDetail.svelte:79` | "View on explorer" drawn identically to the working `<a>` when there is no hash or no explorer URL. The source says it "draws the control inert"; nothing makes it look inert. |**fixed** — visibly disabled, the codebase's one look for an unavailable action |
| 19 | Web | `AssetRow.svelte:24` ← `SendForm.svelte:108` | Sweep form: each asset row is an enabled pressable button that does nothing (only its nested Max works), and `<button>` inside `<button>` is invalid HTML. |**fixed** — the row is a reading and the Max is the control; the nested button that split the DOM is gone |
| 20 | iOS | `SettingsScreen.swift:623`, `SettingsPrimitives.swift:232` | RPC providers with no key: "Get an API key →" shown **twice** in link blue, neither tappable. `ProviderCardModel` has no `linkUrl` field at all. |**fixed** — the link opens the provider's key page, and the in-field action runs the test (Android's `onAction`) |

## What the sweep turned up that was not on the list

Looking at the screens to check the fixes found worse things than the fixes.

- **Desktop opened the wrong person.** The contacts directory is A-Z while
  everything acting on "the selected contact" reads the core's order, and the
  row stored its A-Z position. Clicking Alice opened Dave; right-clicking
  Alice then 移入分组 wrote **Dave's** address into the group. 复制地址 and
  删除 take the same path. It shipped; nothing in this sweep caused it.
- **Desktop's currency picker changed one settings row and nothing else** —
  the wallet home still said USD.
- **The desktop currency menu could not reach half its options**: the endpoint
  prices thirty, the menu had no height cap and no scroll, and fourteen were
  unreachable at any window size.
- **The QR dialog did not occlude**: clicking the code closed it *and*
  actuated the contact row underneath.
- **The web sweep row's nested button** split the server-rendered DOM and made
  hydration fail outright.
- **iOS never told the browser about an account switch**, so a connected site
  kept a grant pinned to an account the wallet had stopped signing as.

## Cannot be settled by reading

- #3 needs a device run: sweep, two or more tokens, MAX on the second row.
- #6 needs a chain with exactly one fee coin.
- #18 needs a hashless activity record, or a custom chain with no explorer URL.
- #19's nested-`<button>` question needs an SSR/hydration check.

## What the sweep also established

- The **outer-gesture-swallowing** variant that started this was searched for
  exhaustively on iOS and returned **zero other hits**. The device bug was the
  only instance of that exact shape.
- Desktop has a house rule — drop `cursor_pointer`/`hover` when the action is
  `None` — that `menu_card`, `rpc_banner`, `address_block`, `danger_card` and
  `flows::panels::clickable` all follow. The findings above are its exceptions.
- Two pre-flagged candidates were **false positives** (`ContactDetailScreen`,
  `GroupDetailScreen` in `RootView`): every handler is passed. A crude script
  finds these; only reading settles them.

## Questions for the founder

**#9 — what does "Switch account" mean for a site that is already connected?**
Not one of the four shells passes a handler: iOS and Android draw the row
inert, and both web mounts (`ExploreHome.svelte:283`, `ExploreDesktop.svelte:147`)
leave `onswitch` out too. So this is a promise nobody kept rather than a
wire somebody dropped. The rule it runs into is deliberate: a grant is
pinned to the address it was given to, "never to the wallet's active
account: switching accounts must not silently hand a site a different
identity" (`DpermWire.swift:24`). An explicit switch is not silent, so it
is allowed — but it means re-granting this origin to another account, and
that is a decision rather than a wiring task.

**#8 — should "could not check" be retryable?** The backup row is tappable
in all four states and only `notBackedUp` does anything. Either the row
stops taking taps where there is nothing to do (the house rule), or
"could not check" retries the check — which is what a person tapping it
actually wants, and is a small feature rather than a fix.
