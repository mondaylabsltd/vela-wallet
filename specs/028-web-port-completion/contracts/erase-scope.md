# Contract — what "Erase this device" erases

Carried from `src/services/erase-device.ts` @ 28d25ae9, including the reason it
was rewritten once already: **a delete-list erase is wrong by default and only
accidentally right.** Nothing about a hand-maintained list of keys fails when
the app grows a key, so it drifts silently and the failure is discovered by
someone whose data was supposed to be gone.

## The rule

**Enumerate what is actually stored and delete everything under the `vela.`
namespace.** Then name the exceptions here, so an exception is a decision
someone made rather than a key someone forgot.

This is the inverse of a delete-list, and it is the whole point: a key added
next year is erased by default.

## Where it must reach

| Store | What lives there |
| --- | --- |
| `localStorage` | accounts, the active index, the intro flag, service endpoints, preferences, the parallel-space flag and its backup |
| IndexedDB KV | contacts, groups, transaction history, balance / rate / token caches, receive acknowledgements |
| `chrome.storage.local` *(extension build only)* | `vela.perm.*` grants, the `ext_cache` snapshot, any in-flight request records |

## Exceptions — and each needs its reason

| Key | Why it survives |
| --- | --- |
| `vela.pendingUploads` | A record here is a passkey public key the index service has never confirmed. The retry on the next launch needs no account list to re-send it, but a DELETED record can never be retried — and that credential then cannot be found at login on **any** device. Erasing it downgrades "recoverable" to "possibly ruined", which is strictly worse here than at sign-out, because the account list is going too and the retry is the only remaining path to that key. Uploading first and erasing after was rejected: it makes a destructive action the person asked for depend on a network that may be down. |

That is the whole list, and it is the same one Expo carries. Everything else
under `vela.` goes, in all three stores.

**Not exceptions, because they are not ours**: keys outside the `vela.`
namespace are left alone. On the web that is whatever the browser and any other
tool on the origin wrote; this module does not judge them.

## What it must NOT claim

The wallet is the passkey. Erasing this browser removes **local data** and
nothing else: the same passkey opens the same wallet at the same address on the
next device. Copy that says or implies "your wallet will be deleted" would
frighten someone out of a safe action; copy that promises more than local
deletion would reassure someone out of an unsafe one.

## How it is proven

After a confirmed erase, an enumeration of every store above returns nothing
under the `vela.` namespace except the named exceptions, and the app is at first
run. A cancelled confirmation changes nothing — asserted, because a destructive
action's cancel path is the one nobody exercises by hand.
