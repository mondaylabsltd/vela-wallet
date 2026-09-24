# Contract — translation alignment

## Reference text

English and Simplified Chinese are finalized **together** and are the reference
(founder, 2026-09-22). They must agree in meaning; where one says more, the other
is brought up to it. Neither is a word-for-word template for the others: a locale
takes the meaning they share, in the phrasing a native product team would use.

## Order of work, per page

1. English and Chinese written and checked against the claim ledger.
2. Each of the 13 locales re-aligned to that meaning (zh-TW, zh-HK, ja, ko, vi,
   id, tr, es-MX, pt-BR, fr, de, ru, it). A regional variant is judged on its own
   market (zh-TW ≠ zh-HK ≠ zh; es-MX, pt-BR) — never a copy of its sibling.
3. The locale is reviewed on five axes — accuracy, naturalness, cultural and tone
   risk, terminology consistency, technical integrity — and findings recorded in
   `specs/080-site-content-accuracy/reviews/<tag>.md`.
4. High and Medium findings fixed; worthwhile Low findings fixed.
5. `bun run i18n:stamp` records the English each translation was made from.

## Invariants (machine-checked)

- Catalog shape equals English; `chrome` and `notice` complete in every locale.
- Every `href`, `<a>`, `<strong>`, `<em>` in a catalog value is preserved exactly.
- Page namespaces are whole or absent.
- A translated doc keeps every link target, code block, table column count,
  component tag (`<Callout …>`, `<script>`) and front-matter key of the English.
- Qualifiers are facts: "up to", "by default", "not necessarily", "where
  supported", "today" must survive translation.
- The audit posture never softens (existing test) — and, in prose, "not audited"
  may not become "audit pending".

## Terminology (established; keep)

`passkey`: zh 通行密钥 · zh-TW 密碼金鑰 · zh-HK 通行密鑰 · ja パスキー · ko 패스키 ·
tr geçiş anahtarı · others `passkey`. Proper nouns stay: Safe, ERC-4337,
EntryPoint, RIP-7212, Vela Relay, GitHub, MetaMask, Base Account.
New terms introduced by this feature (self-hosting, relying party / rpId,
security key, signer) take the form each locale's existing docs already use; the
review records the choice.

## Scope

Localized: landing, get-started, about, roadmap, chain-setup copy, chrome, all
docs including the new self-hosting guide. English only: blog, privacy, terms
(spec 059 R5).
