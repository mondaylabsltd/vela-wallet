//
//  Eip681.swift
//  VelaWallet
//
//  `ethereum:` payment URIs, tokenised.
//
//  Ported from `app-desktop/.../flows/eip681.rs`, which is itself the web's
//  `parseEIP681` — the grammar lives in the shells because Hermes has no
//  WebAssembly, so the native tier runs its own parser and a core one could
//  only ever be a second implementation. This is the fourth, and it keeps the
//  two refusals that were bought with real bugs.
//
//  This file only TOKENISES. Whether a scan locks the send screen, whether a
//  chainless request may lock at all, and how base units become a figure are
//  all `send`'s (`scan_resolved`).
//

import Foundation

/// A parsed request, in the shape the core's `SendScan::Request` takes.
struct Eip681: Equatable {
    let recipient: String
    let chainId: Int?
    /// `nil` = the chain's own coin.
    let tokenAddress: String?
    /// Base units, as a decimal string — the wire shape.
    let amountBaseUnits: String?

    /// The core's event payload for this request.
    var scan: [String: Any] {
        [
            "type": "request",
            "recipient": recipient,
            "chain_id": chainId.map { $0 as Any } ?? NSNull(),
            "token_address": tokenAddress.map { $0 as Any } ?? NSNull(),
            "amount_base_units": amountBaseUnits.map { $0 as Any } ?? NSNull(),
        ]
    }

    /// What a scan of arbitrary text means to the core: a request when it
    /// parses, the raw text otherwise — which the send screen drops into an
    /// editable recipient field rather than refusing outright.
    static func scan(of text: String) -> [String: Any] {
        parse(text)?.scan ?? ["type": "text", "data": text]
    }

    static func isHexAddress(_ value: String) -> Bool {
        let trimmed = value.trimmingCharacters(in: .whitespaces)
        return trimmed.count == 42 && trimmed.hasPrefix("0x")
            && trimmed.dropFirst(2).allSatisfy(\.isHexDigit)
    }

    /// `ethereum:<target>[@chain][/function][?params]`, tolerantly — the legacy
    /// `pay-` prefix and scientific-notation amounts included.
    ///
    /// `nil` means "this is not a payment request".
    static func parse(_ input: String) -> Eip681? {
        let trimmed = input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.count >= 9,
              trimmed.prefix(9).lowercased() == "ethereum:"
        else { return nil }
        var rest = String(trimmed.dropFirst(9))
        if rest.hasPrefix("pay-") { rest = String(rest.dropFirst(4)) }

        let path: String
        let query: String
        if let mark = rest.firstIndex(of: "?") {
            path = String(rest[rest.startIndex..<mark])
            query = String(rest[rest.index(after: mark)...])
        } else {
            path = rest
            query = ""
        }
        let params = parseQuery(query)

        let targetWithChain: String
        let functionName: String
        if let slash = path.firstIndex(of: "/") {
            targetWithChain = String(path[path.startIndex..<slash])
            functionName = String(path[path.index(after: slash)...])
        } else {
            targetWithChain = path
            functionName = ""
        }
        let target: String
        let chainText: String
        if let at = targetWithChain.firstIndex(of: "@") {
            target = String(targetWithChain[targetWithChain.startIndex..<at])
                .trimmingCharacters(in: .whitespaces)
            chainText = String(targetWithChain[targetWithChain.index(after: at)...])
        } else {
            target = targetWithChain.trimmingCharacters(in: .whitespaces)
            chainText = ""
        }
        guard !target.isEmpty else { return nil }
        let chainId = (!chainText.isEmpty && chainText.allSatisfy(\.isNumber))
            ? Int(chainText) : nil

        if functionName == "transfer" {
            let recipient = (params.first { $0.key == "address" }?.value ?? "")
                .trimmingCharacters(in: .whitespaces)
            guard isHexAddress(target), isHexAddress(recipient) else { return nil }
            // `uint256` ONLY. In a `/transfer` URI, EIP-681's `value` is the
            // ether sent ALONG WITH the call, not the token argument — reading
            // it as token base units mixed two units and two decimals, and a
            // URI saying "attach 1 ETH" (value=1e18) prefilled a LOCKED send of
            // 10^12 USDC.
            let amount = params.first { $0.key == "uint256" }.flatMap { parseAmount($0.value) }
            return Eip681(
                recipient: recipient, chainId: chainId,
                tokenAddress: target, amountBaseUnits: amount
            )
        }

        // Any OTHER function is not a payment this can honour. It used to fall
        // through to the native branch, where `target` — the contract the call
        // was addressed to — became the RECIPIENT:
        // `ethereum:<token>@1/approve?…` opened a locked send of the chain's
        // coin to the token contract, which is a burn dressed as the payment
        // somebody thought they scanned.
        guard functionName.isEmpty, isHexAddress(target) else { return nil }

        return Eip681(
            recipient: target, chainId: chainId, tokenAddress: nil,
            amountBaseUnits: params.first { $0.key == "value" }.flatMap { parseAmount($0.value) }
        )
    }

