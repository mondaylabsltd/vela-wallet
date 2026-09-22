# Contract — docs information architecture

## Sidebar (after)

The grouping follows what the reader is trying to do. No slug changes, so no URL
breaks; one page is added.

| Group (key) | Pages, in order | Change |
|---|---|---|
| Getting started (`gettingStarted`) | Introduction · Install Vela · Create your wallet · Why we built Vela | "Why" moves after the task pages: a reader following the path wants to install next |
| Using Vela (`using`) | Send & receive · Networks & fees | — |
| Keys & recovery (`keys`, new) | How passkeys work · Signers & security keys · Recovery & sign-in | Split out of Security: these answer "what if I lose…", the most common question |
| Security (`security`) | Clear signing · The Bybit attack · The account contract · Audits & known issues | — |
| Run it yourself (`selfHost`, new) | **Self-hosting guide** (new, `/docs/self-hosting`) · Self-host the signing page | The ownership promise gets a group of its own |
| Reference (`reference`) | Whitepaper · FAQ | — |

New catalog keys: `chrome.docs.groups.keys`, `chrome.docs.groups.selfHost`,
`chrome.docs.titles['self-hosting']`, in all 15 locales (chrome must be
complete everywhere).

## Docs home (`/docs`, the Introduction page)

Opens with a short "what Vela is" and then **Find an answer** — a list of
questions a user actually arrives with, each a direct link:

| Question | Target |
|---|---|
| Which app should I install, and what does it cost? | `/docs/install` (+ `/get-started`) |
| How do I create a wallet? | `/docs/create-wallet` |
| I lost my phone / I deleted a passkey | `/docs/recovery` |
| Can I add or change keys later? | `/docs/signers` |
| Why does a transaction cost what it costs? | `/docs/networks-and-fees` |
| Is my chain supported? Can I add one? | `/docs/networks-and-fees`, `/chain-setup` |
| How do I use Vela with a dApp? | `/docs/install#dapps` |
| How do I check what I'm really signing? | `/docs/clear-signing`, `/docs/clear-signing-self-host` |
| What if getvela.app goes offline? | `/docs/self-hosting#if-getvela-app-disappears` |
| How do I run everything myself? | `/docs/self-hosting` |
| Is Vela audited? | `/docs/security-audits` |
| What is public about my wallet? | `/docs/create-wallet#what-is-public` |

## Stable anchors

Heading ids are generated from heading text, so they differ per locale. The few
anchors other pages link to are explicit `<span id="…"></span>` markers placed
in **every** locale's copy of the page, and a unit test checks they exist:

| Page | Anchor |
|---|---|
| `self-hosting` | `if-getvela-app-disappears`, `relay`, `index`, `chain-data`, `exchange-rates`, `web-app` |
| `install` | `dapps` |
| `create-wallet` | `what-is-public` |

## Cross-linking rules

- Every doc ends with a next step (existing pager plus a one-line pointer where
  the natural next page is not the next in the sidebar).
- A concept explained on another page is linked on first mention, not
  re-explained with different numbers.
- Facts stated on many pages (network list, key rules, fee formula) live in one
  place and the others link to it; where a page must restate one, it uses the
  claim ledger's wording.
- The landing page's fourth fact, the whitepaper's "If Vela disappears", the FAQ
  shutdown answers, the terms' "Service availability" and the README point to
  `/docs/self-hosting`.
- The footer's "Infrastructure" column links to the guide and to chain setup
  rather than to a mixture of repositories and live instances.
