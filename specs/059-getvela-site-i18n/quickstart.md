# Quickstart — validating 059

All commands run from `app-web/getvela.app`. `bun`, not npm.

## Prerequisites

```bash
cd app-web/getvela.app
bun install
```

## The three commands that must stay green

```bash
bun run check          # svelte-check: catches a key that does not exist in en.ts
bun run test:unit --run   # locale parity, message shape, hreflang reciprocity, sitemap
bun run build          # prerenders 210 localized pages + the English-only ones
```

`bun run build` is also the size gate: note its wall time and the page count it
prints. Research §9 expects ~210 localized pages; a build that produces
materially fewer means the `entries` generator lost a dimension, and one that
produces more means `/en/` came back.

## English is untouched (SC-005, FR-009)

```bash
bun run build && bun run preview     # wrangler dev
# then, in another shell:
for p in / /docs /docs/faq /about /roadmap /blog /privacy /terms; do
  curl -s -o /dev/null -w "%{http_code} %{redirect_url} $p\n" "http://localhost:8787$p"
done
```

Every line must read `200` with an empty redirect field. Any `30x` on an English
path is a regression, not an improvement.

## A locale actually renders, without JavaScript (SC-002, FR-011)

```bash
curl -s http://localhost:8787/ja/ | grep -o '<html lang="[^"]*"'
curl -s http://localhost:8787/ja/ | grep -c '真\|の\|ウォレット'   # non-zero
```

The copy must be in the HTML body itself. If the Japanese only appears after
hydration, FR-011 is not met, regardless of what the browser shows.

## The URL contract (contracts/routing.md)

```bash
curl -s -o /dev/null -w "%{http_code}\n" http://localhost:8787/en/        # 404
curl -s -o /dev/null -w "%{http_code}\n" http://localhost:8787/xx/        # 404
curl -s -o /dev/null -w "%{http_code}\n" http://localhost:8787/ja/blog    # 404
curl -s -o /dev/null -w "%{http_code} %{redirect_url}\n" http://localhost:8787/pt/   # 308 → /pt-BR/
curl -s -o /dev/null -w "%{http_code} %{redirect_url}\n" http://localhost:8787/pt-br/ # 308 → /pt-BR/
```

## Fallback behaves (SC-006, FR-017/FR-018)

Delete one translated doc, rebuild, and read the page:

```bash
mv src/content/docs/ja/faq.md /tmp/ && bun run build
grep -o 'hreflang="ja"' .svelte-kit/cloudflare/docs/faq/index.html   # nothing
# /ja/docs/faq must render: Japanese chrome + Japanese notice + English body
mv /tmp/faq.md src/content/docs/ja/
```

The two things to see: the notice is in Japanese (not English), and no page
advertises a Japanese alternate for that doc.

## Language switching keeps the page (SC-003)

Playwright, `bun run test:e2e`: from `/docs/networks-and-fees`, switch to 日本語,
land on `/ja/docs/networks-and-fees` — never on `/ja/` and never on `/`.

## The status report (FR-026, SC-008)

```bash
bun run i18n:status
```

Then add a sentence to `messages/en.ts`, re-run, and confirm all 14 locales are
listed as needing it — and that `bun run build` still succeeds (FR-027: an
English fix is never blocked by a missing translation).

## What a human still has to do

Nothing above proves the translations are *good* — only that they are present,
structurally sound and correctly annotated. The R7 review (FR-029) is a reading
pass per locale, recorded in `specs/059-getvela-site-i18n/reviews/<tag>.md`, and
no command can stand in for it. `zh` is the one locale the founder can sign off
directly (SC-007).
