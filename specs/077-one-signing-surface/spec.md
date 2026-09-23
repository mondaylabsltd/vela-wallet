# Feature Specification: one signing surface

**Feature Branch**: `077-one-signing-surface` (on `075-clear-signer-channel`)
**Created**: 2026-09-23
**Status**: Not started — design. One fix already landed separately (B-3/B-4).
**Input**: Owner, 2026-09-23, from using the Chrome extension against Uniswap:

> chrome extension 连接 dapp 签名时 UI 很差，我希望的是 侧边栏的钱包 UI 都在，
> 签名弹框是类似于 android ios 那样弹出来的，并且 UI 要保持一致性，现在都没有
> 速度切换
>
> dapp 的签名也需要和转账一样，有上链的那个等待动画效果吧
>
> 设置里的保存公钥到以太坊主网，这个签名时感觉也怪怪的和转账签名的体验差别很大
>
> 我觉得签名管线应该统一一下吧

## The finding: three reports, one cause

B-2, B-6 and B-7 in `specs/BACKLOG.md` read as three complaints about three
screens. They are one.

**The web wallet has two after-signing stories, and the dApp one stops early.**

| | a send | a dApp transaction |
|---|---|---|
| where it is answered | the wallet, in place | `<locale>/request.html`, a page of its own |
| after the signature | `SendReceipt` + `StatusHero`: spinner → clock with a filling ring → tick | `stage = { kind: 'done' }`, then `closeSoon()` **shuts the window 400 ms later** |
| what the person sees land | the transaction landing | nothing |

And B-7 follows without any new defect of its own:
`$lib/backup/ethereum-backup.ts` deliberately posts its call into **the dApp
seam** rather than building "a second, lesser copy of the most dangerous screen
there is" (its own words, and the right instinct). So it inherits exactly what
that seam is missing. Fix the seam and the backup is fixed with it.

B-3 and B-4 — no fee, no speed switch — were a different, narrower defect on the
same surface and are already fixed (`fee-calls.ts`, commit c2b0c5e4).

## What changes

### FR-001 · The side panel IS the wallet
Today `extension/panel.js` sends the panel to `request.html?rid=…`, a page whose
whole job is one request. The wallet is not beneath it; it was replaced.

The panel opens the WALLET, and a pending request raises the signing sheet over
it — as on Android and iOS, where the wallet never goes away.

**This does not weaken the rule it looks like it might.** `request/+page.svelte`
says the surface must not be "an in-page sheet the site could style, cover or
scroll". That is about a sheet inside the DAPP's page. The side panel is the
extension's own origin: the site cannot reach it, style it or scroll it.

### FR-002 · A dApp transaction lands where a send lands
After the signature, the surface shows the send flow's own treatment —
submitting, submitted with the chain's clock, confirmed — not a closing window.

The dApp already has its answer (the operation hash) the moment it is submitted;
what is being watched afterwards is the chain, and that is the person's business,
not the request's.

### FR-003 · The dedicated window lands too
A request a page fired with no user gesture still opens in its own window
(`?rid=`). It was going to keep today's ending — close on `done` — on the
reasoning that there is no wallet under it to return to.

**Driving the packaged extension changed that.** A window that shuts itself
about twenty seconds after the slide, with no word about what happened, is the
complaint whatever surface it is. The receipt is drawn in both, and "Done"
closes the window as before.

What the window does NOT get is a wallet behind it; that is FR-001's, and it is
why the panel is still worth moving.

### FR-004 · One pipeline, said as a rule
A transaction signed anywhere in the web shell — a send, a dApp request, the
Ethereum backup, a payment request — uses one surface: one fee quote, one speed
control, one signing sheet, one receipt. A feature that needs a transaction
signed posts it into that seam, as the Ethereum backup already does.

## The invariant this must not break

`extension/background.js`'s first rule: **a request is never left unanswered.**
An unanswered request is a dApp that hangs; a request answered 4001 when the
operation may already be at the bundler is a double spend.

Today the answer and the surface's life are tied together — the page closes
because the request is done. FR-002 unties them, so:

- the answer goes out when it goes out, and is NOT waited on by the receipt;
- the receipt's life is the wallet's, and closing it answers nothing;
- a panel torn down while a request is still owed settles with the core's code
  (4900 unknown-pending), exactly as now.

**This is the risk of the whole spec.** Every test that pins the answering
behaviour stays, and the receipt is built strictly on top of a request that has
already been settled.

## Success criteria

- **SC-001** In the side panel, a dApp request raises the sheet over the wallet;
  the wallet's own UI is still there behind it.
- **SC-002** A dApp transaction shows submitting → submitted → confirmed, with
  the same component a send uses.
- **SC-003** The Ethereum backup shows the same, with no code of its own.
- **SC-004** Every existing answering test still passes: cancel is 4001, a torn
  down surface is 4900, a request is never left unanswered.
- **SC-005** A dedicated request window (`?rid=`) still ends as it does today.

## Open — needs the owner

- **B-5, Arbitrum fails outright.** Still blocked on the error text. 42161 is a
  built-in chain (`network_admin.rs:234`), so it is not a missing network.
