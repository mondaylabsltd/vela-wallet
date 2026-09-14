//
//  XlsxMatrix.swift
//  VelaWallet
//
//  An Excel workbook, flattened to a grid of strings.
//
//  A payroll file is one of the two ways a person brings a list of people into
//  this wallet, and on a phone it is the more likely one: the file arrived in
//  an email. The core parses the grid; this only gets the grid out of the zip.
//
//  ## What it does not do, and why that is written down
//
//  **No CRC check** — parsing the XML is a stronger integrity test than a
//  checksum, and a corrupt sheet fails as a parse rather than as a number
//  nobody can act on. **No Zip64** — a spreadsheet of recipients is not four
//  gigabytes. Both are deliberate, and a file that needs either fails cleanly
//  rather than half-working.
//
//  ## The central directory, not the local headers
//
//  Zip writers routinely set the data-descriptor flag and leave the local
//  header's sizes as zero, filling them in after the compressed bytes. So the
//  sizes are read from the central directory, which always has them, and the
//  local header is used only to find where the data starts.
//
//  Ported alongside `app-android/.../feature/send/core/XlsxMatrix.kt`
//  (spec 045 research D5).
//

import Foundation

enum XlsxMatrix {

    /// The first worksheet as rows of cell strings.
    ///
    /// `nil` means this is not a workbook this build can read — never an empty
    /// grid, which the core would take as "a file with no recipients in it".
    static func rows(from data: Data) -> [[String]]? {
        guard let entries = zipEntries(data) else { return nil }
        guard let sheet = entries.first(where: { $0.name.hasSuffix("xl/worksheets/sheet1.xml") })
            ?? entries.first(where: { $0.name.contains("worksheets/sheet") })
        else { return nil }
        guard let xml = inflate(entry: sheet, in: data).flatMap({ String(data: $0, encoding: .utf8) })
        else { return nil }

        // `sharedStrings` is optional: a workbook written by a tool that
        // inlines its strings has none, and the desktop's own fixture is one.
        let shared = entries.first { $0.name.hasSuffix("xl/sharedStrings.xml") }
            .flatMap { inflate(entry: $0, in: data) }
            .flatMap { String(data: $0, encoding: .utf8) }
            .map(sharedStrings) ?? []

        return sheetRows(xml: xml, shared: shared)
    }

    // MARK: - The sheet

    /// One `<row>` per line, one `<c>` per cell, **positioned by its
    /// reference**.
    ///
    /// A row with a gap — an address and no amount, which the desktop's
    /// fixture deliberately contains — writes no `<c>` for the empty cell. A
    /// reader that appended cells in order would slide every later value one
    /// column left and hand somebody's address to the amount parser.
    static func sheetRows(xml: String, shared: [String]) -> [[String]] {
        var rows: [[String]] = []
        for rowXml in blocks(in: xml, tag: "row") {
            var cells: [String: String] = [:]
            var widest = 0
            for cellXml in blocks(in: rowXml, tag: "c", allowSelfClosing: true) {
                let reference = attribute("r", in: cellXml) ?? ""
                let column = columnIndex(ofReference: reference)
                guard column >= 0 else { continue }
                widest = max(widest, column + 1)
                cells[String(column)] = cellValue(cellXml, shared: shared)
            }
            guard widest > 0 else { continue }
            rows.append((0..<widest).map { cells[String($0)] ?? "" })
        }
        return rows
    }

    /// `t="s"` indexes the shared table; `t="inlineStr"` carries its own
    /// `<is><t>`; everything else is the raw `<v>`.
    private static func cellValue(_ cell: String, shared: [String]) -> String {
        let type = attribute("t", in: cell)
        if type == "inlineStr" { return text(in: cell) }
        let raw = blocks(in: cell, tag: "v").first.map(text(in:)) ?? ""
        if type == "s", let index = Int(raw), shared.indices.contains(index) {
            return shared[index]
        }
        return raw
    }

    static func sharedStrings(_ xml: String) -> [String] {
        blocks(in: xml, tag: "si").map(text(in:))
    }

    /// `"B3"` → 1. Letters are the column, digits are the row.
    static func columnIndex(ofReference reference: String) -> Int {
        var column = 0
        var sawLetter = false
        for character in reference.uppercased() {
            guard let ascii = character.asciiValue else { return -1 }
            if ascii >= 65, ascii <= 90 {
                column = column * 26 + Int(ascii - 64)
                sawLetter = true
            } else {
                break
            }
        }
        return sawLetter ? column - 1 : -1
    }

    // MARK: - A very small XML reader

    /// The bodies of every `<tag …>…</tag>` at any depth, in document order.
    ///
    /// Deliberately not an XML parser. A worksheet is machine-written, deeply
    /// regular, and the three shapes below are all of it; `XMLParser` would be
    /// a delegate, a state machine and four times the code for a grid of
    /// strings.
    static func blocks(in xml: String, tag: String, allowSelfClosing: Bool = false) -> [String] {
        var found: [String] = []
        var index = xml.startIndex
        let open = "<\(tag)"
        let close = "</\(tag)>"

        while let start = xml.range(of: open, range: index..<xml.endIndex) {
            // `<c…` must not match `<cols…`: the next character has to end the
            // tag name.
            let after = start.upperBound
            guard after < xml.endIndex else { break }
            let next = xml[after]
            guard next == ">" || next == " " || next == "/" else {
                index = after
                continue
            }
            guard let headEnd = xml.range(of: ">", range: after..<xml.endIndex) else { break }
            let head = String(xml[start.lowerBound..<headEnd.upperBound])

            if head.hasSuffix("/>") {
                if allowSelfClosing { found.append(head) }
                index = headEnd.upperBound
                continue
            }
            guard let bodyEnd = xml.range(of: close, range: headEnd.upperBound..<xml.endIndex)
            else { break }
            found.append(String(xml[start.lowerBound..<bodyEnd.upperBound]))
            index = bodyEnd.upperBound
        }
        return found
    }

