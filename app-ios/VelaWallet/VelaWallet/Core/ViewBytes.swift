//
//  ViewBytes.swift
//  VelaWallet
//
//  A core answer's view, by its bytes (native parity Phase 4).
//
//  Every answer a Crux machine gives — to an event or to an effect — is
//  `{"view":<v>,"effects":[…],"cancelled_effect_ids":[…]}` (the uniffi bridge's
//  `DispatchResult`, the view first), and `<v>` is the machine's WHOLE view.
//  Most answers leave it as it was: a pool call settled, a name was not found.
//  Parsing and decoding a 200-row feed again to learn that was most of what a
//  launch spent on the main thread. `split` finds `<v>`'s bytes without
//  parsing anything, so `CoreDriver` can compare them with the last view's
//  and parse only the effects when they are the same.
//
//  The core writes its views with serde_json, whose maps are ordered: the same
//  view is the same bytes. A view this cannot split (another shape, a scalar)
//  answers `nil`, and the caller parses the whole answer as it always did.
//

import Foundation

nonisolated enum ViewBytes {

    struct Split {
        /// The view's bytes, less every ignored `"key":<value>` — compared,
        /// never parsed.
        let comparable: [UInt8]
        /// The answer without its view: `{"effects":…}`.
        let rest: String
    }

    private static let quote = UInt8(ascii: "\"")
    private static let backslash = UInt8(ascii: "\\")
    private static let comma = UInt8(ascii: ",")
    private static let prefix = Array(#"{"view":"#.utf8)

    /// `{"view":<v>,<rest>}` → `<v>`'s comparable bytes and `{<rest>}`.
    ///
    /// `ignoring`: top-level fields of the view this shell never reads, cut
    /// out before the comparison (the pool's `pending_urls`).
    static func split(_ text: String, ignoring keys: [String] = []) -> Split? {
        var text = text
        let patterns = keys.map { Array("\"\($0)\":".utf8) }
        return text.withUTF8 { bytes -> Split? in
            guard bytes.count > prefix.count,
                  memcmp(bytes.baseAddress!, prefix, prefix.count) == 0
            else { return nil }
            let start = prefix.count
            guard let end = valueEnd(bytes, from: start), end < bytes.count, bytes[end] == comma, end > start
            else { return nil }
            // The view's own fields at depth one, less the ignored ones.
            var comparable = [UInt8]()
            comparable.reserveCapacity(end - start)
            var cursor = start
            if !patterns.isEmpty {
                for range in ignoredRanges(bytes, start: start, end: end, patterns: patterns) {
                    comparable.append(contentsOf: UnsafeBufferPointer(rebasing: bytes[cursor..<range.lowerBound]))
                    cursor = range.upperBound
                }
            }
            comparable.append(contentsOf: UnsafeBufferPointer(rebasing: bytes[cursor..<end]))
            let restBytes = UnsafeBufferPointer(rebasing: bytes[(end + 1)...])
            let rest = "{" + String(decoding: restBytes, as: UTF8.self)
            return Split(comparable: comparable, rest: rest)
        }
    }

    /// Where the JSON value starting at `from` ends: the index just past it,
    /// for an object or an array; the first top-level comma or closer for a
    /// scalar. `nil` when the text ends first.
    private static func valueEnd(_ bytes: UnsafeBufferPointer<UInt8>, from: Int) -> Int? {
        var index = from
        var depth = 0
        let base = bytes.baseAddress!
        while index < bytes.count {
            let byte = bytes[index]
            if byte == quote {
                // A string: jump to its closing quote — the next one not
                // escaped by an odd run of backslashes. Most of a view's
                // bytes are inside strings, and `memchr` crosses them fast.
                var at = index + 1
                while true {
                    guard at < bytes.count,
                          let hit = memchr(base + at, Int32(quote), bytes.count - at)
                    else { return nil }
                    let found = base.distance(to: hit.assumingMemoryBound(to: UInt8.self))
                    var slashes = 0
                    while found - 1 - slashes > index, bytes[found - 1 - slashes] == backslash { slashes += 1 }
                    at = found + 1
                    if slashes % 2 == 0 { break }
                }
                index = at
                if depth == 0 { return index }
                continue
            } else {
                switch byte {
                case UInt8(ascii: "{"), UInt8(ascii: "["):
                    depth += 1
                case UInt8(ascii: "}"), UInt8(ascii: "]"):
                    if depth == 0 { return index }
                    depth -= 1
                    if depth == 0 { return index + 1 }
                case comma:
                    if depth == 0 { return index }
                default:
                    break
                }
            }
            index += 1
        }
        return nil
    }

    /// The ranges of `"key":<value>` for each pattern found at the view's top
    /// level, in order.
    private static func ignoredRanges(
        _ bytes: UnsafeBufferPointer<UInt8>, start: Int, end: Int, patterns: [[UInt8]]
    ) -> [Range<Int>] {
        // Walk the view's top-level keys: `{` then `"key":value` pairs.
        guard bytes[start] == UInt8(ascii: "{") else { return [] }
        var ranges: [Range<Int>] = []
        var index = start + 1
        while index < end, bytes[index] == quote {
            let keyStart = index
            guard let keyEnd = valueEnd(bytes, from: index), keyEnd < end, bytes[keyEnd] == UInt8(ascii: ":"),
                  let valueEnd = valueEnd(bytes, from: keyEnd + 1)
            else { return ranges }
            let keyLength = keyEnd + 1 - keyStart
            if patterns.contains(where: { pattern in
                pattern.count == keyLength && memcmp(bytes.baseAddress! + keyStart, pattern, keyLength) == 0
            }) {
                ranges.append(keyStart..<valueEnd)
            }
            index = valueEnd
            guard index < end, bytes[index] == comma else { break }
            index += 1
        }
        return ranges
    }
}
