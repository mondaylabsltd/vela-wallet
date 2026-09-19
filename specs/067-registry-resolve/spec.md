# 067 — The three layers: an index answer is proved, not believed

Design: `specs/062-wallet-keys/plan.md` §6. Founder: "做吧" (2026-09-19).

## The problem

A wallet's ADDRESS is a function of its whole founding key set. Sign-in asks the
index service (p256-index) for that set. Until now every shell believed whatever
the index said, and only asked the registry contract when the index was *gone*
(spec 062). An index that is up and wrong — compromised, or buggy — could hand a
person a founding set with one key swapped, and the wallet would rebuild a
different, fundable, attacker-influenced address without a word.

Each of the four shells also carried its own copy of the index → Gnosis →
Ethereum ladder.

## The rule, written once (`vela_core::registry_resolve`)

| Layer | Role | Believed when |
|---|---|---|
| Index (p256-index) | speed — one HTTP GET | its unit's `contentHashFor(rpId, metadata, groupKey, members)`, computed OFFLINE by the core, equals the `contentHash` a chain stores for that group key |
| Gnosis (100) | truth — where the record lives | always (it is the contract) |
| Ethereum (1) | survival — the person's own backup | for what it HOLDS; its silence about a key is not a verdict |

- One `eth_call` (`getUnitByGroupKey`) proves an honest index. No extra round trips
  when things are well.
- The chain holds a DIFFERENT hash for that group → the index's answer is thrown
  away and the chain's founding set is used (`index_discarded`, logged).
- The chain does not hold the group yet → not proof of forgery (the index accepts
  registrations before they land); the next layer is asked, and failing that the
  index's word stands (`verified_by: none`).
- No chain reachable → the person still signs in on the index's word. Availability
  is never made worse than it was.
- Unit ids are per DEPLOYMENT (Gnosis unit 10 = Ethereum unit 0): a listing's ids
  are only ever asked of whoever listed them (`source` token, owned by the core).

Shells perform `index_get` and `eth_call` and append the outcomes to a transcript.
No new Crux operation, no new capability.

## Out of scope

- `wallet_keys` / `registry_lookup` still run their own (chain-first) walks. They
  never trusted the index for an address, so nothing is unsafe; folding them onto
  the resolver is tidying, not a fix.
- Writes (registration) are unchanged.
