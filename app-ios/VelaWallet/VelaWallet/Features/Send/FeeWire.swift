//
//  FeeWire.swift
//  VelaWallet
//
//  The `fee_policy` machine's view model, in Swift.
//
//  Everything here is a figure the core computed and this client must not
//  re-derive. The tiers, the in-band markup, the Tempo split, the reserves and
//  the staleness clock are 2,322 lines of Rust; what crosses is the result and
//  the one gate consumers AND into their confirm button.
//
//  **The bundler is the gas-price authority and the wallet never vetoes its
//  quote** — a standing rule from the wallet↔bundler parity work, restated here
//  because this file is where a shell would be tempted to clamp one.
//

import Foundation

/// What a fee is paid in.
///
/// A tagged union, so it decodes by hand: `type` is the discriminator and the
/// `erc20` arm carries the contract the fee leg actually transfers.
enum FeeAssetWire: Equatable {
    case native
    case erc20(token: String, decimals: Int, amount: String, symbol: String?)
}

extension FeeAssetWire: Decodable {
    private enum Key: String, CodingKey { case type, token, decimals, amount, symbol }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: Key.self)
        switch try container.decode(String.self, forKey: .type) {
        case "native":
            self = .native
        case "erc20":
            self = .erc20(
                token: try container.decode(String.self, forKey: .token),
                decimals: try container.decode(Int.self, forKey: .decimals),
                amount: try container.decode(String.self, forKey: .amount),
                symbol: try container.decodeIfPresent(String.self, forKey: .symbol)
            )
        case let other:
            throw DecodingError.dataCorruptedError(
                forKey: .type, in: container,
                debugDescription: "unknown fee asset \(other)"
            )
        }
    }
}

/// One settled estimate.
///
/// `total_wei` is the fee in the NATIVE coin's base units even when the fee is
/// paid in an ERC-20 — the ERC-20 figure is `fee_asset`'s own `amount`, in that
/// token's decimals. Reading the first where the second belongs prints a
/// stablecoin fee as an eighteen-decimal number.
struct FeeEstimateWire: Decodable, Equatable {
    let chainId: Int
    let totalWei: String
    let maxFeePerGas: String
    let totalGas: String
    let deployed: Bool
    /// Whether the relay quoted this, as against the core's local fallback.
    let quoted: Bool
    let feeAsset: FeeAssetWire
    /// Where an in-band fee leg pays. Travels to `submit_user_op` as half of
    /// the displayed-equals-signed gate.
    let feeRecipient: String?
}

/// One row of the fee-token sheet.
struct FeeOptionWire: Decodable, Equatable {
    let symbol: String
    /// `nil` = the native coin.
    let contract: String?
    let decimals: Int
    /// Raw base units of what the account holds of this asset.
    let balance: String
    let recipient: String
    let usdBalance: String
    let usdPrice: String?
    /// What the fee would cost in this asset. `nil` = not quotable.
    let amount: String?
    /// The core's verdict, not a comparison this shell makes.
    let insufficient: Bool
    let selected: Bool
}

struct FeeViewWire: Decodable, Equatable {
    /// Estimating or requoting. The confirm slide must stay disabled while it
    /// is true (the core's invariant ⑦).
    let busy: Bool
    /// `missing_public_key` / `fee_token_unavailable` / `quote_unavailable` /
    /// `calculation_failed` / `estimate_failed` / `gas_quote_too_high`.
    let failed: String?
    /// Present only when valid for the form's CURRENT chain. A quote for the
    /// chain somebody just left is withheld rather than shown.
    let fee: FeeEstimateWire?
    /// The 30-second TTL elapsed. Advisory: the shell offers a refresh.
    let stale: Bool
    let feeToken: String?
    let options: [FeeOptionWire]
    /// The single gate. Consumers AND this into their confirm button rather
    /// than assembling their own conjunction of `busy`, `failed` and `fee`.
    let confirmFeeReady: Bool
}
