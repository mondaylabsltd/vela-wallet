# Data model — 080

This feature has no runtime data. Its "entities" are the records the audit
produces and the metadata the site keeps so the next edit cannot silently
undo this one.

## 1. Claim (claim-ledger.md)

One fact that appears on at least one public surface.

| Field | Meaning |
|---|---|
| `id` | `C-<area>-<n>`, e.g. `C-keys-1` |
| `statement` | The canonical wording of the fact, in English |
| `value` | The number / list / rule, when there is one (e.g. `24`, `≤ 7`, `1-of-n`) |
| `evidence` | Code path with line, external URL, or decision record (spec/memory/doc) |
| `surfaces` | Every page (and README) that states it |
| `verdict` | `true` (surfaces already right) · `corrected` (≥1 surface fixed) · `removed` (the fact was false; claim deleted everywhere) |

**Rule**: a fact stated on more than one surface MUST have exactly one `value`
(FR-002). Changing a value means changing every surface in `surfaces` in the
same commit.

## 2. Finding (audit-report.md)

| Field | Meaning |
|---|---|
| `id` | `F-<n>` |
| `surface` | page / file and section |
| `category` | `false` · `stale` · `overstated` · `contradiction` · `evasive` · `unclear` · `unfindable` · `voice` (marketing/AI pattern) · `translation` |
| `severity` | **Fatal** — an expert would call it false and stop trusting the site · **High** — wrong in a way that costs a user money, keys or a decision · **Medium** — stale, imprecise or inconsistent without direct harm · **Low** — voice, clarity, polish |
| `why` | One paragraph: what is wrong and what a reader would conclude |
| `fix` | What the surface says now, or where it moved |
| `status` | `fixed` · `deferred (reason)` |

## 3. Service (self-hosting guide, research §2)

| Field | Meaning |
|---|---|
| `name` | Relay, public-key index, registry contract, chain data index, exchange rates, RPC, web app, signing page, site APIs |
| `default` | The URL / contract Vela ships |
| `used by` | Which shells, for what; what breaks without it |
| `replaceable` | `in-app setting` · `rebuild` · `not replaceable` · `not needed` |
| `code` | Repository and path |
| `requires` | Runtime, accounts, secrets, funds, domain |
| `verify` | How the reader confirms their instance works |

## 4. Locale review (reviews/<tag>.md)

| Field | Meaning |
|---|---|
| `locale` | One of the 13 re-aligned locales |
| `reference` | The en + zh text the locale was aligned to (by page / key) |
| `findings[]` | file, key or section, current text, type (误译 mistranslation / 不自然 unnatural / 文化风险 cultural risk / 术语不一致 terminology / UI 不合适 UI fit / 技术风险 technical), severity (High / Medium / Low), why, fix |
| `result` | `reviewed` once no High or Medium remains open |

`src/lib/i18n/review.json` flips a locale to `reviewed` only with a findings file
path (the existing test enforces this).

## 5. Doc fingerprint (new metadata)

A translated doc records which English text it was made from.

| Field | Where | Meaning |
|---|---|---|
| `source` | front matter of `src/content/docs/<tag>/<slug>.md` | first 12 hex chars of SHA-256 over the English file's bytes at the time the translation was finished |

State transitions per (locale, doc):

```
absent ──translate──▶ translated (no source) ──stamp──▶ current
current ──English edited──▶ STALE ──re-align + stamp──▶ current
```

`bun run i18n:status` counts STALE docs alongside STALE namespaces; `--gate`
fails on either.
