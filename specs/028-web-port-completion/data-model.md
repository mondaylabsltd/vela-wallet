# Data Model — 028 Web Port Completion

Two of these are owned by cores that already exist. The rest are shell state,
and they say so.

## Owned by a core (do not re-model)

| Entity | Machine | What it decides |
| --- | --- | --- |
| **Custom token** | `manage_tokens` (22 tests) | whether an address is a token, what its identity is, whether it is already known, and what a refusal says |
| **Sweep draft** | `send` (91 tests) | which assets are selectable, what the master tick selects, the per-asset amounts, the total, and whether the CTA arms |

## Shell state (no core, by decision D48)

- **Preferences** — theme (`light` / `dark` / `system`), language, number
  format, date format, time format, avatar style (`initials` / `identicon`).
  Persisted beside the other small preferences; read where they are used.
  Currency is NOT here: it has a machine, and is already wired.

## Carried across a boundary

- **Receive code** — the address, or the payment request (address + amount +
  asset + chain), as a module matrix an optical decoder can read.
- **Scan result** — an address, a payment request, or a refusal with its reason
  (no camera, permission denied, insecure origin, nothing found in the image).
- **Share card** — address text, the code, and the account's identicon,
  composed. The identicon is derived from the address, which is what makes a
  doctored card inconsistent.
- **Address book file** — contacts and their groups, in a form another browser
  reads. On import, an existing entry wins a collision and the skipped ones are
  reported.

## Storage keys added

Preference keys under `vela.`, so D49's namespace erase reaches them without
anyone remembering to add them to a list.
