# Vela Connect examples

Working references for [the protocol](../PROTOCOL.md). There is no package to install: read these, copy what you
need.

| File | What it shows |
|---|---|
| [`vela-connect.js`](vela-connect.js) | The protocol written out once: popup mode (`VelaPopup`: open in the click, wait for `ready`, resend on `ready`, one answer per id, what a closed window means) and redirect mode (`redirectToVela`, `takeVelaAnswer` with its `state` check). No dependencies |
| [`index.html`](index.html) | A page using it: connect, sign a message, Sign-In with Ethereum, switch chain, send a batch and follow its status, connect by redirect |

Run it against your local Vela:

```sh
# the wallet (app-web/vela-wallet)
bun run dev                       # http://localhost:5173
# this folder, on another origin
npx serve docs/vela-connect/examples -l 8815
# then open
http://localhost:8815/?wallet=http://localhost:5173
```

These files are the ones Vela's own end-to-end suite drives (`app-web/vela-wallet/e2e/vela-connect.e2e.ts`), so
they keep working as Vela changes.
