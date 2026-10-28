# WalletPair v2 messages

Ported from sub3 (`website/src/lib/walletpair/`), itself from the WalletPair
repository (`walletpair-extension/src/lib/walletpair/*` and
`src/wallet/wallet.ts`, MIT), commit `83b45d7`. `wallet.ts` is WalletPair's
reference Wallet, extended (by sub3) for a Wallet that forwards to an injected
provider (`beginEffect`). Keep every file identical to its source: they are
excluded from Prettier, and ESLint relaxes two rules for them.

Vela is the DApp side (`client.ts`); its `/connect` page is the Wallet side
(`wallet.ts`), forwarding to the browser's own wallet.
