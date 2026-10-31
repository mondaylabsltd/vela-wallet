# iOS — testing the multisig notifications on a simulator

The multisig notifications (M6) come from a background refresh: iOS wakes the
app (`BGAppRefreshTask`), the proposals machine reads every multisig on the
device as it does on screen, and `MultisigNotifier` posts a local notification
for what changed for this device — a transaction that needs its approval, one
ready to execute, an approval counted, one executed. Nothing is posted while
the app is on screen: the screen already says it.

A simulator never runs a `BGAppRefreshTask` by itself, so Debug builds carry a
trigger (`app-ios/VelaWallet/VelaWallet/Dev/DevMultisigPoll.swift`, excluded
from Release and checked absent by `app-ios/scripts/check-ios-store-binary.sh`)
that runs the very same round.

## When the app asks for permission

Once, when there is first something to say:

- the first multisig on the device — created, added by address, added from an
  invite link, or found because one of this device's accounts became a member;
- the first proposal made on the device (it needs the other members).

Never at launch. "Don't Allow" is answered: nothing asks again (Settings →
Notifications → Vela Wallet turns them on later).

`xcrun simctl privacy` cannot grant notifications (its services are calendar,
contacts, location, photos, media library, microphone, motion, reminders and
Siri): answer the app's own ask with **Allow**, or allow it in the
simulator's Settings → Notifications → Vela Wallet.

## The steps

1. Build and install a Debug build on the simulator.
2. Launch it with the trigger armed — from Xcode (scheme → Run → Arguments →
   Environment: `VELA_DEV_MULTISIG_POLL=1`), or from a terminal:

   ```sh
   SIMCTL_CHILD_VELA_DEV_MULTISIG_POLL=1 \
     xcrun simctl launch --terminate-running-process <device> app.getvela.VelaWallet
   ```

   (add the `SIMCTL_CHILD_VELA_…` inputs your setup needs: the parallel space,
   RPC overrides, service endpoints.)
3. Have the multisig shown, and notifications allowed (above). Leave it on
   screen a few seconds: what waits now is recorded as already said.
4. Send the app away — Home (⇧⌘H), or another app in front
   (`xcrun simctl launch <device> com.apple.Preferences`). From here the app
   has about 30 seconds of background time; the trace says
   `away: kept running`.
5. Within those seconds, another member proposes a transaction that needs
   this device's approval (the web wallet, another phone, an outside wallet).
6. Either wait — while the background time lasts the round runs by itself
   every 8 seconds — or run it at once:

   ```sh
   xcrun simctl spawn <device> notifyutil -p app.getvela.VelaWallet.dev.multisigPoll
   ```

7. The banner: "<multisig name> · 1 needs your approval" (the count is what
   waits on this device). Tapping it opens the transaction's sheet.

If no banner shows, read the trace the round writes:

```sh
tail "$(xcrun simctl get_app_container <device> app.getvela.VelaWallet data)/Library/Caches/multisig-poll.log"
```

- no `armed` line: the launch did not carry `VELA_DEV_MULTISIG_POLL=1`;
- `away: the background time ran out` before the proposal: iOS suspended the
  app, and a trigger sent then reaches nobody — bring it to the front and
  start again from step 3, proposing sooner;
- `round: said now {…: N}` with the same N as `said before`: the round read
  nothing new for this device (the proposal does not need its approval, or it
  was already seen on screen before the app went away);
- the count went up but no banner: notifications are not allowed for the app.

The round waits for its own read of the multisig service (up to 22 s), so a
service seconds away (Arc Testnet, a remote mailbox) is read before anything
is said; a read that lands later, while the app is still away, is said when it
lands.

## Automated

`VelaWalletUITests/R9FixLiveUITests` (`test1…` asks on create, `test4…` and
`test6…` — the latter with the service answering 5 s late) runs the same
steps against the multisig fork harness; see the file's header.
