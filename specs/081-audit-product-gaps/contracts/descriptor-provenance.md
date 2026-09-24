# Contract — descriptor provenance (FR-008)

**Core → shells**: `ClearSignResult.provenance: ClearProvenance` (`BuiltIn | PinnedMatch | Fetched | Standard | SelectorDb`), and `verified` becomes a derived mirror of `BuiltIn | PinnedMatch`.

**Label rules per shell** (strings come from the corpus, one key per state):

| provenance | label | warning |
| --- | --- | --- |
| BuiltIn / PinnedMatch | "Verified" | none |
| Fetched | "Description from Vela's descriptor service, not authenticated" | caution |
| Standard | token-standard decode | none (not "selector not listed") |
| SelectorDb | "Best effort" | caution (unchanged) |

Fixes shipped with it: desktop must stop labelling `partial` with `verifiedAbiWarning`; web must stop mapping every `!verified` to `selectorNotListed`.

**Pinning**: a fetched descriptor equal (as parsed JSON) to the built-in one for the same address becomes `PinnedMatch`. The typed descriptors Vela relies on (ERC-2612 permit, Permit2) move into the built-in table so permit screens keep a truthful "Verified".
