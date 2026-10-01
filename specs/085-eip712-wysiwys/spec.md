# Feature Specification: EIP-712 typed data — what you see is what you sign

**Feature Branch**: `085-eip712-wysiwys`
**Created**: 2026-10-01
**Status**: Implemented (owner steps open: signer-page deploy + `LAUNCH`)
**Input**: Owner request (2026-10-01): a read-only security audit of the latest Vela Wallet for
"what you see is not what you sign" in `eth_signTypedData*` — every client and the shared core,
every method (`eth_signTypedData`, `_v1`, `_v3`, `_v4`); for each: how params are validated, which
element the confirmation sheet shows, which element the approval guard and the SafeTx guard read,
which element is hashed for the passkey, and whether several payloads travel on. Two shapes must be
refused before any sheet: v4 `[benign, malicious]` and legacy `[malicious, benign]`. Proof by
executed tests, no real passkey, wallet or chain. Then fix it — one parse, one canonical
document and digest reused by preview, guards, passkey and submit — redoing PR #337 (web v4 only,
to be closed) for every surface.

The audit itself, with file / function / line references at `main` @ `4f2f8b633` and the executed
evidence: [audit.md](audit.md).

## User Scenarios & Testing

### User Story 1 — A site cannot get a signature for a document the person was not shown (Priority: P1)

A dApp sends a typed-data request carrying two documents, a benign one where the sheet looks and a
malicious one (a Permit for the whole balance) where the passkey looks. Today the web sheet shows
the first string while the passkey signs `params[1]`; iOS, Android, the desktop and the Trusted
Signer page show `params[1]` while the passkey signs `params[0]` for the legacy names.

**Why this priority**: it is a signature over money the person never saw.

**Independent Test**: send each malicious shape to each client's signing entry; it is answered
`-32602` and no sheet opens; no digest can be computed from it.

**Acceptance Scenarios**:

1. **Given** v4 `[benignTypedData, maliciousTypedData]`, **When** it arrives on any client,
   **Then** the dApp gets `-32602` and no sheet opens.
2. **Given** legacy `eth_signTypedData` / `_v1` `[maliciousTypedData, benignTypedData]`, **When**
   it arrives, **Then** the same.
3. **Given** a one-element v4, the wrong order, three params, or a name such as
   `eth_signTypedData_v2`, **When** it arrives, **Then** the same.
4. **Given** a well-formed request whose account is not the one the site was granted, **When** it
   arrives, **Then** the dApp gets `4100`.

### User Story 2 — The sheet shows the very document whose digest is signed (Priority: P1)

**Independent Test**: for each method, the document the sheet decodes hashes to the digest the
passkey signs.

**Acceptance Scenarios**:

1. **Given** a well-formed request in any of the four methods, **When** the sheet opens, **Then**
   it decodes the request's one document — for the legacy names too, which today hand the sheet
   the address.
2. **Given** the same request, **When** it is signed, **Then** the digest is that document's
   EIP-712 digest (before the Safe's `SafeMessage` wrap) — the same on every client.

### User Story 3 — The guards judge the document that is signed (Priority: P2)

**Acceptance Scenarios**:

1. **Given** a SafeTx anywhere in a typed-data request, **When** it arrives, **Then** it is
   refused (today v4 `[benign, SafeTx]` slips past the refusal and is signed).
2. **Given** a Permit sent with a legacy name (`[permit, address]`), **When** the sheet opens,
   **Then** the approval guard flags it (today it reads the address and flags nothing).
3. **Given** a Permit2 `PermitTransferFrom` or `PermitBatchTransferFrom`, **When** the sheet
   opens, **Then** the approval guard flags it as an off-chain permit, signed verbatim only under
   the explicit consent (today: not detected).

### User Story 4 — The Trusted Signer page reads like the wallet (Priority: P2)

**Acceptance Scenarios**:

1. **Given** any typed-data intent, **When** the page renders and digests it, **Then** both use
   one reader, and the shapes of US1 are refused by both.
2. **Given** the new page build, **When** it is released, **Then** it follows the HANDOVER order:
   build → `BUILD_ALLOWED` → owner deploys → checks it serves → `LAUNCH`.

### Edge Cases

- A document given as an object rather than a JSON string: accepted, carried on as its JSON.
- A document that does not hash: refused at arrival (a sheet for something unsignable is useless).
- Big numbers as bare JSON numbers: the digest is unchanged from today's (both readings go through
  the same JSON parse), so no signature a dApp verifies changes.

## Requirements

### Functional Requirements

- **FR-001**: One reader, in the core: the four exact methods; exactly two params; legacy
  `[typedData, address]`, v3/v4 `[address, typedData]`; a well-formed address in its slot; one
  EIP-712 document (`types`, non-empty `primaryType`, `domain`, `message`).
- **FR-002**: A typed-data request that is not that, or whose document does not hash, is answered
  `-32602` at arrival, before any sheet; one naming another account than the granted one, `4100`.
- **FR-003**: What passes is carried on rebuilt as exactly the two canonical params, so every later
  reader holds one document.
- **FR-004**: The signed digest, the SafeTx guard, the approval guard (detection and rewrite), the
  chain pick, the chain-context check and the requested-account check read that document; none
  keeps a pick of its own (`params.find`, `params[1] ?? params[0]`, "first that parses").
- **FR-005**: The SafeTx guard refuses a SafeTx anywhere in a typed-data request.
- **FR-006**: The approval guard detects Permit2 SignatureTransfer (single and batch) — no new
  wire value (every client decodes the guard's kind).
- **FR-007**: Every client's sheet decodes the core's document (`typedDataDocument`); the web
  submit path signs it; the extension refuses the US1 shapes at its first boundary as well.
- **FR-008**: The Trusted Signer page's preview and digest share one strict reader.

### Key Entities

- **CanonicalTypedData** — method, account, the one document, its JSON (what is shown), its digest
  (what is signed).

## Success Criteria

- **SC-001**: The two malicious shapes are refused before any sheet on the core, web/extension,
  iOS, Android and desktop paths, and by the signer page — proven by tests on each.
- **SC-002**: For every method, the sheet's document hashes to the signed digest on every client —
  proven by tests on each.
- **SC-003**: v4 `[benign, SafeTx]` is refused; a legacy Permit and Permit2 SignatureTransfer are
  flagged.
- **SC-004**: All existing suites pass: core, desktop, iOS, Android, web, signer page.

## Assumptions

- Legacy v1's array-of-`{type,name,value}` form is not supported (Vela never hashed it; it is
  refused, as before, now at arrival).
- The signer page's new build goes live only through the owner's release steps; until then the
  core's arrival check keeps malformed shapes from ever reaching the live page.
