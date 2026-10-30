//
//  DataScannerView.swift
//  VelaWallet
//
//  VisionKit's live scanner behind the viewfinder, where the phone has it
//  (native parity N-I2).
//
//  `DataScannerViewController` is the system's own: it focuses, zooms and
//  guides ("Move closer"), it finds a code at an angle or in poor light that
//  a frame-by-frame detector misses, and it never hands the app a camera
//  frame — only the code's text. It needs the Neural Engine (A12 and later)
//  and is never available on a simulator, so the AVFoundation scanner
//  (`CameraScanner`'s capture session, `QrDecoder`) stays the fallback and
//  the hermetic suite's path.
//
//  Whatever reads the code, the text goes to the SAME place: `onCode`, once,
//  and the core decides what it means (`scan_resolved`).
//

import AVFoundation
import SwiftUI
import Vision
import VisionKit

/// Whether this phone's live scanner can be used — `nil` decides by the
/// device; a test says.
enum DataScannerSupport {
    /// The device's answer: supported hardware, and nothing (a restriction,
    /// a denial) standing in the way.
    @MainActor
    static var deviceAllows: Bool {
        DataScannerViewController.isSupported && DataScannerViewController.isAvailable
    }

    /// The scanner a session uses: VisionKit's when the phone allows it and
    /// the session may open a camera at all (never a fixture session's).
    static func usesDataScanner(fixtureOnly: Bool, deviceAllows: Bool) -> Bool {
        !fixtureOnly && deviceAllows
    }
}

/// The live scanner, as a view: QR codes only, one at a time, its own
/// guidance on, its highlights off (the brackets are the app's).
struct DataScannerView: UIViewControllerRepresentable {
    let onCode: (String) -> Void

    func makeUIViewController(context: Context) -> DataScannerViewController {
        let controller = DataScannerViewController(
            recognizedDataTypes: [.barcode(symbologies: [.qr])],
            qualityLevel: .balanced,
            recognizesMultipleItems: false,
            isHighFrameRateTrackingEnabled: false,
            isPinchToZoomEnabled: true,
            isGuidanceEnabled: true,
            isHighlightingEnabled: false
        )
        controller.delegate = context.coordinator
        try? controller.startScanning()
        return controller
    }

    func updateUIViewController(_ controller: DataScannerViewController, context: Context) {
        context.coordinator.onCode = onCode
    }

    static func dismantleUIViewController(_ controller: DataScannerViewController, coordinator: Coordinator) {
        controller.stopScanning()
    }

    func makeCoordinator() -> Coordinator { Coordinator(onCode: onCode) }

    final class Coordinator: NSObject, DataScannerViewControllerDelegate {
        var onCode: (String) -> Void

        init(onCode: @escaping (String) -> Void) {
            self.onCode = onCode
        }

        func dataScanner(_ dataScanner: DataScannerViewController, didAdd addedItems: [RecognizedItem],
                         allItems: [RecognizedItem]) {
            for item in addedItems {
                if case .barcode(let code) = item, let text = code.payloadStringValue, !text.isEmpty {
                    onCode(text)
                    return
                }
            }
        }
    }
}
