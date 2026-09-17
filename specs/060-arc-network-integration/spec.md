# Feature Specification: Arc Network Integration

**Feature Branch**: `060-arc-network-integration`

**Created**: 2026-09-17

**Status**: Draft

**Input**: User description: "使用 github speckit 来完成对 arc 的接入，ethereum-data 改动在 /Volumes/data/production/shelchin-workspace/data/ethereum-data（改完部署到 Cloudflare，内置链接改为 https://ethereum-data.getvela.app/），vela-relay 在 /Volumes/data/production/vela-relay 改动并部署到 Cloudflare（使用 https://vela-relay-cf.getvela.app）。费用感知上采用最低 0.01 美元费用，和其他网络保持一致。"

## Context

Arc is Circle's USDC-native Layer 1 (mainnet chain id `5042`, public mainnet opened 2026-09-16; testnet `5042002`). It is EVM-compatible, runs Reth under Malachite BFT with a permissioned institutional validator set, finalises deterministically in under a second, and — the fact that drives this whole feature — **uses USDC as its native gas coin**, with 18 decimals natively and a 6-decimal ERC-20 interface at `0x3600000000000000000000000000000000000000` that is *a second view of the very same balance*.

Reconnaissance against both networks on 2026-09-17 (recorded in `research.md`) confirmed that every contract in the wallet's own admission list — `REQUIRED_CONTRACTS` plus the RIP-7212 P-256 precompile — is already deployed at the canonical addresses on Arc mainnet and testnet. The Safe / ERC-4337 / passkey signing path therefore needs no change at all: counterfactual addresses match other chains, and passkey verification runs on the precompile.

The work is concentrated in three places instead: **what a balance is** (one coin with two interfaces), **what a fee costs** (a chain whose gas coin is worth exactly one dollar and whose base fee has a 20 Gwei floor below which transactions are *silently discarded*), and **where the wallet's own services live** (this feature also relocates the chain-data service and the relay to Cloudflare under `getvela.app`).

Two scope decisions were taken with the requester before drafting:

1. The built-in bundler endpoint moves to `https://vela-relay-cf.getvela.app` **for every chain**, not only for Arc; `https://vela-relay.getvela.app` is retired.
2. Arc testnet (`5042002`) does **not** become a built-in network. It is the verification vehicle for this feature and is reachable through the existing "add a custom network" flow.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Arc is a network the wallet can hold and move money on (Priority: P1)

Someone opens Vela, sees Arc in the network list next to the twelve networks already there, switches to it, and sees their USDC balance. They send USDC to a contact; the wallet quotes a fee in dollars, they approve with a passkey, and the payment lands in about two seconds. An incoming USDC transfer is noticed on the receive screen the same way it is on every other chain.

**Why this priority**: This is the feature. Nothing else in this spec has value if a person cannot read a balance and complete a send on Arc.

**Independent Test**: On Arc testnet with a funded passkey wallet: switch to the network, confirm the balance matches the explorer, send to a second address, confirm the receipt screen resolves and the explorer shows the transfer. Requires nothing from US-3 or US-4 beyond a chain-data entry.

**Acceptance Scenarios**:

1. **Given** a wallet with no Arc history, **When** the person opens the network picker, **Then** Arc appears as a built-in network with its own name, icon and explorer link, and Arc testnet does not.
2. **Given** a Safe that has never been deployed on Arc, **When** the person sends USDC for the first time, **Then** the wallet deploys the Safe and executes the transfer in one operation, exactly as it does on any other built-in chain.
3. **Given** a submitted Arc operation, **When** the receipt screen shows "usually about N seconds", **Then** N reflects Arc's sub-second blocks rather than a copied Ethereum value.
4. **Given** an Arc address that holds USDC, **When** the portfolio loads, **Then** the Arc holding is included in the multi-chain total with a fiat value derived from a $1 peg.
5. **Given** a dApp connected through the browser extension or the in-app browser, **When** it requests `eth_chainId` on Arc, **Then** it receives `0x13b2` and signing proceeds through the ordinary clear-signing path.

---

### User Story 2 - The fee reads like every other network (Priority: P1)

The person sees an Arc fee quoted in dollars, and it obeys the same floor the wallet applies everywhere else: never less than one cent. The operation they approve is one Arc will actually accept — it is never silently discarded for being priced below the chain's floor, leaving a payment that appears submitted and never arrives.

**Why this priority**: Arc discards any transaction whose `maxFeePerGas` is under 20 Gwei **without an error**. The wallet's existing fallback gas price is 5 Gwei, which after the ×2 bundler margin is 10 Gwei — under the floor. A wrongly-priced Arc operation does not fail loudly; it disappears. That is the single most damaging failure mode this integration can ship.

