//
//  XlsxMatrixTests.swift
//  VelaWalletTests
//
//  The workbook reader, against a real workbook.
//
//  The fixture is the desktop's own payroll sample, read through `#filePath`
//  rather than copied — one file, one truth, and a change to it fails here
//  rather than drifting.
//

import Foundation
import Testing
@testable import VelaWallet

struct XlsxMatrixTests {

    private static let repoRoot = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()   // VelaWalletTests
        .deletingLastPathComponent()   // app-ios/VelaWallet
        .deletingLastPathComponent()   // app-ios
        .deletingLastPathComponent()   // repo root

    private func payrollSample() throws -> Data {
        try Data(contentsOf: Self.repoRoot.appendingPathComponent(
            "app-desktop/vela-wallet/tests/fixtures/payroll-sample.xlsx"
        ))
    }

    /// A real workbook, read end to end.
    ///
    /// Deflate entries, inline strings, no `sharedStrings` — which is the
    /// shape a tool that writes its own workbooks produces, and the one a
    /// reader that assumed a shared table would fail on.
    @Test func aRealWorkbookComesBackAsAGrid() throws {
        let rows = XlsxMatrix.rows(from: try payrollSample())
        #expect(rows != nil, "the desktop's own fixture must be readable")
        #expect(rows?.count == 4, "a header and three rows")
        #expect(rows?.first == ["address", "amount"])
        #expect(rows?[1] == ["0x031d7D57c99CAF891e1C250554691Fd12D84772b", "5000"])
        #expect(rows?[2] == ["0x88cCA0EeDbF2C4426110bbFc998F048689266894", "173.88"])
    }

    /// **A gap stays a gap.**
    ///
    /// The fixture's last row has an address and no amount, and writes no cell
    /// for the empty one. A reader that appended cells in order would give
    /// that row one column, slide nothing — but on a row missing a MIDDLE
    /// cell it would hand somebody's address to the amount parser. Positions
    /// come from the `r` reference for exactly that reason.
    @Test func aMissingCellDoesNotSlideTheOnesAfterIt() throws {
        let rows = XlsxMatrix.rows(from: try payrollSample())
        #expect(rows?.last?.first == "0xee2cca98ecbff34663591a925968fa4db5a1f0dd")

        // A synthetic middle gap: A1 and C1, nothing at B1.
        let sheet = """
        <worksheet><sheetData><row r="1">\
        <c r="A1" t="inlineStr"><is><t>left</t></is></c>\
        <c r="C1" t="inlineStr"><is><t>right</t></is></c>\
        </row></sheetData></worksheet>
        """
        #expect(XlsxMatrix.sheetRows(xml: sheet, shared: []) == [["left", "", "right"]])
    }

    @Test func aColumnReferenceIsItsLetters() {
        #expect(XlsxMatrix.columnIndex(ofReference: "A1") == 0)
        #expect(XlsxMatrix.columnIndex(ofReference: "B3") == 1)
        #expect(XlsxMatrix.columnIndex(ofReference: "Z9") == 25)
        #expect(XlsxMatrix.columnIndex(ofReference: "AA1") == 26)
        #expect(XlsxMatrix.columnIndex(ofReference: "9") == -1)
        #expect(XlsxMatrix.columnIndex(ofReference: "") == -1)
    }

    /// A shared-string workbook is the other half of the format.
    @Test func sharedStringsAreResolvedByIndex() {
        let shared = XlsxMatrix.sharedStrings(
            "<sst><si><t>alpha</t></si><si><t>beta</t></si></sst>"
        )
        #expect(shared == ["alpha", "beta"])

        let sheet = """
        <worksheet><sheetData><row r="1">\
        <c r="A1" t="s"><v>1</v></c><c r="B1"><v>42</v></c>\
        </row></sheetData></worksheet>
        """
        #expect(XlsxMatrix.sheetRows(xml: sheet, shared: shared) == [["beta", "42"]])
    }

    /// A run split across formatting is one string, and entities come back as
    /// characters.
    @Test func aSplitRunIsOneStringAndEntitiesAreDecoded() {
        #expect(XlsxMatrix.text(in: "<is><t>Ali</t><t>ce &amp; Bob</t></is>") == "Alice & Bob")
        #expect(XlsxMatrix.text(in: "<t>a &lt;b&gt; c</t>") == "a <b> c")
    }

    /// `<c>` must not be found inside `<cols>`.
    @Test func aTagNameIsMatchedWholeNotAsAPrefix() {
        let sheet = """
        <worksheet><cols><col min="1" max="1"/></cols><sheetData><row r="1">\
        <c r="A1" t="inlineStr"><is><t>only</t></is></c>\
        </row></sheetData></worksheet>
        """
        #expect(XlsxMatrix.sheetRows(xml: sheet, shared: []) == [["only"]])
    }

    /// Anything that is not a workbook answers **nothing**, never an empty
    /// grid — the core reads an empty grid as a file with no recipients in it,
    /// which is a different and much worse answer than "I could not read
    /// this".
    @Test func somethingThatIsNotAWorkbookAnswersNothing() {
        #expect(XlsxMatrix.rows(from: Data("address,amount\n0xabc,1".utf8)) == nil)
        #expect(XlsxMatrix.rows(from: Data()) == nil)
        #expect(XlsxMatrix.rows(from: Data(repeating: 0x50, count: 4096)) == nil)
    }
}
