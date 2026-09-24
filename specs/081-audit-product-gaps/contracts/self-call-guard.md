# Contract — self-call guard (FR-005, FR-006)

**Core → shells**

- `SignView.blocked: Option<SelfCallBlock>` — present when the pending request is refused.
- `SignView.confirm_gate_open` is false whenever `blocked` is present (existing field, existing meaning).
- `SignErrorKind::SelfCallBlocked` — new variant; must be added to the generated TS, `SignWire.kt` and `SignWire.swift` in the same change, or strict decoders reject the whole view.
- JSON-RPC answer to the dApp: `-32603` with a stable message (the unlimited-approval precedent). Never `4001`: the user did not reject it, the wallet did.

**Shell obligations**

1. Render the explanation from `blocked` (function name, and the leg number when `leg_index` is set) in the signing sheet's status area. Web must gain that status area — it renders no `SignView.error` today.
2. Offer only "Dismiss"; no confirm affordance, no swipe-to-sign.
3. Do not send a second request for the same id after a block (it is settled).

**Not covered (explicitly allowed)**: empty-data self-calls (the fee leg and the estimator use them), calls to any other address, and Vela's own flows, which never originate from a dApp request.

**Test hooks**: each shell's test dApp gains buttons for `addOwnerWithThreshold`, `enableModule`, a batch whose second leg is a self-call, and a `SafeTx` typed-data request.