**Independent Test**: Quote and submit an Arc send with the chain's `eth_gasPrice` reachable, then again with it forced to fail (fault injection), and confirm both produce a `maxFeePerGas` at or above 20 Gwei and both land.

**Acceptance Scenarios**:

1. **Given** an Arc send of any size, **When** the fee card settles, **Then** the quoted fee is at least $0.01 worth of USDC, the same floor Ethereum, Gnosis and every other built-in chain apply.
2. **Given** the chain's gas-price read fails, **When** the wallet falls back to a static gas price, **Then** the resulting `maxFeePerGas` is still at or above Arc's 20 Gwei floor.
3. **Given** an Arc fee quote, **When** it is displayed, **Then** the native coin is priced at exactly $1 from a peg rather than from a DEX or Chainlink lookup that does not exist on Arc.
4. **Given** the native coin cannot be priced on some future chain, **When** the fee is computed, **Then** the existing blind-floor behaviour is unchanged — this feature must not alter fee behaviour on the twelve existing networks.
5. **Given** a recipient that Circle has blocklisted, **When** the transfer reverts, **Then** the failure is explained as a rejected-by-issuer transfer, not as an unknown error.

---

### User Story 3 - One coin, one row (Priority: P2)

Arc's native USDC and its ERC-20 interface are the same money. The person sees one USDC row with one balance — never two rows, never a doubled total — no matter which surface they look at or whether they try to add the ERC-20 interface as a custom token.

**Why this priority**: A doubled balance is a correctness failure the person can see and cannot explain; it undermines trust in every other number on the screen. It is P2 only because it is contained — it cannot lose funds, and US-1 is demonstrable before it is fixed.

**Independent Test**: Load an Arc address holding USDC, and confirm the portfolio total equals the on-chain balance rather than twice it; then attempt to add `0x3600…0000` through "add a token" and confirm the wallet refuses it as the native coin.

**Acceptance Scenarios**:

1. **Given** an Arc address holding USDC, **When** the token list renders, **Then** exactly one USDC entry appears and its amount equals the on-chain balance.
2. **Given** the multi-chain portfolio, **When** Arc holdings are summed into the total, **Then** the Arc USDC balance is counted once.
3. **Given** a person pasting `0x3600000000000000000000000000000000000000` into "add a token" on Arc, **When** the wallet resolves it, **Then** it is refused with an explanation that this is Arc's native coin, already shown.
4. **Given** a send of USDC on Arc, **When** the person taps Max, **Then** the reserve arithmetic treats the transfer asset and the fee asset as the same asset, as it already does for a native-coin send elsewhere.

---

### User Story 4 - The wallet's services move house without anyone noticing (Priority: P1)

The chain-data service answers at `https://ethereum-data.getvela.app` and the relay at `https://vela-relay-cf.getvela.app`. Existing wallets keep working on all twelve existing networks; a person who never opens Settings sees no difference beyond Arc's arrival.

**Why this priority**: Arc's chain data has to be served from somewhere, and the requester asked for both services to be relocated as part of this work. It is P1 because US-1 depends on the chain-data entry being reachable at the built-in URL.

**Independent Test**: Point a clean install at the new defaults and exercise balances, sends and deposit detection on Ethereum, Base, Gnosis and Tempo — the pre-existing chains — with no reference to Arc.

**Acceptance Scenarios**:

1. **Given** a clean install, **When** the wallet resolves its service endpoints, **Then** chain data comes from `ethereum-data.getvela.app` and bundler traffic from `vela-relay-cf.getvela.app`, on all four shells.
2. **Given** a wallet whose owner had overridden a service endpoint in Settings, **When** they upgrade, **Then** their override is preserved and not silently replaced by the new default.
3. **Given** the health check in Settings, **When** it probes each relocated endpoint, **Then** the service identity it reports is unchanged (`ethereum-data`, `vela-relay`), so a correct endpoint is never shown as unhealthy.
4. **Given** an existing chain such as Base, **When** a send is quoted and submitted through the relocated relay, **Then** it behaves as it did before this feature.
5. **Given** the relocated chain-data service, **When** any of the 2,600+ existing chain files or the chain-logo and ERC-7730 assets are requested, **Then** they are served as before.

---

### Edge Cases

