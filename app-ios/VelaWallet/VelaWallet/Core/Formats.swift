//
//  Formats.swift
//  VelaWallet
//
//  Numbers, dates and times from the product's own presets.
//
//  **Explicit rules, never the platform's formatter for the output.** The
//  reason survives the port from `app-web/.../services/locale-format.ts`: a
//  wallet that renders a figure differently depending on the device's idea of a
//  locale is a wallet where the same balance reads two ways on two machines,
//  and where a person cannot tell a thousands separator from a decimal point in
//  a language they are guessing at. The preset is chosen by the person and is
//  the same everywhere they open it.
//
//  `Locale` appears exactly once, in the `auto` DETECTION — reading the
//  device's conventions to pick a preset is a different act from letting it
//  format money, and the result is a preset like any other.
//
//  This includes money, which is why it is not cosmetic.
//

import Foundation

enum Formats {

    // MARK: - Presets

    private struct NumberStyle {
        let group: String
        let decimal: String
        /// 2-3 grouping: 12,34,567.
        let indian: Bool
    }

    private static func style(_ key: NumberFormatKey) -> NumberStyle {
        switch resolve(key) {
        case .dotComma: NumberStyle(group: ".", decimal: ",", indian: false)
        case .spaceComma: NumberStyle(group: " ", decimal: ",", indian: false)
        case .indian: NumberStyle(group: ",", decimal: ".", indian: true)
        default: NumberStyle(group: ",", decimal: ".", indian: false)
        }
    }

    // MARK: - `auto`, resolved once

    nonisolated(unsafe) private static var autoNumber: NumberFormatKey?
    nonisolated(unsafe) private static var autoDate: DateFormatKey?
    nonisolated(unsafe) private static var autoTime: TimeFormatKey?

    /// The device's grouping and decimal marks, as a preset.
    static func resolve(_ key: NumberFormatKey) -> NumberFormatKey {
        guard key == .auto else { return key }
        if let cached = autoNumber { return cached }
        let formatter = NumberFormatter()
        formatter.numberStyle = .decimal
        formatter.locale = .current
        let group = formatter.groupingSeparator ?? ","
        let decimal = formatter.decimalSeparator ?? "."
        let resolved: NumberFormatKey
        if decimal == "," && group == "." {
            resolved = .dotComma
        } else if decimal == "," {
            // Space grouping of any width — plain, non-breaking, or narrow.
            resolved = .spaceComma
        } else if formatter.secondaryGroupingSize == 2 {
            resolved = .indian
        } else {
            resolved = .commaDot
        }
        autoNumber = resolved
        return resolved
    }

    static func resolve(_ key: DateFormatKey) -> DateFormatKey {
        guard key == .auto else { return key }
        if let cached = autoDate { return cached }
        let template = DateFormatter.dateFormat(
            fromTemplate: "yMMdd", options: 0, locale: .current
        ) ?? "MM/dd/y"
        // The ORDER of the first letters, which is the only thing that
        // distinguishes the five presets.
        let order = template.filter { "yMd".contains($0) }
            .reduce(into: "") { out, character in
                let lower = Character(character.lowercased())
                if out.last != lower { out.append(lower) }
            }
        let dotted = template.contains(".")
        let dashed = template.contains("-")
        let resolved: DateFormatKey
        switch order {
        case "ymd": resolved = dashed ? .iso : .ymdSlash
        case "dmy": resolved = dotted ? .dmyDot : .dmySlash
        default: resolved = .mdySlash
        }
        autoDate = resolved
        return resolved
    }

    static func resolve(_ key: TimeFormatKey) -> TimeFormatKey {
        guard key == .auto else { return key }
        if let cached = autoTime { return cached }
        let template = DateFormatter.dateFormat(
            fromTemplate: "j", options: 0, locale: .current
        ) ?? "HH"
        let resolved: TimeFormatKey = template.contains("a") || template.contains("h") ? .h12 : .h24
        autoTime = resolved
        return resolved
    }

    /// The hermetic suite's seam. Detection caches on purpose — a preset that
    /// changed between two figures on one screen would be worse than either.
    static func resetAutoDetectionForTests() {
        autoNumber = nil
        autoDate = nil
        autoTime = nil
    }

    // MARK: - Numbers

