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
    /// The speed this estimate was priced at — `fast` / `standard` / `slow`
    /// (spec 069). The speed control reads it to tell one tier's figure from
    /// another's, and the submission names it on the wire.
    var tier: String = "fast"
    /// What this tier bids per gas now, and how high it will go — the two ends
    /// of the gas bid the speed control draws (issues 684/685), wei as decimal
    /// strings. `nil` means nothing honest to show, never 0.
    var effectiveGasPrice: String? = nil
    var maxGasPrice: String? = nil
    /// The per-gas figures the estimate was priced with. Carried whole so the
    /// estimate handed back to the core (the send machine, the speed machine)
    /// is the one it produced, not a copy with holes in it.
    var networkFeePerGas: String = "0"
    var relayerFeePerGas: String = "0"
    var bundlerGasPrice: String = "0"
    var inBandGasBasis: String = "0"

    private enum CodingKeys: String, CodingKey {
        case chainId, totalWei, maxFeePerGas, totalGas, deployed, quoted, feeAsset, feeRecipient
        case tier, effectiveGasPrice, maxGasPrice, networkFeePerGas, relayerFeePerGas
        case bundlerGasPrice, inBandGasBasis
    }

    init(
        chainId: Int, totalWei: String, maxFeePerGas: String, totalGas: String, deployed: Bool,
        quoted: Bool, feeAsset: FeeAssetWire, feeRecipient: String?, tier: String = "fast",
        effectiveGasPrice: String? = nil, maxGasPrice: String? = nil
    ) {
        self.chainId = chainId
        self.totalWei = totalWei
        self.maxFeePerGas = maxFeePerGas
        self.totalGas = totalGas
        self.deployed = deployed
        self.quoted = quoted
        self.feeAsset = feeAsset
        self.feeRecipient = feeRecipient
        self.tier = tier
        self.effectiveGasPrice = effectiveGasPrice
        self.maxGasPrice = maxGasPrice
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        chainId = try c.decode(Int.self, forKey: .chainId)
        totalWei = try c.decode(String.self, forKey: .totalWei)
        maxFeePerGas = try c.decode(String.self, forKey: .maxFeePerGas)
        totalGas = try c.decode(String.self, forKey: .totalGas)
        deployed = try c.decode(Bool.self, forKey: .deployed)
        quoted = try c.decode(Bool.self, forKey: .quoted)
        feeAsset = try c.decode(FeeAssetWire.self, forKey: .feeAsset)
        feeRecipient = try c.decodeIfPresent(String.self, forKey: .feeRecipient)
        tier = try c.decodeIfPresent(String.self, forKey: .tier) ?? "fast"
        effectiveGasPrice = try c.decodeIfPresent(String.self, forKey: .effectiveGasPrice)
        maxGasPrice = try c.decodeIfPresent(String.self, forKey: .maxGasPrice)
        networkFeePerGas = try c.decodeIfPresent(String.self, forKey: .networkFeePerGas) ?? "0"
        relayerFeePerGas = try c.decodeIfPresent(String.self, forKey: .relayerFeePerGas) ?? "0"
        bundlerGasPrice = try c.decodeIfPresent(String.self, forKey: .bundlerGasPrice) ?? "0"
        inBandGasBasis = try c.decodeIfPresent(String.self, forKey: .inBandGasBasis) ?? "0"
    }

    /// Back out as the core's own `FeeEstimateView` — for the send machine and
    /// the speed machine alike. A re-encode, never a computation: it came from
    /// `fee_policy` and this shell must not reshape it on the way past.
    var coreJSON: [String: Any] {
        let asset: [String: Any]
        switch feeAsset {
        case .native:
            asset = ["type": "native"]
        case .erc20(let token, let decimals, let amount, let symbol):
            asset = [
                "type": "erc20", "token": token, "decimals": decimals, "amount": amount,
                "symbol": symbol.map { $0 as Any } ?? NSNull(),
            ]
        }
        return [
            "chain_id": chainId,
            "total_wei": totalWei,
            "max_fee_per_gas": maxFeePerGas,
            "network_fee_per_gas": networkFeePerGas,
            "relayer_fee_per_gas": relayerFeePerGas,
            "bundler_gas_price": bundlerGasPrice,
            "in_band_gas_basis": inBandGasBasis,
            "effective_gas_price": effectiveGasPrice.map { $0 as Any } ?? NSNull(),
            "max_gas_price": maxGasPrice.map { $0 as Any } ?? NSNull(),
            "total_gas": totalGas,
            "deployed": deployed,
            "tier": tier,
            "quoted": quoted,
            "fee_asset": asset,
            "fee_recipient": feeRecipient.map { $0 as Any } ?? NSNull(),
        ]
    }
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
    /// Spec 096 F2: the operation itself may spend this coin by an amount no
    /// call states, and nothing measured what is left — the core's
    /// `spent_by_operation`. The sheet warns while it is the coin paying.
    /// Optional on the wire so a hand-written row without it reads `false`.
    var spentByOperation: Bool?
}

struct FeeViewWire: Decodable, Equatable {
    /// Estimating or requoting. The confirm slide must stay disabled while it
    /// is true (the core's invariant ⑦).
    let busy: Bool
    /// `missing_public_key` / `fee_token_unavailable` / `quote_unavailable` /
    /// `calculation_failed` / `estimate_failed` / `gas_quote_too_high` /
    /// `would_fail` (spec 083 fee; not said to this shell yet) — or, since
    /// spec 082 (RJ13), the object form `{"chain_read":{"rate_limited":…}}`
    /// kept as its JSON text. Either way it is what the core's
    /// `feeRequoteDelayMs` and `feeFailureReasonKey` take back, so this shell
    /// never reads it apart (`FeeFailureText`).
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

extension FeeViewWire {
    private enum CodingKeys: String, CodingKey {
        case busy, failed, fee, stale, feeToken, options, confirmFeeReady
    }

