//
//  BalanceWire.swift
//  VelaWallet
//
//  The `balance_dashboard` machine's view model, in Swift.
//
//  Same rule as every other `*Wire.swift`: views are `Decodable` through
//  `CoreJSON.decoder`; operations and results stay dictionaries.
//
//  ## The one field worth reading the Rust for
//
//  `BalanceToken.balance` is a **human decimal string** — the core says so at
//  `balance_dashboard.rs:298`, "human decimal string (never a JSON number)".
//  Desktop's 031 wrote raw units there and the defect was invisible for as long
//  as prices were `None`; the first real price would have multiplied the total
//  by 10^18.
//

import Foundation

/// One asset the person holds.
struct BalanceTokenWire: Decodable, Equatable {
    let chainId: Int
    let symbol: String
    let name: String
    /// **A human decimal string**, not raw units and not a number. `"0.75897"`.
    let balance: String
    let decimals: Int
    /// `nil` = the chain's native coin.
    let tokenAddress: String?
    /// `nil` = nothing could price it. **Never read as 1.**
    let priceUsd: Double?
    let spam: Bool
}

/// One account switcher row, and the cached total behind it.
struct BalanceCacheEntryWire: Decodable, Equatable {
    let address: String
    let usd: Double
}

/// What the home screen says when it cannot say a number.
enum BalanceNoticeWire: String, Decodable {
    /// A fetch is still running over a cached figure.
    case stillUpdating = "still_updating"
    /// Held, but nothing could price it — the amount shows, the fiat does not.
    case unpriced
}

/// One network the last read could not reach, and what was last read there
/// (spec 092). The core orders the list and picks each row's sentence.
struct UnreachableNetworkWire: Decodable, Equatable {
    let chainId: Int
    /// `held` / `empty` / `not_read`.
    let lastKnown: String
    /// The worth of what it last held, USD. `nil` when nothing priced was
    /// held — and while privacy hides it, like the hero's figure.
    let lastSeenUsd: Double?
    /// The corpus key of the row's line; `assets.lastSeen` fills `{{amount}}`.
    let lineKey: String
}

struct BalanceSwitcherViewWire: Decodable, Equatable {
    let open: Bool
    let loading: Bool
    let balances: [BalanceCacheEntryWire]
}

struct BalanceViewWire: Decodable, Equatable {
    /// `nil` before an account is known — the neutral surface's trigger.
    let address: String?
    /// `nil` means **no total can be stated**, which is not the same as zero
    /// and must never render as `$0.00`.
    let displayTotalUsd: Double?
    /// Nothing is known yet.
    let balanceUnknown: Bool
    /// Some chains answered and some did not. The total is a floor, not a sum,
    /// and the screen has to say so rather than quietly under-reporting.
    let balancePartial: Bool
    /// Nothing could be read and nothing is known: the first fetch failed with
    /// no cache behind it. Optional so a view written before the field (a test
    /// fixture, an older core) still decodes; `nil` reads as "not unreachable".
    var unreachable: Bool? = nil
    let notice: BalanceNoticeWire?
    /// The person tapped to hide the figure. A display state the core owns so
    /// it survives a relaunch.
    let hidden: Bool
    let refreshing: Bool
    let lastRefreshedAtMs: Double?
    let tokens: [BalanceTokenWire]
    /// Held but unpriceable — listed separately so the total stays honest.
    let unpricedTokens: [BalanceTokenWire]
    let failedChainIds: [Int]
    /// The transient subset. A chain here keeps its cached balance and must
    /// **never** be offered the "swap in your own RPC" banner (invariant ④).
    let rateLimitedChainIds: [Int]
    /// Every network the wallet cannot reach (spec 092), held or not, in the
    /// core's order. The core always sends it (a JSON fixture must too); the
    /// default only serves Swift-built test views.
    var unreachableNetworks: [UnreachableNetworkWire] = []
    /// The corpus key of the home line over them (`assets.unreachableOne` /
    /// `assets.unreachableMany`); `nil` when every network answered.
    var unreachableKey: String? = nil
    let holdingsLoading: Bool
    let cachedTotalUsd: Double?
    let switcher: BalanceSwitcherViewWire
}