    /// Group + decimal marks for a preset, for a caller that formats its own.
    static func separators(_ key: NumberFormatKey) -> (group: String, decimal: String) {
        let style = style(key)
        return (style.group, style.decimal)
    }

    /// Separators for seeding an EDITABLE numeric field: the preset's decimal
    /// mark, but NO grouping — thousands separators must not jump around while
    /// somebody is typing.
    static func inputSeparators(_ key: NumberFormatKey) -> (group: String, decimal: String) {
        ("", separators(key).decimal)
    }

    /// Somebody's typed figure, back to a plain decimal string.
    static func parse(_ text: String, _ key: NumberFormatKey) -> String {
        let marks = separators(key)
        var out = text.trimmingCharacters(in: .whitespaces)
        // Eastern Arabic and Persian digits, which a keyboard can produce.
        out = String(out.map { character in
            guard let scalar = character.unicodeScalars.first else { return character }
            switch scalar.value {
            case 0x0660...0x0669: return Character(String(scalar.value - 0x0660))
            case 0x06F0...0x06F9: return Character(String(scalar.value - 0x06F0))
            default: return character
            }
        })
        if marks.group == " " {
            out = out.filter { !$0.isWhitespace }
        } else if !marks.group.isEmpty {
            out = out.replacingOccurrences(of: marks.group, with: "")
        }
        if marks.decimal != "." {
            out = out.replacingOccurrences(of: marks.decimal, with: ".")
        }
        return out
    }

    /// Group an INTEGER DIGIT STRING without routing it through a `Double`.
    ///
    /// The bigint-safe entry point: a uint256 base-unit string must never
    /// become a floating-point number, because precision loss on a money
    /// surface is not a rounding error — it is a wrong figure shown to somebody
    /// about to sign.
    static func groupDigits(_ digits: String, _ key: NumberFormatKey) -> String {
        let style = style(key)
        return group(digits, separator: style.group, indian: style.indian)
    }

    private static func group(_ digits: String, separator: String, indian: Bool) -> String {
        guard digits.count > 3 else { return digits }
        let characters = Array(digits)
        if !indian {
            var parts: [String] = []
            var index = characters.count
            while index > 3 {
                parts.insert(String(characters[(index - 3)..<index]), at: 0)
                index -= 3
            }
            parts.insert(String(characters[0..<index]), at: 0)
            return parts.joined(separator: separator)
        }
        // 2-3 grouping: the last three, then twos.
        let tail = String(characters.suffix(3))
        var head = Array(characters.dropLast(3))
        var parts: [String] = []
        while head.count > 2 {
            parts.insert(String(head.suffix(2)), at: 0)
            head = Array(head.dropLast(2))
        }
        if !head.isEmpty { parts.insert(String(head), at: 0) }
        return (parts + [tail]).joined(separator: separator)
    }

    /// A DECIMAL STRING with the chosen preset's marks, without ever becoming a
    /// `Double`. This is what money goes through.
    static func decimal(_ value: String, _ key: NumberFormatKey) -> String {
        let style = style(key)
        var text = value.trimmingCharacters(in: .whitespaces)
        var sign = ""
        if text.hasPrefix("-") {
            sign = "-"
            text.removeFirst()
        }
        let parts = text.split(separator: ".", maxSplits: 1, omittingEmptySubsequences: false)
        let integer = String(parts.first ?? "0")
        let fraction = parts.count > 1 ? String(parts[1]) : ""
        guard integer.allSatisfy(\.isNumber), fraction.allSatisfy(\.isNumber) else { return value }
        let grouped = group(integer.isEmpty ? "0" : integer, separator: style.group, indian: style.indian)
        return sign + grouped + (fraction.isEmpty ? "" : style.decimal + fraction)
    }

    /// A number with the chosen preset.
    static func number(
        _ value: Double, _ key: NumberFormatKey,
        minimumFractionDigits: Int = 0, maximumFractionDigits: Int = 2
    ) -> String {
        guard value.isFinite else { return "0" }
        let maxFrac = max(0, maximumFractionDigits)
        let minFrac = min(max(0, minimumFractionDigits), maxFrac)
        let sign = value < 0 ? "-" : ""
        var fixed = String(format: "%.\(maxFrac)f", abs(value))
        if maxFrac > minFrac, fixed.contains(".") {
            while fixed.hasSuffix("0"),
                  fixed.split(separator: ".").last?.count ?? 0 > minFrac {
                fixed.removeLast()
            }
            if fixed.hasSuffix(".") { fixed.removeLast() }
        }
        return sign + decimal(fixed, key)
    }

