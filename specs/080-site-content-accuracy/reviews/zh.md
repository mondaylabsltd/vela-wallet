# zh — reference text, written with the English (spec 080)

Date: 2026-09-22 · Author/reviewer: Claude (Opus 5) · Method: en + zh finalized
together as the reference (founder, 2026-09-22), then read against each other
page by page.

Simplified Chinese is not a translation in this spec: every changed page was
written in English and Chinese side by side from the claim ledger, so that the
two say the same thing and the other 13 locales can take their shared meaning.

## Terminology and register

| Concept | zh | Note |
| --- | --- | --- |
| key (any signing credential) | 钥匙 | umbrella term, as the app uses it |
| passkey | 通行密钥 | 059 choice, kept |
| security key | 安全密钥 / 硬件安全密钥 | replaces the mixed 安全钥匙 in the old catalog |
| relay | 中继 | |
| registry | 注册表 | the on-chain contract |
| public-key index | 公钥索引 | the service |
| self-hosting | 自托管 / 自己运行 | guide title 自托管指南 |
| signing page | 签名页 | |
| clear signing / blind signing | 清晰签名 / 盲签 | |
| fallback handler | fallback handler | left in English, as developers say it |

Register: plain 你, unchanged from 059. Product names, contract names, EIP/ERC
numbers, commands and addresses stay in English.

## Findings in the old zh (all fixed)

| # | Where | Before | Type | Severity | Why | After |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | All docs, landing | 12 条网络 | mistranslation (stale fact) | High | the wallet has 24 | 24 条网络 |
| 2 | 网络与手续费, FAQ, 白皮书 | gas 账户激活押金 | mistranslation (stale fact) | High | mechanism no longer exists | 金库补充（自愿、不退还） |
| 3 | 创建钱包, 恢复, 白皮书 | 单一通行密钥、只由 iCloud/Google 同步 | mistranslation (stale fact) | High | 1–7 keys of three kinds | 按 C-keys-1/2 改写 |
| 4 | 账户合约 | 任何兼容 Safe 的界面都能操作它 | mistranslation (false) | High | rpId binding | 能读取、能构造交易；签名需要能请求 getvela.app 签名的软件 |
| 5 | 白皮书 | Vela 服务器被攻破不会带来签名能力 | mistranslation (false) | High | app delivery can present a malicious transaction | 威胁模型区分后端服务与 App 分发 |
| 6 | 首页 FAQ | 通过零依赖的清晰签名扩展签名 | mistranslation (false) | High | the signing page can't operate a wallet and isn't published | 扩展或自编译 App |
| 7 | 首页 FAQ #1 | 安全钥匙 | terminology | Medium | inconsistent with 安全密钥 elsewhere | 安全密钥 |
| 8 | 关于 | 加密资产丢失最主要的原因 | cultural/tone (unsourced superlative) | Medium | unsupported claim | removed |
| 9 | 关于 | 你的密钥，你的币——这不是一句口号 | UI fit / tone | Low | slogan voice | plain statement |
| 10 | 路线图 | 地址簿、无需手机的 DApp Connect、审计列为计划 | mistranslation (stale) | High | shipped / dropped / not scheduled | rewritten from shipped work |

## Open items

None. The zh landing hero headline and subtitle are the 059-approved strings and
were not touched.

## Result

reviewed — no open High or Medium findings
