# Multisig background checks on Android (and what MIUI needs)

Round 10, 2026-11-01. The multisig's notifications (M6) come from this device:
no push service, no server that learns anything. While Vela is closed a
WorkManager round reads every multisig here the way the screens do and says
what changed. This page is how the round is scheduled, why, and what an OEM
scheduler (MIUI / HyperOS above all) needs from the person.

## How a round is scheduled (`MultisigNotifier`)

| Round | When | Constraints | Why |
| --- | --- | --- | --- |
| Periodic (`vela.multisig.poll`) | every 15 min (Android's floor), 5 min flex | a network with internet; battery not low; exponential back-off from 1 min | The system batches it with other work in its window; a round that could not finish is tried again a few times, then the next period. `UPDATE`, never `KEEP`, so an older install gets these constraints. |
| Leaving (`vela.multisig.poll.leaving`) | the app's last screen stops, with a multisig here | a network with internet | **Expedited**: the work OEM schedulers still run promptly (MIUI holds periodic jobs for hours). It measures what changes next from what the person last saw, and the first round does not wait for the periodic one. Out of expedited quota it runs as ordinary work; before Android 12 it shows a silent, minimum-importance "Checking for transactions waiting for approval…" while it runs. |

A round posts nothing while any screen of the app is in front (the views say
it). Rounds never stack: each has a unique name.

**"Internet", not "validated".** `NetworkType.CONNECTED` is read by
JobScheduler as INTERNET + VALIDATED. Android validates a network by reaching
Google; where Google is out of reach the network is never validated and a
job asking for it never runs at all. Measured on an emulator behind such a
network: `CONNECTIVITY` stayed unsatisfied for good. The rounds read the
multisig service and the chains, never Google, so they ask for a network with
the INTERNET capability only (`setRequiredNetworkRequest`, Android 9+; VPNs
included).

## Measured (2026-11-01)

- Emulator (Android 14), app in the background, no trigger: leaving round
  6 s after Home; periodic round on its own at 14:20:47, 10 min after it was
  first enqueued (`scratchpad/explore/r10-fix-android/emu-poll-natural.log`).
- Redmi K40 (MIUI 14 / Android 13), device-test build, local services only:
  leaving round 4 s after Home; periodic round on its own at 14:27:36, 13 min
  after the app was put away (`redmi-poll-natural.log`). In the first
  real-device pass the periodic job stayed `Ready=false` for 25 min; the job
  then asked for a VALIDATED network.

## What MIUI (and other OEM battery savers) need

No API reports these settings. Vela says so honestly, once, where the person
looks for it — the multisig's Settings → Notifications — never as a banner:

- **Battery saver: "No restrictions"** for Vela (Settings → Apps → Vela →
  Battery saver). Under MIUI's default "Smart" a periodic job of an app that
  is not in front may wait hours; under "Restrict background apps" it does not
  run.
- **Autostart: on** (Settings → Apps → Vela → Autostart, or Security →
  Permissions → Autostart). Swiping Vela away from Recents on MIUI stops the
  process like a force-stop: Android cancels its scheduled jobs until the app
  is opened again (WorkManager re-schedules them then). Autostart lets the
  system start Vela for its jobs.
- Locking Vela in Recents (the padlock) also keeps it from being stopped.

The section names the makers whose battery savers are known to do this
(Xiaomi/Redmi/POCO, Huawei/Honor, OPPO/realme/OnePlus, vivo/iQOO, Meizu,
Samsung) and opens the app's system page. Android's own restrictions are said
as facts: a background restriction ("Restricted" battery use) or a `rare` /
`restricted` standby bucket. Android's default "Optimized" battery use is NOT
said as a limit: periodic work still runs in its maintenance windows, and a
warning every phone shows would be a nag.

## The notification permission

Asked once per install, when there is first something to say (a multisig
created, added, or an invite taken; or the first proposal), on Android 13+.
The mark is written when the system ANSWERS, so an ask the system never
showed is asked again on a later run, once per run. Two asks in the same
frame launch one request (on MIUI the second came back "denied" at once while
MIUI's own dialog was still up). Proven on a clean install: emulator
(Android 14, the system dialog) and the Redmi (MIUI's
`com.lbe.security.miui` dialog, "始终允许" → `POST_NOTIFICATIONS: granted=true`).
