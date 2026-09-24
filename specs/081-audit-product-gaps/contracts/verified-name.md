# Contract — forward-verified names (FR-010)

**Core owns the rule**, shells own the transport — the pattern already used by `registry_lookup.rs` (`registryNameStep`).

- Core: `name_verify::step(state, answer) -> NameVerifyStep` emitting `Call { chain_id, to, data }` for `resolver(namehash(name))` then `addr(bytes32)` (ENSIP-10 `resolve(bytes,bytes)` where the resolver is a wildcard/CCIP one), and finally `Verdict { forward_state }` after a case-insensitive address comparison.
- Shells: perform the `eth_call`s, feed answers back, and show the name only on `Verified`.
- Cache: only verified positives, 24 h, cache key versioned so pre-change entries are discarded.
- Timeout/unavailable resolver → `Unavailable` → address shown alone (never the unverified name).

Applies to ENS, Basename (ENSIP-19 reverse → forward), `.bnb`, `.arb`, `.g`. Vela registry names are labels, not records: unchanged.
