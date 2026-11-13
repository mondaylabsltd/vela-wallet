# Security policy

Vela Wallet is a self-custodial Ethereum wallet. An account is an unmodified Safe v1.4.1 on
ERC-4337, and its owners are passkeys. This page covers how to report a vulnerability to us and
what to expect when you do. It is maintained by MONDAY LABS LTD, the company that makes Vela.

## Reporting a vulnerability

Please do **not** open a public issue, discussion or pull request for a security problem, and do
not use the in-app bug report. In-app bug reports can become public issues.

1. **Preferred: GitHub private vulnerability reporting.** Open
   <https://github.com/mondaylabsltd/vela-wallet/security/advisories/new>. Only you and the
   maintainers can see the report, and we can work on the advisory and fix there together. Use
   this channel for findings in any repository listed under [Scope](#scope), including the
   service repositories.
2. **Email: `security@getvela.app` (being set up).** This address is not confirmed yet. Until it
   is, use GitHub. If you cannot use GitHub, open a public issue that says only that you have a
   security report and need a private channel. Put no details in it, and we will contact you.

We do not have a PGP key yet. If your report needs encryption, say so through either channel and
we will arrange it.

## What to include

- The component and the version or commit. The version is on each app's About or Settings page,
  for example `0.9.8 (ced8298)`.
- The platform: web wallet, Chrome extension, Safari extension, iOS, Android, desktop (and which
  OS), the signing page, or a service.
- What an attacker needs before the attack works: a connected site, a malicious RPC, a
  compromised service, an installed app, physical access, and so on.
- Steps to reproduce, ideally with a proof of concept on a testnet or a local fork. Synthetic
  accounts are best.
- What the attack achieves (for example, a signature the person was not shown, or funds moved),
  and what you have **not** verified.
- How you would like to be credited, or that you would rather not be.

A precise report is more useful to us than a severity claim. If you are unsure whether something
is a vulnerability, report it anyway.

## What to expect

These are targets, not guarantees. Vela has a small team.

| Step | Target |
| --- | --- |
| Acknowledge the report | within 3 business days |
| Triage: we confirm or dispute it, give a severity and say what happens next | within 7 business days of acknowledgement |
| Progress updates while a fix is in progress | at least every 14 days |
| Fix for a confirmed critical or high issue in funds or signing | as soon as we can, aiming for 30 days |

If we miss a target, say so in the thread.

## Coordinated disclosure

- By default we ask for **90 days** from your report before public disclosure. We may ask for an
  extension if a fix needs an app-store review or a slow-to-update package, such as the
  hand-installed browser extension.
- If a fix ships sooner, we would like to publish together once enough users have updated.
- Once a fix is released, we publish an advisory. That includes issues found internally if they
  affected released versions.
- If an issue is being actively exploited, we may disclose sooner, and we will tell you before we
  do.

## Safe harbour

We will not pursue or support legal action against you if you research and report in good faith
under this policy, which means you:

- only use accounts, keys and funds that are yours, or synthetic ones on a testnet or local fork;
- do not access, change or delete other people's data or funds, and stop and tell us if you
  reach any by accident;
- do not degrade our services: no load testing, no denial of service, and no spam against
  production hosts;
- do not write to production services more than a test requires. Key-index registrations and
  Safe Genesis Registry records go on-chain and are permanent. Prefer a local stand-in (both
  services are open source; see Scope).
- do not use social engineering, phishing or physical attacks against Vela staff or users;
- give us reasonable time to fix the issue before you disclose it, as described above.

If someone else brings a legal claim over research you did under this policy, we will make it
known that your research was authorised under this policy. This safe harbour covers only Vela's
own code and services. It cannot authorise testing against third parties, such as chains, RPC
providers, Safe's contracts, passkey providers or app stores. Their own policies apply.

## Scope

In scope: code in these repositories, as released, and these services as Vela runs them.

| Component | Where |
| --- | --- |
| Shared Rust core (`vela-core`): signing, clear signing, guards, fees, multisig rules | [`rust/`](rust/) |
| Web wallet (`wallet.getvela.app`) and the Chrome extension | [`app-web/vela-wallet/`](app-web/vela-wallet/) |
| Safari extension | [`app-ios/VelaWallet/VelaSafariExtension/`](app-ios/VelaWallet/VelaSafariExtension/) |
| iOS, Android and desktop (macOS, Windows, Linux) apps | [`app-ios/`](app-ios/), [`app-android/`](app-android/), [`app-desktop/`](app-desktop/) |
| Trusted Signer page (`sign.getvela.app`) | [`app-web/trusted-signer/`](app-web/trusted-signer/) |
| Website and its API (`getvela.app`), including `/.well-known/` association files | [`app-web/getvela.app/`](app-web/getvela.app/) |
| Relay: bundler and fee sponsor | [mondaylabsltd/vela-relay](https://github.com/mondaylabsltd/vela-relay) |
| Public-key index and its registry contracts | [mondaylabsltd/p256-index](https://github.com/mondaylabsltd/p256-index) |
| Multisig genesis service, its mailbox, and the `SafeGenesisRegistry` contract | [mondaylabsltd/safe-genesis-registry](https://github.com/mondaylabsltd/safe-genesis-registry) |
| Exchange-rate service | [mondaylabsltd/vela-currency](https://github.com/mondaylabsltd/vela-currency) |
| Chain data and ERC-7730 descriptors as served by `ethereum-data.getvela.app` | [atshelchin/ethereum-data](https://github.com/atshelchin/ethereum-data) |
| Release integrity: build provenance, signed packages | [`.github/workflows/`](.github/workflows/), [`docs/RELEASING.md`](docs/RELEASING.md) |

The multisig feature has not been released yet; it is on the `feat/multisig-account` branch.
Reports against it are welcome and in scope.

**Supported versions.** We fix the latest release only. The web wallet always runs the latest
release. On desktop and with the extensions, update to the newest
[release](https://github.com/mondaylabsltd/vela-wallet/releases). A report against `main` or a
feature branch is welcome; tell us the commit.

## Out of scope

- **Third-party contracts Vela uses unmodified:** Safe, Safe4337Module, the Safe passkey
  contracts, EntryPoint and Multicall3. Report bugs in them to their owners, through the
  [Safe Foundation bug bounty](https://docs.safefoundation.org/security/bug-bounty) or the
  [ERC-4337 bug bounty](https://docs.erc4337.io/community/bug-bounty). *How Vela uses them* is
  in scope, for example a wrong address, a wrong configuration or a wrong signature framing.
- Bugs in chains, their P-256 precompiles, public RPC providers, passkey providers (iCloud
  Keychain, Google Password Manager, security keys), browsers or operating systems. Report
  those upstream. Tell us as well if Vela should defend against them.
- Attacks that need an already-compromised OS or browser, or a rooted or jailbroken device,
  **unless** Vela's code gives that attacker more than the platform already would. One example is
  software you run asking a passkey for a getvela.app signature: that is a known limit, and it is
  described in [self-hosting](https://getvela.app/docs/self-hosting#if-getvela-app-disappears).
  Reports that narrow it are welcome.
- Volumetric denial of service, and rate limits on endpoints that hold nothing sensitive.
- Missing headers, cookie flags or email-domain records (SPF, DMARC) with no demonstrated impact.
- Scanner output without a working proof of concept, self-XSS, and clickjacking on pages with no
  sensitive action.
- Issues already listed in [Audits & known issues](https://getvela.app/docs/security-audits) or
  [`docs/KNOWN-BUGS.md`](docs/KNOWN-BUGS.md). A new way to exploit a listed issue *is* in scope.

## Bug bounty

**Vela has no bug bounty at present.** We plan to start one, and this page will say when we do and
on what terms. Until then, we cannot promise payment for a report. Any payment we choose to make
would be ex gratia and would set no precedent.

## Credits

With your agreement, we credit you in the advisory by name or handle, linked as you choose. If
you prefer to stay anonymous, the advisory says "an external researcher". We credit the first
report of an issue. If several people report the same issue independently before we fix it, we
credit each of them. We do not credit reports that break the rules under
[Safe harbour](#safe-harbour).

## Advisories

- Published: <https://github.com/mondaylabsltd/vela-wallet/security/advisories>
- In this repository: [`docs/security/advisories/`](docs/security/advisories/), one file per
  issue, with the affected versions, the fix commit, a timeline and workarounds. Internal
  incidents that affected no user's funds or keys are in
  [`docs/security/incidents.md`](docs/security/incidents.md).
- What Vela depends on, who audited it, and what has no audit at all:
  [Audits & known issues](https://getvela.app/docs/security-audits).
