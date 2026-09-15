//
//  CameraPreviewView.swift
//  VelaWallet
//
//  The viewfinder itself — a `UIView` whose backing layer IS the preview.
//
//  `layerClass` rather than a sublayer added in `makeUIView`: a sublayer has to
//  be resized by hand on every bounds change, and the one that gets forgotten
//  is the rotation. A backing layer is laid out by UIKit for free.
//

import AVFoundation
import SwiftUI
import UIKit

struct CameraPreviewView: UIViewRepresentable {
    let session: AVCaptureSession

    func makeUIView(context: Context) -> PreviewView {
        let view = PreviewView()
        view.previewLayer.session = session
        // Fill, not fit: a letterboxed viewfinder inside a square frame makes
        // the aiming brackets lie about what the camera can see.
        view.previewLayer.videoGravity = .resizeAspectFill
        return view
    }

    func updateUIView(_ view: PreviewView, context: Context) {
        if view.previewLayer.session !== session { view.previewLayer.session = session }
    }

    final class PreviewView: UIView {
        override class var layerClass: AnyClass { AVCaptureVideoPreviewLayer.self }
        var previewLayer: AVCaptureVideoPreviewLayer {
            // Safe by `layerClass` above.
            layer as! AVCaptureVideoPreviewLayer
        }
    }
}
