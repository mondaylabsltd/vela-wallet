# Security advisories

One file per vulnerability that affected, or could have affected, Vela users. Each file follows
GitHub's security-advisory (GHSA) form: the table at the top gives the form's fields, and the
body is the description. How to report a new issue is in [`SECURITY.md`](../../../SECURITY.md).
Internal incidents that put no user's keys or funds at risk are in
[`../incidents.md`](../incidents.md).

| ID | Title | Severity | Fixed in | Published |
| --- | --- | --- | --- | --- |
| [VELA-2026-001](VELA-2026-001-typed-data-preview-mismatch.md) | A typed-data request could preview one EIP-712 message and sign another | High (CVSS 5.3) | v0.9.6 | not yet |
| [VELA-2026-002](VELA-2026-002-parent-safe-authority.md) | A site connected to a Vela account could act for a Safe that account owns | High (CVSS 6.1) | `64da454d3`, not released yet. The multisig feature never shipped with the issue. | not yet; wait for the release |
| [VELA-2026-003](VELA-2026-003-passkey-app-association.md) | getvela.app's app-association files let development and demo apps request users' passkeys | Medium (CVSS 5.3) | site `0562c17bb`, deployed 2026-11-07 | not yet |

`VELA-YYYY-NNN` is a local ID. GitHub assigns a `GHSA-…` ID when an advisory is published. Add
that ID to the file and to this table once it exists.

## Publishing (owner)

1. Repository → Security → Advisories → **New draft security advisory**.
2. Copy the title, the affected and patched versions, the severity (choose "CVSS 3.1" and paste
   the vector) and the weakness from the file's table. Paste the body, from "Summary" to the end,
   as the description.
3. Credit the reporter only as the file says. If the reporter later agrees to be named, update
   the credit first.
4. Publish VELA-2026-002 only after a release containing `64da454d3` has shipped.
