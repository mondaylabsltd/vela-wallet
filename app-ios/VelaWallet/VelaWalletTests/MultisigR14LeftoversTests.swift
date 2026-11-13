//
//  MultisigR14LeftoversTests.swift
//  VelaWalletTests
//
//  Round 14's iOS leftovers (docs/multisig-machines.md §19.12):
//
//  - A rate-limited Safe{Wallet} import is said as the core says it — busy,
//    its network named (`add.busy`), never "unavailable" — and every row the
//    core lists in `import.retry` offers Try again, the same `import` asked
//    again.
//  - r14-11a: a short address inside prose (a row's subtitle, a note, a
//    proposal's title) is never wrapped after its "…": the shell binds it
//    with word joiners when it draws it, and only then.
//

import CoreText
import Foundation
import Testing
import UIKit
import VelaCore
@testable import VelaWallet

@MainActor
struct MultisigR14LeftoversTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private var words: MsWords { MsWords(loc: loc, hub: nil) }

    private static let safe = MultisigViewsTests.safe
    private static let short = "0x777E98\u{2026}61875a"

    private func decode<T: Decodable>(_ type: T.Type, _ json: String) -> T? {
        MultisigMachine.decode(type, from: try! JSONSerialization.jsonObject(with: Data(json.utf8)))
    }

    private func outcome(_ json: String) -> MsJSON {
        decode(MsJSON.self, json)!
    }

    // MARK: - A rate-limited import

    @Test func theImportViewReadsWhichRowsOfferTryAgain() throws {
        let view = try #require(decode(MsImportView.self, """
        {"safes":[],"error":null,"busy":null,
         "outcomes":{"\(Self.safe.lowercased())":{"kind":"busy","chain_id":100}},
         "retry":["\(Self.safe.lowercased())"]}
        """))
        #expect(view.retry == [Self.safe.lowercased()])
        // The export's own spelling (EIP-55) finds its row.
        #expect(view.offersRetry(Self.safe))
        #expect(!view.offersRetry(MultisigViewsTests.one))
        // An older core (no `retry`): nothing offered.
        let old = try #require(decode(MsImportView.self, #"{"safes":[],"outcomes":{}}"#))
        #expect(old.retry.isEmpty)
        #expect(!old.offersRetry(Self.safe))
    }

    @Test func aBusyImportSaysTheNetworkIsBusyNeverUnavailable() {
        let busy = MultisigAddSheet.importStatus(outcome(#"{"kind":"busy","chain_id":100}"#), already: false, words: words)
        #expect(busy == words.t("add.busy", ["network": words.network(100)]))
        #expect(busy?.contains(words.network(100)) == true)
        #expect(busy != words.t("add.unavailable"))
        #expect(busy?.contains("{{") == false)
        // An unverified import names its network too (the add's words).
        let unverified = MultisigAddSheet.importStatus(outcome(#"{"kind":"unverified","chain_id":100}"#), already: false, words: words)
        #expect(unverified == words.t("authority.unverified", ["network": words.network(100)]))
        // The rest as before.
        #expect(MultisigAddSheet.importStatus(outcome(#"{"kind":"unavailable"}"#), already: false, words: words) == words.t("add.unavailable"))
        #expect(MultisigAddSheet.importStatus(outcome(#"{"kind":"imported"}"#), already: false, words: words) == words.t("import.imported"))
        #expect(MultisigAddSheet.importStatus(nil, already: true, words: words) == words.t("import.already"))
        #expect(MultisigAddSheet.importStatus(nil, already: false, words: words) == nil)
    }

    @Test func tryAgainOnAnImportRowAsksTheSameImportAgain() {
        let event = MultisigAddSheet.importEvent(Self.safe)
        #expect(event["type"] as? String == "import")
        #expect(event["address"] as? String == Self.safe)
        #expect(event["accept_risks"] as? Bool == false)
        #expect(event.count == 3)
        // Its words are the add's Try again.
        #expect(loc.t("home.multisig.review.state.tryAgain") == "Try again")
    }

    // MARK: - r14-11a: a short form inside prose stays whole

    @Test func aShortFormInProseIsBoundAndNothingElseIs() {
        let joiner = "\u{2060}"
        let bound = "0x777E98\(joiner)\u{2026}\(joiner)61875a"
        #expect(AddressText.bound(Self.short) == bound)
        #expect(AddressText.bound("Sent by \(Self.short) · Not deployed yet") == "Sent by \(bound) · Not deployed yet")
        #expect(AddressText.bound("由成员\(Self.short)发起") == "由成员\(bound)发起")
        // Two in one line.
        #expect(AddressText.bound("\(Self.short) → 0xd9fBf7\u{2026}5C4Fca") == "\(bound) → 0xd9fBf7\(joiner)\u{2026}\(joiner)5C4Fca")
        // Prose with an ellipsis of its own, a full address, plain text: untouched.
        #expect(AddressText.bound("Reading\u{2026}") == "Reading\u{2026}")
        #expect(AddressText.bound("正在导入\u{2026}") == "正在导入\u{2026}")
        #expect(AddressText.bound(Self.safe) == Self.safe)
        #expect(AddressText.bound("Team fund") == "Team fund")
        // Already bound: unchanged (drawing twice adds nothing).
        #expect(AddressText.bound(bound) == bound)
        // The core's short form itself is never changed — only what is drawn.
        let said = MsWords.short(Self.safe)
        #expect(!said.contains(joiner))
    }

    /// The line ends CoreText picks for `text` at `width` (UTF-16 offsets).
    private func lineEnds(_ text: String, font: UIFont, width: CGFloat) -> [Int] {
        let attributed = NSAttributedString(string: text, attributes: [.font: font])
        let setter = CTFramesetterCreateWithAttributedString(attributed)
        let path = CGPath(rect: CGRect(x: 0, y: 0, width: width, height: 10_000), transform: nil)
        let frame = CTFramesetterCreateFrame(setter, CFRange(location: 0, length: 0), path, nil)
        let lines = CTFrameGetLines(frame) as! [CTLine]
        return lines.dropLast().map { line in
            let range = CTLineGetStringRange(line)
            return range.location + range.length
        }
    }

    /// Whether some width from the short form's own up to 420 pt ends a
    /// line inside it. "Own" counts the punctuation that clings to it — in
    /// "(0x…)" the brackets never part from the address — below that width
    /// any engine must cut the unit somewhere, and that is no line break.
    private func breaksInside(_ text: String, short: String, font: UIFont) -> Int? {
        let ns = text as NSString
        let range = ns.range(of: short)
        precondition(range.location != NSNotFound)
        let clings = CharacterSet(charactersIn: "()[]\"'“”‘’,.:;!?")
        var unit = range
        while unit.location > 0, let s = Unicode.Scalar(ns.character(at: unit.location - 1)), clings.contains(s) {
            unit = NSRange(location: unit.location - 1, length: unit.length + 1)
        }
        while unit.location + unit.length < ns.length,
              let s = Unicode.Scalar(ns.character(at: unit.location + unit.length)), clings.contains(s) {
            unit.length += 1
        }
        // The unit as the line sets it: the system face spaces Latin apart
        // from the CJK beside it, so its room is measured with its neighbours
        // and their own widths taken off again.
        let measure = { (piece: String) in (piece as NSString).size(withAttributes: [.font: font]).width }
        let before = unit.location > 0 ? ns.substring(with: NSRange(location: unit.location - 1, length: 1)) : ""
        let after = unit.location + unit.length < ns.length
            ? ns.substring(with: NSRange(location: unit.location + unit.length, length: 1)) : ""
        let set = measure(before + ns.substring(with: unit) + after) - measure(before) - measure(after)
        let own = Int(max(set, measure(ns.substring(with: unit))).rounded(.up)) + 2
        for width in own...420 {
            for end in lineEnds(text, font: font, width: CGFloat(width))
            where end > range.location && end < range.location + range.length {
                return width
            }
        }
        return nil
    }

    @Test func aBoundShortFormIsNeverWrappedAfterItsEllipsisAtAnyWidth() {
        let zh = [
            "由成员\(Self.short)发起的交易还需要另外 1 位成员批准，然后才能执行。",
            "\(Self.short) · 尚未部署 · 已导入",
            "向\(Self.short)转账 1.5 USDC",
        ]
        let en = "Sent by Parallel One (\(Self.short)) · waiting for 1 more approval"
        var controls = 0
        for font in [Typography.rowSub.uiFont, Typography.body.uiFont, Typography.flowCaption.uiFont] {
            for text in zh + [en] {
                let bound = AddressText.bound(text)
                let boundShort = AddressText.bound(Self.short)
                #expect(breaksInside(bound, short: boundShort, font: font) == nil, "\(text) at \(font.pointSize) pt")
                if breaksInside(text, short: Self.short, font: font) != nil { controls += 1 }
            }
        }
        // The control: the same lines unbound DO break after the "…" somewhere.
        #expect(controls > 0)
    }

    /// Every multisig screen draws its words through the binding: the only
    /// verbatim texts left are links and what is shared (never prose drawn
    /// here, and copied or sent as it is).
    @Test func everyMultisigTextDrawsShortFormsWhole() throws {
        let views = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("VelaWallet/Features/Multisig/Views")
        let files = try FileManager.default.contentsOfDirectory(at: views, includingPropertiesForKeys: nil)
            .filter { $0.pathExtension == "swift" }
        #expect(!files.isEmpty)
        var stray: [String] = []
        for file in files {
            let source = try String(contentsOf: file, encoding: .utf8)
            for (n, line) in source.components(separatedBy: "\n").enumerated()
            where line.contains("Text(verbatim:") && !line.contains("Text(verbatim: link)") && !line.contains("ShareLink(") {
                stray.append("\(file.lastPathComponent):\(n + 1)")
            }
        }
        #expect(stray.isEmpty, "draw with Text(prose:): \(stray)")
    }
}
