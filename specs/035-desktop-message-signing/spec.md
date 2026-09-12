# Feature Specification: Desktop Message Signing — the rung the ladder was missing

**Feature Branch**: `035-desktop-message-signing` (stacked on `034-desktop-contacts-parity`)

**Created**: 2026-09-09

**Status**: Draft

**Input**: The re-run comparison (034 phase 3) left ten "weak" differences —
events the web dispatches only from inside `lib/**/core`, where a caller has to
be found by hand. This cut checks all ten and builds the one that is real.

## The ten, checked

| Event | Verdict |
|---|---|
| `clear_signing.MessagePresented` | **REAL** — dispatched by `sheet.svelte.ts` whenever a `personal_sign` request arrives. This cut. |
| `fee_policy.Requote` / `LeaveConfirm` | Retired: `FeeQuote` declares both methods and **nothing calls either**. |
| `activity_feed.DeleteRequested` | Retired (034): the button that would send it never renders — its label is never set. |
| `manage_tokens.DeleteRequested` | Retired (034): the web dispatches three manage-token events and this is not one of them. |
| `payment_request.LinkOpened` | The `/pay` URL grammar — a web route's parser, with no desktop URL to open. Out of scope until deep links. |
| `sign_request.FundingCancelled` | Not a dispatch at all: the grep hit a match arm in an error-message mapper. The desktop reaches the same state through `SwipeDismissed`'s funding-phase routing. |
| `token_trust.SimDeltasComputed` | Retired (034): the whole chain terminates in storage and an unread field on both shells. |
| `dapp_permissions.PopupRequest` | Extension-only surface. |
| `tx_tracker.Abort` | Teardown internals, not a capability. |

## Why the one that is real matters

The desktop's signing panel has rendered `ClearSurface::MessageSign` since spec
022 — the decoded text, the SIWE fields, the domain binding, the danger class.
Its host's comment even says *"typed data and a plain message are their own
rungs of the same ladder"*. The match under that comment had **no arm for the
message**. So the core computed the whole analysis of a login request and the
shell threw it away, showing the raw request instead.

That is the surface where address-poisoning and phishing are caught, so it is
worth a spec of its own even though the fix is one arm.

## Scope

1. `personal_sign` → `MessagePresented { PersonalSign, … }`.
2. `eth_sign` → `MessagePresented { EthSign, … }` — **not** the calm view: it
   signs an opaque hash and the core gives it the hard warning.
3. The origin passed is the BROWSER's, and an empty one is `None` — unbindable,
   never a match.

## Out of Scope

- Any change to `clear_signing`'s rules.
- The `/pay` deep link (no desktop URL scheme handler yet).
- New corpus keys.

## Success Criteria

- **SC-351**: each of the four request methods reaches its own rung, and an
  unknown method starts nothing.
- **SC-352**: params reach the core as the strings the site sent, in order,
  with non-strings dropped rather than stringified — the machine reads them
  positionally.
- **SC-353**: `cargo test` counts strictly increase; the four gates stay green.