- **Arc RPC unreachable at load**: the chain keeps its last known holdings and reports a read failure rather than claiming a zero balance (the rule established in #196).
- **`eth_gasPrice` succeeds but returns below 20 Gwei**: the wallet must still price at or above the floor; the floor is a property of the chain, not of the reading.
- **Block timestamps repeat** (sub-second blocks share a timestamp): anything ordered by time must break ties by block number, or activity rows will shuffle.
- **A person adds Arc testnet as a custom network**: the compatibility checker must pass it (all contracts and the P-256 precompile are present) and the wallet must work there, even though it is not built in.
- **The relay has no funded relayer on Arc**: submission must fail with an explainable "relay cannot serve this chain right now", not a silent hang.
- **A person's override still points at the old service hostnames**: they keep working as long as those hosts answer; nothing in this feature may rewrite a person's stored override.
- **A dApp sends a value transfer to a destructed or blocklisted account**: the revert must be surfaced with the issuer/destructed explanation rather than a bare estimation failure.
- **Arc's chain-data entry is missing or stale in the cache**: the wallet falls back to the built-in RPC and still reads native balance; it does not crash or show an empty network.

## Requirements *(mandatory)*

### Functional Requirements

#### Network presence

- **FR-001**: Arc MUST be a built-in network on all four shells (iOS, Android, web, desktop) with chain id `5042`, display name "Arc", native symbol `USDC`, default RPC `https://rpc.mainnet.arc.io`, and explorer `https://explorer.arc.io`.
- **FR-002**: The wallet MUST state an expected inclusion time for Arc that reflects its sub-second blocks, so the receipt screen's "usually about N seconds" is honest.
- **FR-003**: Arc testnet (`5042002`) MUST NOT appear as a built-in network, and MUST remain addable through the existing custom-network flow.
- **FR-004**: The provider-key feature MUST only offer a third-party RPC provider for Arc if that provider's network identifier has been verified to resolve; an unverified provider MUST be omitted rather than guessed, because a wrong identifier poisons the RPC pool.
- **FR-005**: Arc MUST NOT be treated as an L2, an Arbitrum-family chain, or an OP-stack chain for the purposes of gas adders.

#### Money: one balance

- **FR-006**: The wallet MUST present Arc's native USDC and its ERC-20 interface at `0x3600000000000000000000000000000000000000` as a single holding with a single balance, on every surface that lists assets (token list, portfolio total, send asset picker, fee asset picker).
- **FR-007**: The chain-data entry for Arc MUST NOT list the ERC-20 USDC interface as a stablecoin, so it is never discovered as a separate token.
- **FR-008**: The wallet MUST refuse to add `0x3600…0000` on chain `5042` as a custom token, explaining that it is the network's native coin.
- **FR-009**: Arc's other issued assets (EURC, USYC) MUST NOT be listed in the chain's curated stablecoin list. Membership in that list *means* "worth about one dollar" and causes the wallet to price the asset at exactly $1.00 — true of neither a euro token nor a yield-bearing one. They remain addable as ordinary custom tokens.

#### Money: fees

- **FR-010**: The wallet MUST price Arc's native coin at exactly $1 through a stablecoin peg rather than through a DEX quote or Chainlink feed.
- **FR-011**: The peg rule MUST be expressed once, in the shared core, and consulted by all four shells — the current per-shell `"USD" ⇒ $1` duplication MUST NOT be extended to a second symbol in four places.
- **FR-012**: The peg MUST cover Arc's `USDC` native symbol and preserve Tempo's existing `USD` behaviour unchanged.
- **FR-013**: With the peg in place, an Arc fee MUST be subject to the same $0.01 value floor every other network already applies; no Arc-specific fee floor may be introduced.
- **FR-014**: The wallet MUST NOT submit an Arc operation whose `maxFeePerGas` is below Arc's 20 Gwei minimum base fee, including on the static-fallback path taken when the chain's gas-price read fails.
- **FR-015**: The minimum-gas-price rule MUST be per-chain data consulted by the shared fee machine, not a hard-coded Arc branch scattered across shells.
- **FR-016**: Arc MUST NOT use Tempo's stablecoin gas model. Arc has a native coin and pays gas natively; only the coin's price is special.
- **FR-017**: A transfer that reverts because the sender or recipient is blocklisted by the token issuer, or because the destination is a destructed account, MUST be explained as such in the failure message.

#### Chain data service

- **FR-018**: The chain-data service MUST serve an Arc entry at `chains/eip155-5042.json` carrying: name, RPC list, native currency (`USDC`, 18 decimals), explorer, the RIP-7212 feature marker, the Safe proxy factory contract, EURC and USYC as stablecoins, and no wrapped-native token and no DEX (Arc has neither).
- **FR-019**: The chain-data service MUST also serve an Arc testnet entry at `chains/eip155-5042002.json`, so the custom-network path and verification work.
- **FR-020**: The chain-data service MUST be deployed to Cloudflare and answer at `https://ethereum-data.getvela.app`, serving the same paths (`/chains/*`, `/chainlogos/*`, `/erc7730/*`, `/api/*`, `/index/*`) as today.
- **FR-021**: The wallet's built-in chain-data URL MUST become `https://ethereum-data.getvela.app` in the core and in all four shells, including the chain-logo base URL.

#### Relay service

- **FR-022**: The relay MUST be deployed to Cloudflare and answer at `https://vela-relay-cf.getvela.app`, reporting the service identity `vela-relay` the wallet's health check expects.
- **FR-023**: The wallet's built-in bundler base URL MUST become `https://vela-relay-cf.getvela.app` for every chain, in the core and all four shells.
- **FR-024**: The relay MUST serve chain `5042`: resolving Arc RPCs, admitting Arc operations, funding its per-Safe relayer from an Arc treasury denominated in native USDC, and quoting Arc in-band fees through the ordinary native path.
- **FR-025**: The relay's own chain-directory source MUST point at `https://ethereum-data.getvela.app`.
- **FR-026**: The relay MUST NOT apply Tempo's `0x76` settlement path to Arc.
- **FR-027**: The relay MUST answer browser requests cross-origin. The wallet is a browser app; a relay without CORS headers is unreachable from the web shell no matter how healthy it is to `curl`.
- **FR-028**: The relay MUST expose its treasury status per chain, so the wallet can ask — before anyone signs — whether the relay can pay gas there.
- **FR-029**: When the relay cannot serve a chain (no treasury, or a treasury below the float floor), the wallet MUST say so before the person signs, naming the chain and what would fix it. It MUST NOT accept the operation and leave a person watching a spinner for a payment that can never land.

#### Compatibility and non-regression

- **FR-030**: A person's stored service-endpoint overrides MUST be preserved across this change; the new defaults apply only where no override exists.
- **FR-031**: The service-identity health checks MUST continue to accept the relocated services under their existing identities.
- **FR-032**: No change in this feature may alter fee amounts, balances or submission behaviour on the twelve existing built-in networks.

### Key Entities

- **Arc network**: chain id `5042`, native coin USDC (18 decimals on-chain, presented as USDC), deterministic finality, ~0.5 s blocks, 20 Gwei minimum base fee, permissioned validators.
- **Native USDC / ERC-20 USDC pair**: one balance with two interfaces — 18 decimals natively, 6 decimals through `0x3600…0000`. Not two assets.
- **Peg rule**: the mapping from a native gas symbol to a fixed $1 price, today implicit for Tempo's `USD`, to be made explicit and shared.
- **Per-chain minimum gas price**: the floor below which a chain discards a transaction; `0` for chains without one, 20 Gwei for Arc.
- **Chain-data entry**: the per-chain JSON the wallet and relay both consume for RPCs, stablecoins, DEX and contract addresses.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A person can complete a USDC send on Arc end to end — quote, passkey approval, submission, receipt — on all four shells, with the transfer visible on the Arc explorer.
- **SC-002**: 100% of Arc operations submitted by the wallet are accepted into the chain's mempool; none is discarded for a `maxFeePerGas` below the chain floor, including when the gas-price read fails.
- **SC-003**: The fee quoted for an Arc send is never below $0.01 and matches, within rounding, the fee the same send would be quoted on Gnosis for equivalent gas.
- **SC-004**: An Arc address's USDC balance shown in the wallet equals the on-chain balance exactly — not twice it — in the token list, the portfolio total and the send screen.
- **SC-005**: The twelve existing networks show identical balances and fee quotes before and after the service relocation, verified on at least Ethereum, Base, Gnosis and Tempo.
- **SC-006**: Both relocated services answer their health checks at the new hostnames with their expected service identities, and the wallet's Settings screen shows them healthy without any override.
- **SC-007**: The compatibility checker passes Arc mainnet and Arc testnet on all eleven required contracts plus the P-256 precompile.

## Assumptions

- The passkey/Safe/ERC-4337 signing path needs no change on Arc: verified on 2026-09-17 that all eleven required contracts are deployed at the canonical addresses on both Arc networks and that the RIP-7212 precompile at `0x100` validates a known-good P-256 signature.
- Arc's native USDC is worth $1 for display and fee purposes. The wallet does not model depeg; no other network in the wallet does either.
- Arc has no DEX and no Chainlink deployment the wallet can quote against at the time of writing; the peg is therefore the only price source, and the absence of a DEX is recorded in the chain-data entry rather than discovered at runtime.
- Cloudflare accounts, custom domains under `getvela.app`, and the Workers paid plan required by both services are available to the deployer; DNS for the two new hostnames can be pointed at the Workers.
- The relay's Arc treasury will be funded with native USDC out of band; this feature covers the code path, not the funding operation.
- `https://vela-relay.getvela.app` remains reachable during rollout so already-installed wallets are not stranded, but it is no longer the built-in default.
- Arc's opt-in privacy sector, post-quantum wallet signatures, multi-stablecoin gas and paymaster services are out of scope: they are not live on Arc at the time of writing.
