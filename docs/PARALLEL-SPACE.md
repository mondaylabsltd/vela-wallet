# Parallel Space — the test environment

> A running copy of the **real** Vela app whose **only** difference is that passkey
> signing uses a fixed keyset instead of a real device credential. Everything else —
> chains, RPC, bundler, backend, relay, storage, and every screen — is the real thing,
> pixel-for-pixel. It exists so every feature (onboarding, home, send, receive, connect
> dApp, settings) can be driven end-to-end and on a real network (e.g. Gnosis) without a
> biometric prompt, deterministically.

## The boundary (real space ⇄ parallel space)

| | Real space (production) | Parallel space (test) |
|---|---|---|
| **Passkey / WebAuthn** | real device credential (`navigator.credentials` / Secure Enclave) | **fixed fixture keyset** ← the ONLY difference |
| Wallet accounts | user's real accounts | fixture Safe accounts (swapped in; real cache backed up & restored on exit) |
| Chains · RPC · bundler · backend | real | **real** (unchanged) |
| Relay · storage · UI · gestures | real | **real** (unchanged, pixel-for-pixel) |
| Entry | normal app | `/parallel` route or `vela.parallel.enter()` |
| Marker | — | persistent **PARALLEL SPACE** badge (top of screen) whenever active |

Because the only swap is the signer, and the true keys live in the device passkey (never
in local storage), entering/exiting the parallel space is safe and fully reversible.

Everything is gated on `__DEV__`: the passkey override in `src/modules/passkey/index.ts`
is a compile-time no-op in release builds, so none of this ships enabled in production.

## The fixture keyset

Three fixed P-256 keypairs → three deterministic Safe accounts. Defined in
[`src/services/dev/passkey-fixture.ts`](../src/services/dev/passkey-fixture.ts); locked by
[`src/__tests__/services/passkey-fixture.test.ts`](../src/__tests__/services/passkey-fixture.test.ts).

| Account | Safe address (identical on every EVM chain) |
|---|---|
| **Parallel One** (primary) | `0xD400866e00B055B20752a826CD5C89b811de130b` |
| Parallel Two | `0x031d7D57c99CAF891e1C250554691Fd12D84772b` |
| Parallel Three | `0x58cd0ce6A27099220543b31710d7860d75Ba1d3d` |

The fixture signer builds a **genuine** WebAuthn assertion (real ECDSA-P256 signature over
`sha256(authenticatorData ‖ sha256(clientDataJSON))`), so the same bytes verify against
Safe's on-chain P-256 verifier — a real UserOp from a fixture Safe settles on-chain.

## Enter / exit

**Route (manual + e2e):** open `/parallel`. The layout installs the fixed-key signer,
loads the fixture wallet, and drops you into the real home. `/parallel/connect` opens the
real Connect screen directly under the prefix.

**Console (`vela.*`, dev builds):**

```
vela.parallel.enter()      // seed fixture wallet + install mock passkey (then reload)
vela.parallel.exit()       // restore the real wallet + remove mock passkey (then reload)
vela.parallel.status()     // active? + fixture accounts
vela.parallel.addresses()  // the fixture Safe addresses to fund
vela.parallel.help()
```

**e2e (Playwright):** seed `localStorage` before the app boots so the mode is armed on the
first render — see `e2e/support/parallel.ts`.

## Test dApp + relay

The "dApp side" is a tiny, self-contained page served by a local relay — no external
services, no crypto to fake. It speaks the real RemoteInject wire protocol (SSE + POST), so
the wallet's real transport drives the whole connect → request → approve → respond loop.

```
node e2e/support/relay.js
#   prints a connect URL:  http://localhost:8788/s/<id>?n=<n>&k=<k>
#   serves the test dApp:  http://localhost:8788/
```

1. In the wallet, open `/parallel/connect` and paste the connect URL.
2. Open the test dApp page, fire a request (transfer, approve, personal_sign, typed data,
   batch, chain switch, reads…).
3. Approve it in the wallet — signed by the fixture passkey — and the dApp shows the result.

Playwright automates exactly this (`e2e/parallel-dapp.spec.ts`).

## On-chain (opt-in, real xDAI)

The hermetic suite mocks the bundler/RPC and is the default. A single opt-in test
(`@onchain`, excluded from default CI) deploys the counterfactual fixture Safe and submits a
real UserOp on Gnosis. Fund **Parallel One** with a little xDAI first; the bundler gas
account may also need a top-up (see `bundler-service.ts`). Run with the on-chain tag enabled.
