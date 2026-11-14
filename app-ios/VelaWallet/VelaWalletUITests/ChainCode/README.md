# Contract code for `LocalMultisigWorld`'s chain

Runtime code the hermetic multisig UI tests place on their local anvil
(`anvil_setCode`), beside Safe v1.4.1's own (read from
`rust/crates/vela-core/tests/vectors/safe-code/`).

| File | Address | Source |
| --- | --- | --- |
| `safe-webauthn-shared-signer.hex` | `0x94a4F6affBd8975951142c3999aEAB7ecee555c2` (`safe::WEBAUTHN_SIGNER`) | `eth_getCode` on Gnosis (chain 100), 2026-11-14; the same 2954 bytes from rpc.gnosischain.com and gnosis-rpc.publicnode.com (sha256 `580cc9ee35fa35108e6a0731159e072c2b316b7e01845ca65ccd930bb5bd2de0`) |

Deployed code, as the chain holds it: nothing here is written for the tests.
