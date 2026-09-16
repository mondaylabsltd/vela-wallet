# R7 review — `zh-HK` (Cantonese, Hong Kong)

Date: 2026-09-15 · Method: [spec §R7](../spec.md) · Reference frame: **English
and Chinese** (founder, 2026-09-15).

**Scope: the four page namespaces** — `home`, `about`, `roadmap`, `getStarted`.
The sixteen docs were translated into `zh-HK` afterwards (T056/T057, 2026-09-16)
and have NOT been through this reading pass; they are drafted, not reviewed.

## Findings

### zh-HK-1 — `chrome.docs.titles.passkeys` — consistency — **Medium** — fixed

`密碼金鑰點運作` used 密碼金鑰, the **Taiwanese** term, in a file that says
通行密鑰 thirty-two times and is otherwise written in Cantonese. R7 names this
locale explicitly: a Hong Kong reader must not be quietly served Taiwanese
wording. Fixed to `通行密鑰點運作`.

### zh-HK-2 — `home.signing.next.body` — ambiguity — **Low** — fixed

冇打包器 → 冇構建工具, for the same reason as ZH-2: "bundler" here is the build
tool, and 打包器 can be heard as the ERC-4337 role on a page that has one.

This locale's four page namespaces were written in this session against the
current English, so the drift that affected the nine older locales (a signing
section still carrying the retired "sign what you see" claim) never applied here.
Paragraph structure, href sets and tag counts match — asserted, not eyeballed.

## Axes

**Accuracy.** The honesty posture survives: no audit promised, alpha stated,
and the 1-of-n caveat kept blunt — an extra key is a way back in, not a second
lock.

**Naturalness and register.** One register throughout, chosen for what the
market's own product copy uses — written Cantonese (呢版 / 唔使 / 我哋), not Traditional Mandarin, per the corpus rule for this locale.

**Consistency.** One term per concept inside the locale.

**Technical.** Links, tags and paragraph structure asserted by
`messages.test.ts`.

## State

`review.json` → stays **`drafted`**. No High or Medium is open, but no native
speaker of Hong Kong Cantonese has read it. That is the whole distinction between `drafted`
and `reviewed` here.
