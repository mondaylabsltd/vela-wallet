//
//  PhotoPicker.swift
//  VelaWallet
//
//  One photo, for the scanner to read.
//
//  `PHPickerViewController` runs OUT OF PROCESS and needs **no** photo
//  permission: the person picks in Apple's UI and this app receives only what
//  they chose. That is what keeps the album entry in `Info.plist` the narrowest
//  one Photos offers — add-only, for saving a receive card (spec 051) — and a
//  wallet asking to READ somebody's photos in order to scan one QR code would
//  be asking for far more than it needs.
//

import Foundation
import PhotosUI
import UIKit

@MainActor
enum PhotoPicker {

    /// The picked image, or `nil` if they changed their mind.
    static func pick() async -> UIImage? {
        guard let presenter = topmost() else { return nil }
        var configuration = PHPickerConfiguration()
        configuration.filter = .images
        configuration.selectionLimit = 1
        let picker = PHPickerViewController(configuration: configuration)
        let delegate = Delegate()
        picker.delegate = delegate
        return await withCheckedContinuation { continuation in
            delegate.finish = { image in
                // The delegate is kept alive by this closure until it fires,
                // and released after — `PHPickerViewController` holds its
                // delegate weakly.
                _ = delegate
                continuation.resume(returning: image)
            }
            presenter.present(picker, animated: true)
        }
    }

    private static func topmost() -> UIViewController? {
        UIApplication.shared.connectedScenes
            .compactMap { $0 as? UIWindowScene }
            .first { $0.activationState == .foregroundActive }?
            .windows
            .first { $0.isKeyWindow }?
            .rootViewController?
            .topmost
    }

    private final class Delegate: NSObject, PHPickerViewControllerDelegate {
        var finish: ((UIImage?) -> Void)?
        /// Answered exactly once, whichever way the sheet ends.
        private var answered = false

        func picker(_ picker: PHPickerViewController, didFinishPicking results: [PHPickerResult]) {
            picker.dismiss(animated: true)
            guard let provider = results.first?.itemProvider,
                  provider.canLoadObject(ofClass: UIImage.self)
            else {
                answer(nil)
                return
            }
            provider.loadObject(ofClass: UIImage.self) { [weak self] object, _ in
                Task { @MainActor in self?.answer(object as? UIImage) }
            }
        }

        @MainActor private func answer(_ image: UIImage?) {
            guard !answered else { return }
            answered = true
            finish?(image)
            finish = nil
        }
    }
}
