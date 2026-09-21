# Quickstart — validating 080

Run from `app-web/getvela.app` unless stated.

## 1. Gates the site already has

```bash
bun install
bun run check                 # a component reading a key that does not exist fails here
bun run test:unit -- --run    # catalog shape, chrome completeness, href/tag parity,
                              # audit posture, docs front matter, stable anchors (new)
bun run i18n:status --gate    # fails on any STALE namespace or doc (docs are new in 080)
bun run build                 # prerenders every locale × page
```

Expected: all green; `i18n:status` shows every locale with 22 pages (17 docs + 5
pages), zero stale.

## 2. Shared facts have one value

The claim ledger lists the facts that appear on several surfaces. For each, a
search must return only the ledger's value, in every locale and the README. For
example, the network count:

```bash
cd ../..   # repo root
rg -n "\b(8|11|12|24|25)\b.{0,20}(networks|chains|网络|ネットワーク)" \
  README.md app-web/getvela.app/src/content app-web/getvela.app/src/lib/i18n/messages
```

Expected: every hit is the ledger value. Repeat for the key maximum, the fee
formula, and the contract list (see claim-ledger.md "check" column).

## 3. No broken internal links

With the dev server running (`bun run dev -- --port 5288`), crawl every internal
link from `/`, `/docs`, each doc, `/zh/docs`, `/ja/docs` and the footer; every
target must answer 200 and every `#anchor` must exist on the target page.
(A small crawler script lives in the implementation notes of results.md.)

## 4. Look at it

Screenshot, at 1440 px and 390 px, in `en` and `zh` at least: `/`, `/docs`,
`/docs/self-hosting`, `/docs/install`, `/get-started`, `/roadmap`. Read each
screenshot before calling the page done.

## 5. The self-hosting guide, walked

Follow `/docs/self-hosting` from the top. Steps that run locally (the signing
page, the exchange-rate service, the web app build) are run; steps that need a
cloud account, a domain or funds are checked against the service's code and
README. results.md records which was which.

## 6. The expert read

An adversarial reviewer who did not write the copy reads the landing page,
whitepaper, account contract, audits, passkeys, signers, recovery, fees, FAQ and
self-hosting guide and lists anything false, overstated or evasive. Expected:
nothing left open.