    /// Compact form for large magnitudes: 1234567.89 → "1.23M".
    ///
    /// Latin suffixes, deliberately — locale myriads (万 / 億) are what a
    /// person would NOT see on the price feed they are comparing against.
    static func compact(_ value: Double, _ key: NumberFormatKey) -> String {
        guard value.isFinite else { return "0" }
        let sign = value < 0 ? "-" : ""
        let magnitude = abs(value)
        for tier in [(1e12, "T"), (1e9, "B"), (1e6, "M"), (1e3, "K")] where magnitude >= tier.0 {
            let scaled = magnitude / tier.0
            let frac = scaled < 10 ? 2 : (scaled < 100 ? 1 : 0)
            return sign + number(scaled, key, maximumFractionDigits: frac) + tier.1
        }
        return number(value, key, maximumFractionDigits: magnitude < 1 ? 4 : 2)
    }

    /// A token amount with magnitude-appropriate precision: ≥1000 → 2
    /// decimals · ≥1 → 4 · <1 → 6, so dust still reads as non-zero.
    static func tokenAmount(_ value: Double, _ key: NumberFormatKey, compact wantCompact: Bool = false) -> String {
        guard value.isFinite, value != 0 else { return "0" }
        let magnitude = abs(value)
        if wantCompact, magnitude >= 1e6 { return compact(value, key) }
        if magnitude >= 1000 {
            return number(value, key, minimumFractionDigits: 2, maximumFractionDigits: 2)
        }
        if magnitude >= 1 { return number(value, key, maximumFractionDigits: 4) }
        if !wantCompact { return number(value, key, maximumFractionDigits: 6) }
        // The glanceable form caps at 4 — but not when that would round a tiny
        // amount to "0", which prints a stray "+0" beside a real transfer.
        let capped = number(value, key, maximumFractionDigits: 4)
        return capped == "0" ? number(value, key, maximumFractionDigits: 6) : capped
    }

    // MARK: - Dates and times

    static func date(_ date: Date, _ key: DateFormatKey) -> String {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = .current
        let parts = calendar.dateComponents([.year, .month, .day], from: date)
        let year = String(parts.year ?? 1970)
        let month = pad(parts.month ?? 1)
        let day = pad(parts.day ?? 1)
        switch resolve(key) {
        case .ymdSlash: return "\(year)/\(month)/\(day)"
        case .iso: return "\(year)-\(month)-\(day)"
        case .dmySlash: return "\(day)/\(month)/\(year)"
        case .dmyDot: return "\(day).\(month).\(year)"
        default: return "\(month)/\(day)/\(year)"
        }
    }

    static func time(_ date: Date, _ key: TimeFormatKey) -> String {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = .current
        let parts = calendar.dateComponents([.hour, .minute], from: date)
        let hour = parts.hour ?? 0
        let minute = pad(parts.minute ?? 0)
        if resolve(key) == .h12 {
            let h12 = hour % 12 == 0 ? 12 : hour % 12
            return "\(h12):\(minute) \(hour < 12 ? "AM" : "PM")"
        }
        return "\(pad(hour)):\(minute)"
    }

    static func dateTime(_ date: Date, _ dateKey: DateFormatKey, _ timeKey: TimeFormatKey) -> String {
        "\(self.date(date, dateKey)), \(time(date, timeKey))"
    }

    private static func pad(_ value: Int) -> String {
        value < 10 ? "0\(value)" : String(value)
    }

    // MARK: - The pickers' examples

    /// 2026-06-13 13:45 — the one sample that distinguishes every order.
    static var sample: Date {
        var parts = DateComponents()
        parts.year = 2026
        parts.month = 6
        parts.day = 13
        parts.hour = 13
        parts.minute = 45
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = .current
        return calendar.date(from: parts) ?? Date(timeIntervalSince1970: 0)
    }

    /// The label on a format row IS a live example of it.
    static func example(_ key: NumberFormatKey) -> String {
        number(1234567.89, key, minimumFractionDigits: 2, maximumFractionDigits: 2)
    }

    static func example(_ key: DateFormatKey) -> String { date(sample, key) }
    static func example(_ key: TimeFormatKey) -> String { time(sample, key) }
}
