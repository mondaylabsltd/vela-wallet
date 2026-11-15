# getvela.app

The Vela Wallet website and its small API: the landing page, docs, blog and
notes in fifteen languages, the download links, the on-chain wallet registry,
and the endpoints the apps use to file bug reports and fetch exchange rates. It
is a SvelteKit 3 app that runs as one Cloudflare Worker, `getvela`.

This is not the wallet itself. The web wallet is `app-web/vela-wallet`, served
at wallet.getvela.app, and it is deployed separately. Deploying this site never
changes how a wallet signs or sends a transaction.

## Quick start

You need [Bun](https://bun.sh) 1.4.2, the version CI pins. No keys or accounts
are needed to develop.

```sh
cd app-web/getvela.app
bun install
bun run dev          # http://localhost:5173
```

The hashing and signing module (`rust/pkg-web-site-crypto`) is committed, so
no Rust toolchain is needed either, unless you change `rust/vela-site-crypto`
(see [Outside this folder](#outside-this-folder)).

## What runs where

Most of the site is static. It is rendered to HTML at build time and served as
files:

- everything under `src/routes/[[locale=locale]]/`: the landing page, about,
  roadmap, get-started, chain-setup and docs, in every locale. English is at
  `/`, other locales add a prefix (`/ja/`, `/pt-BR/docs/faq`).
- `blog/`, `notes/`, `sitemap.xml` and the blog's `rss.xml`.

The rest is rendered by the Worker on each request:

| Path                                               | What it does                                                                                                |
| -------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| `/registry`                                        | Lists the wallets recorded on chain. It reads the chain from the browser; the Worker only renders the page. |
| `/privacy`, `/terms`, `/support`, `/delete`        | Legal and support pages.                                                                                    |
| `/download/<platform>`                             | Serves the latest release from the R2 mirror, filling it from GitHub Releases on the first request.         |
| `/pay`                                             | Redirects payment links to `https://wallet.getvela.app/pay`.                                                |
| `/.well-known/*`                                   | `assetlinks.json`, `apple-app-site-association` and `security.txt`, for the Android and iOS apps.           |
| `/api/downloads`                                   | The latest release's version and files.                                                                     |
| `/api/exchange-rate`                               | USD exchange rates from frankfurter.app, cached for an hour.                                                |
| `/api/bug-report`, `/api/bug-report/attachments/*` | Files a GitHub issue from the apps; screenshots go to R2.                                                   |
| `/api/peerconnect-token`                           | Issues PeerConnect tokens to the apps.                                                                      |

`src/hooks.server.ts` adds the security headers to every Worker response. For
static files the same headers come from `_headers`, and
`security-headers.test.ts` fails if the two lists differ.

## Project layout

```
src/
  routes/              pages and endpoints (see the table above)
  content/
    docs/              the docs, one .md per page; translations in docs/<locale>/
    blog/              blog posts
    notes/<module>/    short technical notes; drafts show only under `bun run dev`
  lib/
    components/        shared Svelte components
    i18n/              locales, URL rules and UI strings (messages/en.ts is the source)
    evm/               the site's own EVM client, ABI coding and the wasm loader
    server/            code that only runs in the Worker (downloads, bug reports, tokens)
    content/           reading notes and building the social-preview card list
  env.ts               the Worker's secrets and variables (see Configuration)
  params.ts            the `locale` route matcher
  hooks.server.ts      locale redirects and security headers
  cloudflare-workers.d.ts  types for the `cloudflare:workers` module
scripts/               i18n tools, Markdown heading ids and highlighting, OG image rendering
e2e/                   Playwright tests, run against a production build in the local Worker
static/                icons, manifest, robots.txt
_headers               headers for static files
vite.config.ts         Vite and SvelteKit config, mdsvex, notes index, OG images
wrangler.jsonc         the Worker: name, assets and the R2 binding
```

The `.md` files in `src/content` are compiled as Svelte components by
[mdsvex](https://mdsvex.com), so they can import components (for
example `#lib/components/Callout.svelte`). Code blocks are highlighted at build
time and ship no JavaScript.

Social-preview images (`/og/<page>.png`) are drawn by `rust/vela-og` and
written as static files during the build. Under `bun run dev` they are drawn
on request.

## Commands

| Command                       | Use it to                                                                                 |
| ----------------------------- | ----------------------------------------------------------------------------------------- |
| `bun run dev`                 | Develop. Vite dev server with hot reload.                                                 |
| `bun run preview`             | Build, then serve the build in the local Worker (`wrangler dev`). Closest to production.  |
| `bun run check`               | Type-check Svelte and TypeScript.                                                         |
| `bun run lint`                | Prettier and ESLint.                                                                      |
| `bun run format`              | Format everything with Prettier.                                                          |
| `bunx vitest run`             | Unit tests, both projects: `server` (Node) and `client` (Chromium).                       |
| `bun run test:e2e`            | Playwright tests. Builds the site and serves it with `wrangler dev` first.                |
| `bun run test`                | Unit tests, then e2e.                                                                     |
| `bun run deploy`              | Unit tests, build, then `wrangler deploy` to production. See [Deploying](#deploying).     |
| `bun run i18n:status`         | Translation coverage per locale, and what is STALE.                                       |
| `bun run i18n:stamp`          | Mark every UI-string translation as matching the current English.                         |
| `bun run i18n:stamp:doc <f>…` | Mark the named translated docs (for example `zh/faq.md`) as matching the current English. |
| `bun run og:fonts`            | Rebuild the font subsets the OG images use.                                               |

The browser tests need Chromium once: `bunx playwright install chromium`.

Before opening a PR, run `bun run check`, `bun run lint` and `bunx vitest run`.
CI (the `site` job in `.github/workflows/ci.yml`) runs `check` and the unit
tests, and they block the merge. Also run `bun run test:e2e` if you changed
routing, locales or anything the Worker renders.

## Configuration

The site needs no configuration to build or run. Every variable is optional,
declared once in `src/env.ts` and read through `$app/env/private`. None of them
reach the browser.

| Variable                 | Used by                  | When it is not set                                                     |
| ------------------------ | ------------------------ | ---------------------------------------------------------------------- |
| `GITHUB_BUG_TOKEN`       | `/api/bug-report`        | Answers `not_configured`. The apps then open a prefilled GitHub issue. |
| `GITHUB_BUG_REPO`        | `/api/bug-report`        | `mondaylabsltd/vela-wallet`.                                           |
| `PEERCONNECT_APP_SECRET` | `/api/peerconnect-token` | Answers 503 `not_configured`.                                          |
| `PEERCONNECT_APP_ID`     | `/api/peerconnect-token` | The id built into `src/lib/server/peerconnect-token.ts`.               |

Where to put them:

- `bun run dev`: a `.env` file in this folder.
- `bun run preview` and e2e: a `.dev.vars` file in this folder.
- production: `bunx wrangler secret put <NAME>`.

`.env` and `.dev.vars` are git-ignored. Only create them to test the real
GitHub or PeerConnect call; the "not configured" answer is a path worth
testing too.

### Cloudflare bindings

The one binding is the R2 bucket `vela-downloads`, bound as `DOWNLOADS`. It is
the download mirror, and it also stores bug-report screenshots. Server code
reads it with `import { env, waitUntil } from 'cloudflare:workers'`. In
`bun run dev` that module is backed by a local, empty bucket.

The types for `cloudflare:workers` are written by hand in
`src/cloudflare-workers.d.ts`. Do not generate them with `wrangler types`: its
output declares the Workers runtime globally, which replaces the browser's DOM
types (`Response.json()` stops type-checking, for example). If you add a
binding, add it to `wrangler.jsonc` and to that file.

Create a new bucket before the binding that uses it is deployed: `wrangler
deploy` fails on a binding to a bucket that does not exist.

## Deploying

The site deploys by hand from a developer machine. Merging to `main` does not
deploy it, and it is not part of the app release (`docs/RELEASING.md`).

You need a Cloudflare login with access to the account that owns the `getvela`
Worker (`bunx wrangler login`).

1. Check out an up-to-date `main` with a clean tree. What you deploy is your
   working copy, not a commit.
2. Run the checks: `bun install --frozen-lockfile && bun run check && bun run lint`.
3. Deploy: `bun run deploy`. It runs the unit tests, builds, and runs
   `wrangler deploy`. It stops if a test fails.
4. If a secret is new or has been rotated, set it: `bunx wrangler secret put GITHUB_BUG_TOKEN`.
5. Check the live site:

   ```sh
   curl -sI https://getvela.app/ | head -1                          # 200
   curl -sI https://getvela.app/registry | head -1                  # 200
   curl -s "https://getvela.app/api/exchange-rate?currency=CNY"     # rates
   curl -s https://getvela.app/api/downloads | head -c 300          # current version
   curl -s https://getvela.app/.well-known/apple-app-site-association
   ```

   Then open `/registry` in a browser and check that wallets load.

To roll back, run `bunx wrangler rollback`, or deploy the previous commit.

The project-wide runbook is `docs/project-takeover/05-deployment-runbook.md`.

## Things that are easy to get wrong

- **POSTs to the API must send a JSON `Content-Type`.** SvelteKit 3 rejects a
  cross-origin POST that has no `Content-Type` or a form content type, with 403. All of the apps send `application/json`; keep it that way in new
  clients.
- **Server code must not compute anything from `import.meta.url` at import
  time.** In the Worker it is not a URL, and the error takes down every page
  that imports the module. `src/lib/evm/wasm.ts` resolves its wasm URL only
  when it is first loaded, for this reason.
- **Paths passed to `resolve()` have no leading `/`**, except route IDs with
  parameters: `resolve('docs/faq')`, `resolve('')` for the home page, but
  `resolve('/download/[platform]', { platform })`.
- **Internal links go through `resolve()`.** Localized ones use
  `localHref(locale, path)` and `switchHref` from `#lib/i18n/links.ts`, never
  `pathFor` directly; ESLint (`svelte/no-navigation-without-resolve`) fails on
  a bare internal `href`. Links to other sites carry `rel="external"`.
- **No web fonts.** The site uses system fonts only (`docs/FONT-POLICY.md`):
  `--font-ui` and `--font-mono` in `src/lib/styles/tokens.css`. A downloaded
  font makes the text change face after the page appears, and the CSP
  (`font-src 'none'`) blocks one anyway.
- **Import from `#lib/…` with the file extension**: `#lib/i18n/urls.ts`,
  `#lib/components/Seo.svelte`. `#lib` is a Node subpath import declared in
  `package.json`.
- **Changing an English doc makes its translations STALE**, even when the
  change is only an import line. If the meaning did not change, re-stamp the
  translations that were up to date with `bun run i18n:stamp:doc`. Do not run
  `i18n:stamp` for this: it marks every translation as up to date.

## Outside this folder

- `rust/vela-site-crypto`: Keccak-256 and secp256k1 for `src/lib/evm`, built to
  `rust/pkg-web-site-crypto` and committed. After changing it, rebuild with
  `node rust/scripts/build-web-site.mjs site-crypto` from the repository root
  and commit the output.
- `rust/vela-og`: draws the social-preview images.
- `app-web/vela-wallet`: its `SUPPORTED_LOCALES` must equal this site's locale
  list, and a test checks it.

## Localization

The site is served in fifteen locales. English lives at the unprefixed paths
(`/`, `/docs/faq`); every other locale is a prefix (`/ja/`, `/pt-BR/docs/faq`).
The full design is in `specs/059-getvela-site-i18n/`.

### The three rules worth knowing before you touch it

1. **Fallback is per page, not per key.** A page namespace is used only when it
   is complete; otherwise the whole namespace renders English under a notice in
   the reader's own language. A key-by-key merge would ship a page with a
   Japanese heading and an English third paragraph, and nothing would catch it.
2. **`chrome` and `notice` must be complete in every locale.** They are the
   frame an untranslated page is drawn in — including the sentence that says the
   page is untranslated, which cannot itself fall back.
3. **`hreflang` advertises only what exists.** A falling-back page is an English
   page at a localized URL; listing it as a translation is what earns a
   duplicate-content penalty. `src/lib/i18n/urls.ts` is the single source the
   router, the head and the sitemap all read.

### Adding a string

1. Add it to `src/lib/i18n/messages/en.ts`. That file is the shape: `bun run
check` will fail on any component reading a key that does not exist.
2. Ship it. A missing translation never blocks an English change (FR-027) — the
   locales that lack the new key fall back for that page and say so.
3. When a locale is translated, run `bun run i18n:stamp` so the English it was
   translated from is recorded.

### Adding a translation

1. Edit `src/lib/i18n/messages/<tag>.json`. Translate link TEXT, never the
   `href`; keep `<a>`, `<strong>` and `<em>` exactly as the English has them —
   `messages.test.ts` fails if the href or tag set differs.
2. A page namespace is all-or-nothing: a partial one is a test failure, not a
   partial translation. Leave it out entirely instead.
3. `bun run i18n:stamp`, then `bun run i18n:status` to see where things stand.

### Translating a doc

Each translated doc in `src/content/docs/<locale>/` records, in its `source:`
frontmatter, a fingerprint of the English file it was translated from. After
translating or updating one, run `bun run i18n:stamp:doc <locale>/<file>.md`
for exactly the files you finished.

### Adding a locale

The locale list must equal `app-web/vela-wallet`'s `SUPPORTED_LOCALES` — a test
reads that file off disk and fails on any divergence. Changing the set means
changing the app's too, deliberately, in both places.

### Commands

```bash
bun run i18n:status   # per-locale coverage, and what is STALE
bun run i18n:stamp    # record the English a translation was made from
bun run check         # catches a key that does not exist
bun run test:unit     # shape, chrome completeness, href/tag integrity, hreflang
```

**STALE** means the English changed after a translation was written: the page
still renders, but it now says something the English no longer says. It is the
one state in the report that means something is wrong rather than unfinished.
