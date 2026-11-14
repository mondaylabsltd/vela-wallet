// Apple's QR readers over the lab corpus, as the iOS app calls them.
//
//   cidetector  CoreImage CIDetector (QR, accuracy high) on the image as it is —
//               exactly QrDecoder.swift's photo path (and its camera fallback);
//   cidetector-2x  the same, then a small picture once more at double size;
//   vision      Vision's VNDetectBarcodesRequest, QR only, for reference.
//
// macOS runs the same CoreImage/Vision frameworks as iOS (not bit-identical
// builds, the same algorithms). Times are native, GPU where the framework uses
// one: they do not compare with the wasm timings in V8.
//
//   swiftc -O main.swift -o ../target/apple-qr && ../target/apple-qr [--only <category>]
// writes out/apple-results.json; bench/merge-external.mjs folds it into results.json.

import CoreImage
import Foundation
import Vision

let lab = URL(fileURLWithPath: CommandLine.arguments[0]).deletingLastPathComponent().deletingLastPathComponent()
let corpus = lab.appendingPathComponent("out/corpus")
let args = CommandLine.arguments
let only = args.firstIndex(of: "--only").map { args[$0 + 1] }

/// A `.gray.gz` from the corpus: gzip = 10-byte header + raw DEFLATE + 8-byte trailer.
func luma(_ file: String) -> Data {
    let gz = try! Data(contentsOf: corpus.appendingPathComponent(file))
    let deflate = gz.subdata(in: 10..<(gz.count - 8))
    return try! (deflate as NSData).decompressed(using: .zlib) as Data
}

func cgImage(_ bytes: Data, _ w: Int, _ h: Int) -> CGImage {
    let provider = CGDataProvider(data: bytes as CFData)!
    return CGImage(width: w, height: h, bitsPerComponent: 8, bitsPerPixel: 8, bytesPerRow: w,
                   space: CGColorSpaceCreateDeviceGray(), bitmapInfo: CGBitmapInfo(rawValue: 0),
                   provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent)!
}

// QrDecoder.swift's detector, verbatim.
let detector = CIDetector(ofType: CIDetectorTypeQRCode,
                          context: CIContext(options: [.useSoftwareRenderer: false]),
                          options: [CIDetectorAccuracy: CIDetectorAccuracyHigh])!

func ciDetector(_ image: CGImage) -> String? {
    let features = detector.features(in: CIImage(cgImage: image)) as? [CIQRCodeFeature] ?? []
    return features.lazy.compactMap { $0.messageString }.first { !$0.isEmpty }
}

/// The candidate iOS change: CIDetector as today, then — for a small picture —
/// once more at double size (Lanczos), which is what reads modules under ~2 px.
func ciDetectorUpscaled(_ image: CGImage) -> String? {
    if let found = ciDetector(image) { return found }
    guard max(image.width, image.height) < 640 else { return nil }
    let doubled = CIImage(cgImage: image).applyingFilter("CILanczosScaleTransform", parameters: [kCIInputScaleKey: 2.0, kCIInputAspectRatioKey: 1.0])
    let features = detector.features(in: doubled) as? [CIQRCodeFeature] ?? []
    return features.lazy.compactMap { $0.messageString }.first { !$0.isEmpty }
}

func vision(_ image: CGImage) -> String? {
    let request = VNDetectBarcodesRequest()
    request.symbologies = [.qr]
    try? VNImageRequestHandler(cgImage: image, options: [:]).perform([request])
    return request.results?.lazy.compactMap { $0.payloadStringValue }.first { !$0.isEmpty }
}

func run(_ read: (CGImage) -> String?, _ image: CGImage, _ expected: String) -> [String: Any] {
    let t0 = DispatchTime.now().uptimeNanoseconds
    let text = read(image)
    let ms = Double(DispatchTime.now().uptimeNanoseconds - t0) / 1e6
    var r: [String: Any] = ["calls": 1, "ms": ms, "firstMs": ms]
    switch text {
    case nil: r["status"] = "miss"
    case .some(let t) where t == expected: r["status"] = "hit"
    case .some(let t): r["status"] = "wrong"; r["got"] = String(t.prefix(120))
    }
    return r
}

let manifest = try! JSONSerialization.jsonObject(with: Data(contentsOf: corpus.appendingPathComponent("manifest.json"))) as! [String: Any]
var out: [[String: Any]] = []
// Warm both up once: the first call pays for pipeline compilation.
let warm = cgImage(Data(count: 64 * 64), 64, 64)
_ = ciDetector(warm); _ = vision(warm)

for s in manifest["samples"] as! [[String: Any]] {
    let category = s["category"] as! String
    if let only, only != category { continue }
    let expected = s["expected"] as! String
    let original = cgImage(luma(s["file"] as! String), s["width"] as! Int, s["height"] as! Int)
    let cam = s["camera"] as! [String: Any]
    let frame = cgImage(luma(cam["file"] as! String), cam["w"] as! Int, cam["h"] as! Int)
    var results: [String: Any] = [:]
    for (name, read) in [("cidetector", ciDetector), ("cidetector-2x", ciDetectorUpscaled), ("vision", vision)] as [(String, (CGImage) -> String?)] {
        results[name] = ["still": run(read, original, expected), "camera": run(read, frame, expected)]
    }
    out.append(["id": s["id"]!, "results": results])
    FileHandle.standardError.write(".".data(using: .utf8)!)
}
let file = lab.appendingPathComponent("out/apple-results.json")
try! JSONSerialization.data(withJSONObject: ["samples": out]).write(to: file)
FileHandle.standardError.write("\n\(out.count) samples → \(file.path)\n".data(using: .utf8)!)
