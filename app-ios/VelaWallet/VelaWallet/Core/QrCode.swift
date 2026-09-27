//
//  QrCode.swift
//  VelaWallet
//
//  A code a camera can actually read.
//
//  The encoder is the bridge's — `cable_qr_matrix`, the same one that draws the
//  `FIDO:/` code during a hybrid passkey ceremony (spec 019). One encoder for
//  every code this app shows, so a receive QR and a pairing QR cannot disagree
//  about版本, masking or error correction, and neither is a second QR library
//  somebody has to keep.
//
//  ## Why this matters more than it looks
//
//  Until this landed the receive screen drew `QrPattern` — a deterministic demo
//  pattern, captioned 不可扫描 in the gallery — beside an address. The pattern
//  is honest in a mock and dangerous over a real address: money sent to the
//  wrong place does not come back, and a person holding up a phone does not
//  read captions.
//

import Foundation
import VelaCore

enum QrCode {

    /// `text` as a module grid, row-major, or `nil` when it cannot be encoded.
    ///
    /// `nil` is a real answer and the caller must render it as one — the drawn
    /// demo pattern is NOT a fallback here. A code that looks scannable and is
    /// not is the failure this file exists to prevent.
    static func modules(_ text: String) -> [[Bool]]? {
        guard !text.isEmpty else { return nil }
        return grid(cableQrMatrix(text: text))
    }

    /// The saved share card's code: the same encoder at error-correction
    /// level H (`shareCardQrMatrix`).
    ///
    /// The card puts the network's logo on a plate in the middle of the code,
    /// and the picture is recompressed by every chat app it passes through;
    /// level H recovers 30% where the screen's level M recovers 15%. A plain
    /// address comes out 37 modules across instead of 29. The receive SCREEN
    /// keeps `modules` — a code on a lit phone is read straight off the glass.
    static func shareModules(_ text: String) -> [[Bool]]? {
        guard !text.isEmpty else { return nil }
        return grid(shareCardQrMatrix(text: text))
    }

    private static func grid(_ matrix: QrMatrix?) -> [[Bool]]? {
        guard let matrix else { return nil }
        let width = Int(matrix.width)
        guard width > 0, matrix.modules.count == width * width else { return nil }
        return (0..<width).map { row in
            Array(matrix.modules[(row * width)..<((row + 1) * width)])
        }
    }
}
