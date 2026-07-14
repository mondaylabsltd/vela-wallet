# Vela Safe Recovery

This unpacked WXT/Chrome extension is a service-independent recovery wallet for
Safes whose owner list contains Vela's shared WebAuthn signer contract:

`0x94a4F6affBd8975951142c3999aEAB7ecee555c2`

The signer contract is an authorization module. It is not an EOA and never
signs the outer Ethereum transaction. The extension separates the two roles:

```text
Safe app builds SafeTx / execTransaction
        │
        ├─ passkey signs the SafeTx hash
        │  (contract-signature ABI consumed by the WebAuthn owner)
        │
        └─ local gas-only relayer EOA signs the outer transaction
           and sends it directly to the Safe
```

For a 1/1 Safe, Safe Wallet's immediate **Execute** path may skip
`eth_signTypedData` and put a `v=1` prevalidated-owner placeholder directly in
`execTransaction`. The extension recognizes only the shared Vela owner's exact
placeholder, reads the current Safe nonce, verifies its reconstructed EIP-712
hash against `Safe.getTransactionHash`, opens the passkey approval window, and
replaces that slot with the `v=0` WebAuthn contract signature before relaying.
Already signed executions are relayed without asking for the passkey twice.

The relayer key is generated locally and stored in `chrome.storage.local`. It
only controls native funds held by the gas address; it is not a Safe owner.
Before broadcasting, the extension checks that the target is a deployed Safe,
contains the shared signer, includes that contract signature in the required
threshold slots, validates each fresh passkey assertion against the Safe's
configured key with an ERC-1271 `eth_call`, and passes the complete
`execTransaction` through `eth_call` and `eth_estimateGas`. A failed signature
or inner call therefore does not spend relayer gas.

Safe's Transaction Service may still be used by the web app to store a pending
unsigned transaction, but it cannot validate `SafeWebAuthnSharedSigner`
confirmations: the hosted service calls the owner contract directly, while the
shared signer must read its public key from the calling Safe's storage. For
threshold-1 Safes the extension therefore maintains a local confirmation
overlay. Safe Wallet's **Sign** action stores the validated passkey signature in
`chrome.storage.local`, proposes the matching transaction without a signature,
and merges the local confirmation into matching queue and transaction-detail
responses. A later Execute reuses the exact signature and still passes the
relayer's on-chain preflight. The confirmation is intentionally visible only in
the Chrome profile that holds it; it is not synchronized by Safe's service.

Safe Wallet's **Execute** action supports both fee choices:

- **Sponsored by Gnosis** signs the canonical SafeTx with the passkey. The
  extension makes only the Transaction Service proposal unsigned, then Safe
  Wallet sends the still-signed `execTransaction` calldata to Gnosis relay.
- **Connected wallet** replaces Safe Wallet's `v=1` placeholder with the
  passkey contract signature and broadcasts through the local gas-only EOA.

Neither execution path uses a SafeOp. If a Safe has the Safe4337 module enabled
and a compatible bundler/paymaster is configured, a separate ERC-4337 Relay Kit
adapter can also replace the outer EOA path; the authorization boundary remains
the same.

## Run locally

```bash
npm install
npm run check
npm test
npm run build
```

Load the generated `.output/chrome-mv3` directory from `chrome://extensions`
with Developer mode enabled. Open the extension popup, grant the selected RPC
and `getvela.app` host permissions, verify the chain, and enable recovery mode.
Fund the displayed gas address with a small amount of that chain's native coin.
After every extension install or reload, fully reload the open Safe page before
connecting. Safe memoizes whether an owner has contract code; an already-open
page can otherwise keep the stale on-chain-only `approveHash` signing path and
never request the off-chain passkey signature.

The provider is exposed only on `https://app.safe.global/*` through EIP-6963 and
the `window.ethereum` provider list. It rejects arbitrary signing methods and
only accepts canonical SafeTx EIP-712 requests. A narrowly scoped Safe-page
fetch shim makes the shared validator look EOA-like during Safe's owner
classification lookup; the background worker still uses the real bytecode and
requires the contract signature before relaying.

## Recovery limitations

- The selected Safe must already contain the shared WebAuthn signer and have a
  passkey/public-key registration understood by that signer contract.
- This version intentionally supports Safe `execTransaction` recovery only; it
  does not impersonate an EOA, approve arbitrary typed data, or submit raw
  transactions.
- **Sign / queue confirmation** is a local extension overlay, not a hosted Safe
  confirmation. Back up the extension profile if queued approvals must survive
  a browser reset, and do not expect another browser or owner to see them.
- Local queue confirmations and sponsored Passkey signing currently support
  threshold-1 Safes. **Connected wallet** direct execution can still merge the
  shared signer slot with other signatures already present in calldata.
- Keep the relayer key backed up separately if the gas balance matters. Rotate
  it only after exporting or emptying the old gas address.
- The browser extension is a recovery tool. Review the Safe confirmation and
  the passkey approval window before authorizing a transaction.
