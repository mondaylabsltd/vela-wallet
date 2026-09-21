---
title: Self-host the signing page
description: The zero-dependency page and Chrome extension that decode a transaction themselves and sign it with your passkey — how to run your own copy, and which copy can sign for your wallet.
---

# Self-host the signing page

Vela decodes every transaction before you approve it, and that decoding is
honest work — but it is work done by the same app that built the transaction.
If the app or the way it reaches you is tampered with, it can show you one
thing and sign another. That is exactly what happened to
[Bybit](/docs/bybit-attack).

The signing page exists to split that in two: the transaction comes from one
place, and the check and the signature happen somewhere you control.

**Status:** built and tested locally; **not published**, and **no Vela app sends
requests to it yet**. Today it is something to read, run and try with the demo
requesters in its `samples/` folder. Using it for real signatures needs the apps
to route their requests to it, which is still to be built.

## What it is

One folder — `app-web/clearsigning` in the repository — that is both a web page
and a Chrome extension. Pure HTML, CSS and JavaScript: no framework, no
bundler, no build step, no dependencies, and no data fetched from a server —
its only request is for decorative token logos.

Given a signing request it does not trust the summary that came with it. It
decodes the raw calldata itself, computes its own digest, shows you what the
signature will actually authorise, and only then asks your passkey.

Because there is no build step, the files you read are the files that run. You
can diff the folder against the repository and know what you are serving.

## Which copy can sign for your wallet

A passkey is bound to the domain it was created on. Your Vela keys are
registered under `getvela.app`, and a browser will only offer them to a page
whose relying party is `getvela.app`. That single rule decides which way of
running your own copy is useful to you.

**As a Chrome extension — the one to use with your existing wallet.** The
extension's relying party is `getvela.app` regardless of where the folder came
from, so your existing keys can sign in it, while the code is the folder you
loaded and inspected.

1. Get the folder: `git clone https://github.com/mondaylabsltd/vela-wallet`
   (it is `app-web/clearsigning` inside).
2. Open `chrome://extensions` and turn on **Developer mode**.
3. **Load unpacked**, and pick the `app-web/clearsigning` folder.
4. The toolbar icon opens the page in a tab.

**As a page on your own domain, or on localhost.** Served over HTTP(S), the
page's relying party is its own hostname — so it can sign with keys registered
under _that_ hostname, not with keys registered under `getvela.app`. That makes
it the right way to try the whole ceremony end to end, to run the desktop flow,
and to sign for a wallet whose key was created on your own domain. It is not a
way to sign for an existing `getvela.app` wallet.

```sh
cd app-web/clearsigning
python3 -m http.server 8080   # → http://localhost:8080
```

Every path in the app is relative, so a subdirectory on an existing host works
too, and opening `index.html` straight off disk (`file://`) works for a look
around — with no origin, there is no relying party and nothing can be signed.

## What it does before it signs

- **It decodes the transaction itself.** What the call does, to whom, and for
  how much, from the calldata — including calls nested inside a batch.
- **It signs only a digest it computed.** EIP-191, EIP-712, SafeOp and
  SafeMessage digests are computed in the page and cross-checked against
  `vela-core`, the same code the wallet uses. A digest it cannot compute is a
  refusal, not a signature.
- **It checks the transaction is the one that was requested.** The call the
  site asked for has to actually be inside the operation being signed.
- **It refuses an unlimited approval.** Not a warning — a refusal, with a
  pointer to what to do instead.
- **It says when it cannot read something,** instead of showing a friendly
  summary it cannot stand behind.
- **It shows the account's address and identicon,** and does not show a
  recipient name supplied by whoever asked for the signature. Anything the
  requester controls is either dropped or labelled as theirs.

## What it deliberately does not have

- **No editors.** The request is fixed when it arrives: you sign it or you do
  not. A fee picker or an allowance editor would rewrite the calldata, which is
  the disease this page exists to prevent.
- **No key creation.** The signing page cannot create a passkey. Creating one
  would be creating a different account.
- **No data from the network.** Nothing it shows or signs is fetched. The only
  thing it loads is token logos, as images, from Vela's chain-data server; if
  they fail, a letter takes their place.

## How a request reaches it

| Requester                                    | Channel                                                                    |
| -------------------------------------------- | -------------------------------------------------------------------------- |
| A page in the same browser                   | `postMessage`                                                              |
| A page in the same browser, to the extension | Extension port                                                             |
| A desktop app on the same machine            | URL fragment + loopback callback (demo in `samples/`; the Vela desktop app doesn't use it yet) |
| A phone or another computer                  | Bluetooth LE (protocol implemented; radio not yet tested on real hardware) |

The wire format, the digests, and a table of where every item on the screen
comes from are in `PROTOCOL.md` beside the code.

## Where it fits

Once the apps can hand their requests to it, the intended use is simple: from
the day the account holds money you would mind losing, every signature goes
through a page whose code you loaded yourself. Not only for large amounts — a
small approval can hand over enough to empty an account. Until then, the page is
a way to read and test exactly how that second opinion will work.
