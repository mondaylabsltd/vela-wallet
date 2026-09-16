# R7 review — `zh` (简体中文)

Date: 2026-09-15 · Reviewer: this session · Method: [spec §R7](../spec.md)
**Scope: complete.** The four page namespaces *and* all sixteen docs — `zh` is
the only locale where the docs exist, so it is the only one where a full-corpus
pass was possible.

Reference frame: English source. `zh` is itself one of the two reference
languages the founder reads, so this file is also the baseline the other
fourteen are judged against.

## Findings

### ZH-1 — `home.seal.verify` — terminology — **Medium** — fixed

`每一个钱包都在链上 — 查看注册表`

注册表 is the Windows registry. An on-chain list of wallets is 登记 / 登記 —
which `zh-HK` had already written correctly, so this was also an inconsistency
across the three Chinese locales.

Fixed: `每一个钱包都在链上 — 查看链上登记`. Same fix applied to `zh-TW`
(註冊表 → 鏈上登記) and to two occurrences in `docs/zh/security-audits.md`.

### ZH-2 — `home.signing.next.body` — ambiguity — **Low** — fixed

English: *"No framework, no bundler, no network requests."* Here "bundler" is the
build tool. Rendered as 打包器 on a page that uses 中继 for the ERC-4337 role,
a reader can still hear the 4337 bundler. Changed to 构建工具 (and
構建工具 in `zh-HK`), which cannot be misread.

### ZH-3 — docs internal links — technical — **Medium** — prevented, not found

Markdown bodies are not passed through the catalog's link localizer. A
translated doc has to write `/zh/docs/passkeys` itself; writing `/docs/passkeys`
silently walks a Chinese reader back into English and breaks nothing that a test
would see. All sixteen `zh` docs were written with the prefix, and this is now
recorded in the commit for T049 so the next locale does not learn it the hard
way.

## Axes with no findings

**Accuracy / honesty posture.** The three claims that must never soften all
survive: 没有排期 for "no audit is scheduled" (not "planned"), 内测 for alpha,
and the 1-of-n caveat — 多加的钥匙是一条回得来的路，不是第二道锁 — which is as
blunt in Chinese as in English. `messages.test.ts` asserts the audit claim
mechanically in every locale; the other two were read.

**Naturalness.** Reads as written, not translated: no English clause order
carried over, 的-strings kept short, and the em-dash-heavy English rhythm
converted to 破折号 or split sentences where Chinese would not use one.

**Consistency.** 通行密钥 33× and no competing term; 中继 for the relay
throughout, including the docs; 自托管 for self-custodial; 清晰签名 for clear
signing. Checked by counting, not by impression.

**Technical.** Every `href` and inline tag matches English —
`messages.test.ts` fails otherwise — and paragraph structure now matches too,
under the gate added the same day.

## State

`review.json` → **`reviewed`**. No open High or Medium.

Worth saying plainly: this pass was run by the same session that wrote much of
the Chinese, which is the weakest kind of review there is. The founder reads
Chinese and is the real check. Nothing here should be read as a substitute for
that.