    /// Written out for `failed` alone: a `FeeFailure` is a string for every
    /// variant but `chain_read`, which is an object (spec 082 RJ13). Decoding
    /// it as a `String` failed the whole fee view the day the core grew it.
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        busy = try c.decode(Bool.self, forKey: .busy)
        failed = try c.decodeIfPresent(FeeFailureText.self, forKey: .failed)?.text
        fee = try c.decodeIfPresent(FeeEstimateWire.self, forKey: .fee)
        stale = try c.decode(Bool.self, forKey: .stale)
        feeToken = try c.decodeIfPresent(String.self, forKey: .feeToken)
        options = try c.decode([FeeOptionWire].self, forKey: .options)
        confirmFeeReady = try c.decode(Bool.self, forKey: .confirmFeeReady)
    }
}

/// `fee_policy::FeeFailure` as the text the core's own functions take back:
/// the wire name for a string variant, the compact JSON for the object one
/// (`{"chain_read":{"rate_limited":true}}`). Nothing here reads it apart.
struct FeeFailureText: Decodable, Equatable {
    let text: String

    init(_ text: String) { self.text = text }

    /// The failure a shell produces itself (spec 082 RJ13): a chain read the
    /// fee is blocked on — the account's deployment — got no answer, or a
    /// rate limit. The fee machine never emits it.
    static func chainRead(rateLimited: Bool) -> FeeFailureText {
        FeeFailureText(#"{"chain_read":{"rate_limited":\#(rateLimited)}}"#)
    }

    init(from decoder: Decoder) throws {
        let single = try decoder.singleValueContainer()
        if let name = try? single.decode(String.self) {
            text = name
            return
        }
        let object = try single.decode(JSONValue.self)
        text = object.compactJSON
    }

    /// For a log line: the wire name, or `chain_read` with its one flag —
    /// never the braces.
    var cause: String {
        guard text.hasPrefix("{") else { return text }
        if text.contains("\"chain_read\"") {
            return text.contains("true") ? "chain_read(rate_limited)" : "chain_read"
        }
        return "other"
    }
}

/// Just enough JSON to re-encode an object this shell does not read apart.
private indirect enum JSONValue: Decodable {
    case string(String), number(Double), bool(Bool), null
    case array([JSONValue]), object([(String, JSONValue)])

    private struct Key: CodingKey {
        var stringValue: String
        var intValue: Int? { nil }
        init(stringValue: String) { self.stringValue = stringValue }
        init?(intValue: Int) { nil }
    }

    init(from decoder: Decoder) throws {
        if let keyed = try? decoder.container(keyedBy: Key.self) {
            // `convertFromSnakeCase` rewrote the keys on the way in; the core
            // wants its own spelling back.
            self = .object(try keyed.allKeys.sorted { $0.stringValue < $1.stringValue }.map {
                (JSONValue.snake($0.stringValue), try keyed.decode(JSONValue.self, forKey: $0))
            })
            return
        }
        if var list = try? decoder.unkeyedContainer() {
            var items: [JSONValue] = []
            while !list.isAtEnd { items.append(try list.decode(JSONValue.self)) }
            self = .array(items)
            return
        }
        let single = try decoder.singleValueContainer()
        if single.decodeNil() { self = .null }
        else if let flag = try? single.decode(Bool.self) { self = .bool(flag) }
        else if let number = try? single.decode(Double.self) { self = .number(number) }
        else { self = .string(try single.decode(String.self)) }
    }

    private static func snake(_ camel: String) -> String {
        var out = ""
        for character in camel {
            if character.isUppercase {
                out += "_" + character.lowercased()
            } else {
                out.append(character)
            }
        }
        return out
    }

    var compactJSON: String {
        switch self {
        case .string(let text):
            let data = (try? JSONSerialization.data(withJSONObject: text, options: [.fragmentsAllowed])) ?? Data()
            return String(decoding: data, as: UTF8.self)
        case .number(let number):
            return number == number.rounded() && abs(number) < 1e15 ? String(Int64(number)) : String(number)
        case .bool(let flag): return flag ? "true" : "false"
        case .null: return "null"
        case .array(let items): return "[" + items.map(\.compactJSON).joined(separator: ",") + "]"
        case .object(let pairs):
            return "{" + pairs.map { "\(JSONValue.string($0.0).compactJSON):\($0.1.compactJSON)" }
                .joined(separator: ",") + "}"
        }
    }
}

// MARK: - The speed control (spec 069)

/// One option of the speed control, as the `fee_speed` core decided it.
struct FeeSpeedOptionWire: Decodable, Equatable {
    /// `fast` / `standard` / `slow`.
    let tier: String
    let selected: Bool
    /// This tier's OWN settled quote — formatted exactly as the fee row
    /// formats one, with the fee-coin options of the session pricing it.
    let fee: FeeEstimateWire?
    /// No figure of its own but one is coming: "…". Neither: "—".
    let measuring: Bool
    /// Its gas bid as a range, formatted by the core over the whole set.
    let gasPrice: String?
}

/// The `fee_speed` core's view (spec 069): every decision the speed control
/// shows — the tier in force, the free upgrade, the one-speed statement, and
/// which other tiers must be kept priced.
struct FeeSpeedViewWire: Decodable, Equatable {
    /// The tier THIS send runs at: the session in force prices it, the
    /// submission names it.
    let tier: String
    let preferred: String
    /// The other tiers the shell must keep priced.
    let previews: [String]
    let open: Bool
    let picked: Bool
    let free: Bool
    let freeNote: Bool
    let single: Bool
    let gasPriceLine: Bool
    let options: [FeeSpeedOptionWire]
}
