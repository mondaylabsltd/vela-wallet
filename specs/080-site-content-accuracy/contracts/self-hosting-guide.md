# Contract — the self-hosting guide (`/docs/self-hosting`)

The page that proves the landing page's fourth claim. Every statement traces to
[research.md](../research.md) §3; every value to the [claim ledger](../claim-ledger.md).

## Required sections, in order

1. **What "without us" means** — one paragraph: your funds are in a Safe on-chain;
   what Vela runs by default is convenience and liveness; here is each piece and
   how to replace it.
2. **The map** — one table, every piece of infrastructure the wallet uses by
   default: relay, public-key index, registry contract, chain data, exchange
   rates, RPC, AAGUID directory, selector lookups, phone-sign-in tunnel, the web
   app and getvela.app itself. Columns: what it does · default · replaceable how ·
   what breaks without it.
3. **What you cannot replace: the passkey domain** — the rpId rule in plain
   words; a copy of the web wallet on your domain is a different wallet; the
   anchor `if-getvela-app-disappears` sits here, listing every way to keep
   signing for an existing wallet, by key type:
   - Vela browser extension (release zip or build from source) — all key types
     the browser can reach;
   - self-built desktop or phone app — phone by QR and USB security keys; not
     "this device" passkeys;
   - the signing page (`app-web/clearsigning`) is **not** a way to operate a
     wallet on its own: it signs requests that another program sends it, and no
     Vela app sends them yet.
   Also: a domain that changed hands could request signatures for these passkeys
   (the prompt does not show the transaction) — why the extension and self-built
   apps matter.
4. **Point the wallet at your own services** — where the setting is on each
   platform, what each field expects, the health check it runs, and the honest
   per-platform table (iOS page not wired; Android name lookups; desktop restart).
5. **Run your own relay** (`relay`) — requirements (Rust/Docker with Redis + Iggy,
   or Cloudflare Workers Paid), `OPERATOR_SECRET`, fund the treasury on each
   chain, check `/api/health` and `/v1/treasury/{chainId}`; known limits: it reads
   chain metadata from `ethereum-data.getvela.app` (hard-coded today), the source
   Dockerfile, the relay collects the fee the wallet pays.
6. **Run your own public-key index** (`index`) — it writes to the shared registry
   contract on Gnosis (keep it: the wallet reads that contract directly);
   requirements: funded Gnosis key, the contract and domain-registry addresses
   (with the undocumented `P256_INDEX_DOMAIN_REGISTRY` value), health check;
   what happens when no index answers (the wallet reads the contract; a
   single-key wallet can also be recovered from two signatures).
7. **Run your own chain data** (`chain-data`) — the repository, the three ways to
   run it, health check; the relay's dependency on its Vela-specific fields.
8. **Run your own exchange rates** (`exchange-rates`) — vela-currency or any
   Frankfurter-compatible USD source; `?base=USD` or conversions are wrong.
9. **Build the apps yourself** (`web-app`) — web wallet (and why it is a
   different wallet on your domain), the extension (`pnpm build:extension`, load
   unpacked), desktop, iOS, Android; what a self-built app can and cannot sign
   with.
10. **Add a network Vela doesn't ship** — link to `/chain-setup` and the networks
    page.
11. **What still points at us** — the short list of remaining dependencies after
    all of the above (relay → chain-data host; AAGUID directory; selector
    lookups; Apple/Google tunnel; store apps' domain association), each with
    whether it matters.

## Rules

- No step without a way to confirm it worked.
- A step that needs an account, a domain or funds says so up front.
- Commands are copied from the repositories' own READMEs and code, not written
  from memory; environment-variable names are exact.
- Nothing in the guide contradicts the claim ledger; where the product has a
  gap, the guide names it and links nothing that does not exist.