    private static func parseQuery(_ query: String) -> [(key: String, value: String)] {
        query.split(separator: "&").compactMap { pair in
            let text = String(pair)
            guard !text.isEmpty else { return nil }
            guard let equals = text.firstIndex(of: "=") else {
                return (decodeComponent(text), "")
            }
            return (
                decodeComponent(String(text[text.startIndex..<equals])),
                decodeComponent(String(text[text.index(after: equals)...]))
            )
        }
    }

    /// `decodeURIComponent`, for the parts of it a payment URI can carry. A
    /// malformed escape is kept verbatim rather than dropped — losing a
    /// character silently is how an address becomes a different address.
    private static func decodeComponent(_ raw: String) -> String {
        raw.removingPercentEncoding ?? raw
    }

    /// An integer amount, accepting decimals and scientific notation, as a
    /// decimal STRING.
    ///
    /// A negative amount answers `nil` rather than zero: a request for minus
    /// one coin is not a request for none of them, it is one this cannot
    /// honour.
    static func parseAmount(_ raw: String) -> String? {
        var rest = raw.trimmingCharacters(in: .whitespaces)
        guard !rest.isEmpty else { return nil }
        if rest.hasPrefix("-") { return nil }
        if rest.hasPrefix("+") { rest = String(rest.dropFirst()) }

        var exponent = 0
        if let marker = rest.firstIndex(where: { $0 == "e" || $0 == "E" }) {
            guard let parsed = Int(rest[rest.index(after: marker)...]) else { return nil }
            exponent = parsed
            rest = String(rest[rest.startIndex..<marker])
        }
        let intDigits: String
        let fracDigits: String
        if let dot = rest.firstIndex(of: ".") {
            intDigits = String(rest[rest.startIndex..<dot])
            fracDigits = String(rest[rest.index(after: dot)...])
        } else {
            intDigits = rest
            fracDigits = ""
        }
        guard !(intDigits.isEmpty && fracDigits.isEmpty),
              (intDigits + fracDigits).allSatisfy(\.isNumber)
        else { return nil }

        let digits = String((intDigits + fracDigits).drop { $0 == "0" })
        let value = digits.isEmpty ? "0" : digits
        let decimalExponent = exponent - fracDigits.count

        if decimalExponent >= 0 {
            // A cap, not a policy: an exponent nobody could mean is refused
            // rather than turned into a megabyte of zeros.
            guard decimalExponent <= 78 else { return nil }
            return value == "0" ? "0" : value + String(repeating: "0", count: decimalExponent)
        }
        // Negative exponent: truncate, because base units are integral.
        let drop = -decimalExponent
        guard drop < value.count else { return "0" }
        return String(value.prefix(value.count - drop))
    }
}
