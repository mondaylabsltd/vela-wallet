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
        guard !text.isEmpty, let matrix = cableQrMatrix(text: text) else { return nil }
        let width = Int(matrix.width)
        guard width > 0, matrix.modules.count == width * width else { return nil }
        return (0..<width).map { row in
            Array(matrix.modules[(row * width)..<((row + 1) * width)])
        }
    }
}
