---
title: "What happens when you press Send"
description: "Your device builds the payment and your key signs it. A relay carries it to the network, where your wallet contract checks the signature and moves the money."
facts:
  - "Built and signed | On your device"
  - "Put on chain by | The relay"
  - "Signature checked by | Your Safe, on chain"
  - "If one call fails | None of the batch takes effect"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - rust/crates/vela-core/src/app/send.rs#L1-L44
  - rust/crates/vela-core/src/app/fee_policy.rs#L1-L16
  - rust/crates/vela-core/src/user_op.rs#L200-L227
  - rust/crates/vela-core/src/user_op.rs#L1515-L1572
  - rust/crates/vela-core/src/user_op.rs#L724-L769
  - app-desktop/vela-wallet/src/executor/user_op.rs#L634-L665
  - app-desktop/vela-wallet/src/executor/user_op.rs#L770-L855
  - app-desktop/vela-wallet/src/executor/user_op.rs#L983-L1045
  - rust/crates/vela-core/src/app/tx_tracker.rs#L1-L35
  - specs/080-site-content-accuracy/claim-ledger.md#L14
  - specs/080-site-content-accuracy/claim-ledger.md#L17-L21
  - https://github.com/safe-global/safe-modules/blob/4337/v0.3.0/modules/4337/contracts/Safe4337Module.sol
  - https://github.com/mondaylabsltd/vela-relay
docs: send-and-receive
related:
  - how-it-works/what-your-passkey-signs
  - how-it-works/what-the-relay-does
  - transactions/a-timeout-is-not-a-failure
order: 7
featured: true
draft: true
---

Your wallet is a Safe, a smart contract, and a contract can't start a transaction by itself. Your
device prepares an operation, a request for your wallet to carry out. Your key signs it, and a relay
puts it on chain. Vela follows ERC-4337, the Ethereum standard for this, which calls the signed
request a UserOperation and the relay a bundler.

## Step by step

1. **Build.** Vela builds the operation: your transfer or other calls, plus one more call that
   pays the relay its fee. All the calls go into one batch. In your first transaction on a network,
   the operation also carries the code that deploys your wallet.

2. **Quote.** Vela asks the relay to estimate the gas and quote the fee, and checks that the
   relay has gas of its own on that network.

3. **Confirm.** The confirm screen shows what the operation does and the exact fee. You confirm.

4. **Sign.** Vela computes the operation's hash. Your key checks that it's you, with Face ID,
   fingerprint, device PIN, or a touch and PIN on a security key, and signs the hash.

5. **Hand over.** Vela saves the transaction as "may have been sent", then sends the signed
   operation to the relay.

6. **Submit.** The relay places the operation in an ordinary transaction to the EntryPoint, the
   ERC-4337 contract that runs operations, and pays the network's gas from its own funds.

7. **Check.** If your wallet isn't deployed on this network yet, the EntryPoint deploys it first.
   The EntryPoint then asks your Safe to validate the operation, and the Safe checks your key's
   signature.

8. **Run.** The EntryPoint tells your Safe to run the batch. Your calls run, and the last call
   pays the relay.

9. **Track.** Vela follows the operation until it finds a receipt on chain, and shows the result.

## If something goes wrong

The batch runs as one unit. If any call in it fails on chain, none of them takes effect, the fee
payment included, and Vela marks the transaction as failed.

If the relay or the connection stops answering, Vela doesn't mark the transaction as failed,
because it may still land. [Why a timeout never marks your transaction
failed](/notes/transactions/a-timeout-is-not-a-failure) explains how Vela keeps checking.

## Who can change what

Once your key has signed in step 4, nobody can change the operation without invalidating the
signature. The relay can delay or refuse it, but it can't change it. The fee goes to the relay
the wallet is set to: Vela's by default, or another vela-relay deployment, including one you run.

[What your passkey actually signs](/notes/how-it-works/what-your-passkey-signs) and [How the
blockchain checks a passkey signature](/notes/how-it-works/how-the-chain-checks-a-passkey) cover
steps 4 and 7 in detail.
