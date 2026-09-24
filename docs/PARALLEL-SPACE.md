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

On the web the fixture signer is reached only through a dynamic import behind the dev gate
(spec 026 D18), so it is never in a production page's startup chunk; on the desktop it is the
`dev-fixtures` cargo feature, which release packages do not compile. Entering is always explicit,
and the badge is unconditional — it gates itself rather than being compiled out, so a fixture
wallet can never be on screen without saying so.

## The fixture keyset

Three fixed P-256 keypairs → three deterministic single-key Safe accounts, plus the multi-key
Safe derived from all three. Defined in
[`app-web/vela-wallet/src/lib/dev/passkey-fixture.ts`](../app-web/vela-wallet/src/lib/dev/passkey-fixture.ts);
locked by `passkey-fixture.test.ts` beside it and by `core/golden-addresses.test.ts`.

| Account | Safe address (identical on every EVM chain) |
|---|---|
| **Parallel One** (primary) | `0xD400866e00B055B20752a826CD5C89b811de130b` |
| Parallel Two | `0x031d7D57c99CAF891e1C250554691Fd12D84772b` |
| Parallel Three | `0x58cd0ce6A27099220543b31710d7860d75Ba1d3d` |
| **golden multi-key Safe** (all three keys, 1-of-3) | `0x88cCA0EeDbF2C4426110bbFc998F048689266894` |

The fixture signer builds a **genuine** WebAuthn assertion (real ECDSA-P256 signature over
`sha256(authenticatorData ‖ sha256(clientDataJSON))`), so the same bytes verify against
Safe's on-chain P-256 verifier — a real UserOp from a fixture Safe settles on-chain.

## Enter / exit

**Route (manual + e2e):** open `/[locale]/parallel` (e.g. `/en/parallel`). The page installs the
fixed-key signer, loads the fixture wallet, and drops you into the real home. It is the only route
under the prefix — there is no `/parallel/connect`; a dApp request is exercised through the
extension instead (below).

**Console (`vela.*`, dev builds):**

```
vela.parallel.enter()      // seed fixture wallet + install mock passkey (then reload)
vela.parallel.exit()       // restore the real wallet + remove mock passkey (then reload)
vela.parallel.status()     // active? + fixture accounts
vela.parallel.addresses()  // the fixture Safe addresses to fund
vela.parallel.help()
```

**e2e (Playwright):** seed `localStorage` before the app boots so the mode is armed on the
first render — see `app-web/vela-wallet/e2e/parallel-entry.e2e.ts`.

## Test dApp + relay

The "dApp side" is a tiny, self-contained page — no external services, no crypto to fake:
`app-web/vela-wallet/e2e/testdapp/index.html` (plus `legacy.html`, which asks through
`window.ethereum` instead of EIP-6963). There is no relay and no pairing step any more: the page is
served by the Playwright fixture and the **Chrome extension** injects the provider into it, which is
the real transport the product ships.

The specs that drive it, all in `app-web/vela-wallet/e2e/`:

| Spec | What it drives |
|---|---|
| `extension-discovery.e2e.ts` | EIP-6963 announcement, `window.ethereum` |
| `extension-connect.e2e.ts` | connect consent, modern and legacy pages |
| `extension-live-provider.e2e.ts` | reads, chain switch, event broadcast |
| `extension-signing.e2e.ts` | `personal_sign`, typed data, a transaction |
| `extension-security.e2e.ts` | the origin and permission boundaries |
| `extension-connections.e2e.ts` | the sessions list |

Run them with `pnpm test:e2e` from `app-web/vela-wallet` (it starts a real worker; screenshots land
in `e2e/__screenshots__/`).

## On-chain (opt-in, real xDAI)

The hermetic suite stubs the bundler and RPC and is the default; `e2e/stub-chain.ts` is what it
stubs with. For a real-network check, enter the parallel space in a browser against the golden
multi-key Safe `0x88cCA0EeDbF2C4426110bbFc998F048689266894` on Gnosis: read-only inspection needs
nothing, and a real send needs a little xDAI in that Safe. The fixture private keys are public, so
**never leave more than pocket change there, and never a real asset.**
