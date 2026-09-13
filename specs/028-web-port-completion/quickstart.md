# Quickstart — 028 Web Port Completion

## Gates (unchanged order)
`pnpm check` → `pnpm lint` → `pnpm test:unit -- --run` → `pnpm build` →
`pnpm build:extension` → `pnpm test:e2e` + repo-root `gen-core-types --check`.
Corpus changes follow the 5-step process.

## The one that matters most
**Decode what you rendered.** The receive code is asserted by a round trip: the
rendered image goes through the same decoder the scanner uses and must return
the address. A test that asserts a QR appeared is the test that let a
decorative pattern ship as a receive code.

## Hermetic scenarios (CI)
1. **Receive code (SC-401)** — render for a known address, decode, compare. With
   a payment request set, the amount, asset and chain survive the round trip.
2. **Scan (SC-402)** — a generated code image is picked from the file input and
   read; a refused camera states its reason instead of showing a dead viewfinder.
3. **Preferences (SC-406)** — each row changes the app and survives a reload,
   ×3 engines.
4. **Erase (SC-407)** — after a confirmed erase, no `vela.` key remains except
   the named exceptions and the app is at first run; cancel changes nothing.
5. **Sweep (SC-404)** — two assets on one chain, one address, ONE operation.
6. **Add token (SC-405)** — a contract address becomes an asset that survives a
   reload and can be sent.
7. **Contacts I/O (SC-408)** — export, re-import, nothing changes; import over an
   existing entry leaves it untouched and says what it skipped.
8. **Desktop send (SC-409)** — a send completes at desktop width.

## Budgets
The decoder's wasm is lazy: a normal visit, and every startup chunk, must be
free of it — asserted the way 026 asserted SheetJS. The core artifact stays
byte-identical and remains the only one a wallet route loads.

## The device pass (before results.md closes)
Scan a real code with a real camera on a phone, and scan a photo of one. The
preprocessing ladder in `docs/qr-scanner-web.md` was measured on real hardware;
this confirms the port kept it.