    static func attribute(_ name: String, in element: String) -> String? {
        guard let key = element.range(of: "\(name)=\"") else { return nil }
        guard let end = element.range(of: "\"", range: key.upperBound..<element.endIndex)
        else { return nil }
        return String(element[key.upperBound..<end.lowerBound])
    }

    /// Every `<t>` inside, concatenated and unescaped. A cell split across
    /// runs — Excel does that for mixed formatting — is one string.
    static func text(in element: String) -> String {
        let runs = blocks(in: element, tag: "t")
        let joined = runs.isEmpty
            ? inner(of: element)
            : runs.map(inner(of:)).joined()
        return joined
            .replacingOccurrences(of: "&lt;", with: "<")
            .replacingOccurrences(of: "&gt;", with: ">")
            .replacingOccurrences(of: "&quot;", with: "\"")
            .replacingOccurrences(of: "&apos;", with: "'")
            .replacingOccurrences(of: "&amp;", with: "&")
    }

    private static func inner(of element: String) -> String {
        guard let headEnd = element.range(of: ">"),
              let tailStart = element.range(of: "</", options: .backwards)
        else { return "" }
        guard headEnd.upperBound <= tailStart.lowerBound else { return "" }
        return String(element[headEnd.upperBound..<tailStart.lowerBound])
    }

    // MARK: - The zip

    struct Entry {
        let name: String
        let method: UInt16
        let compressedSize: Int
        let localOffset: Int
    }

    static func zipEntries(_ data: Data) -> [Entry]? {
        guard let eocd = lastIndex(of: 0x0605_4b50, in: data) else { return nil }
        guard eocd + 20 <= data.count else { return nil }
        let count = Int(u16(data, eocd + 10))
        let directory = Int(u32(data, eocd + 16))
        guard directory < data.count else { return nil }

        var entries: [Entry] = []
        var cursor = directory
        for _ in 0..<count {
            guard cursor + 46 <= data.count, u32(data, cursor) == 0x0201_4b50 else { break }
            let method = u16(data, cursor + 10)
            let compressed = Int(u32(data, cursor + 20))
            let nameLength = Int(u16(data, cursor + 28))
            let extraLength = Int(u16(data, cursor + 30))
            let commentLength = Int(u16(data, cursor + 32))
            let offset = Int(u32(data, cursor + 42))
            let nameStart = cursor + 46
            guard nameStart + nameLength <= data.count else { break }
            let name = String(
                data: data.subdata(in: nameStart..<(nameStart + nameLength)), encoding: .utf8
            ) ?? ""
            entries.append(Entry(
                name: name, method: method, compressedSize: compressed, localOffset: offset
            ))
            cursor = nameStart + nameLength + extraLength + commentLength
        }
        return entries.isEmpty ? nil : entries
    }

    static func inflate(entry: Entry, in data: Data) -> Data? {
        let header = entry.localOffset
        guard header + 30 <= data.count, u32(data, header) == 0x0403_4b50 else { return nil }
        let nameLength = Int(u16(data, header + 26))
        let extraLength = Int(u16(data, header + 28))
        let start = header + 30 + nameLength + extraLength
        let end = start + entry.compressedSize
        guard end <= data.count else { return nil }
        let payload = data.subdata(in: start..<end)

        switch entry.method {
        case 0: return payload
        case 8:
            // Apple's `.zlib` is RFC 1951 raw DEFLATE — which is what a zip
            // stores, despite the name suggesting the RFC 1950 wrapper.
            return try? (payload as NSData).decompressed(using: .zlib) as Data
        default: return nil
        }
    }

    private static func lastIndex(of signature: UInt32, in data: Data) -> Int? {
        guard data.count >= 4 else { return nil }
        // The comment is at most 64 KiB, so the record is within the last
        // 64 KiB + 22 bytes. Scanning further would only find a signature
        // inside compressed data.
        let floor = max(0, data.count - (65_536 + 22))
        var index = data.count - 4
        while index >= floor {
            if u32(data, index) == signature { return index }
            index -= 1
        }
        return nil
    }

    private static func u16(_ data: Data, _ offset: Int) -> UInt16 {
        guard offset + 2 <= data.count else { return 0 }
        return UInt16(data[data.startIndex + offset])
            | (UInt16(data[data.startIndex + offset + 1]) << 8)
    }

    private static func u32(_ data: Data, _ offset: Int) -> UInt32 {
        guard offset + 4 <= data.count else { return 0 }
        var value: UInt32 = 0
        for byte in (0..<4).reversed() {
            value = (value << 8) | UInt32(data[data.startIndex + offset + byte])
        }
        return value
    }
}
