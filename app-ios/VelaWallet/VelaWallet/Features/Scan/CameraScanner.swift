//
//  CameraScanner.swift
//  VelaWallet
//
//  The camera behind the scanner surface (spec 055 US3).
//
//  Four rules here were bought on other clients and each one is load-bearing:
//
//  1. **The session is configured and started off the main thread.**
//     `startRunning` blocks — sometimes for most of a second — and doing it on
//     the main queue freezes the sheet as it appears.
//  2. **The rotation is set on the connection**, or every frame arrives on its
//     side and a sideways code reads as no code.
//  3. **A hit is delivered ONCE.** Without the flag a code decodes thirty times
//     a second and the send screen is re-locked on every frame.
//  4. **No camera is not an error.** The simulator has none; a
//     `DiscoverySession` simply comes back empty, and the surface falls back to
//     the photo library rather than showing a failure nobody can act on.
//
//  And a fifth, bought on the desktop (PR 3 note 7, PRIVACY):
//
//  5. **A gallery, fixture, board or dev-flow session never opens the
//     camera.** A desktop gallery sweep started the real camera and captured
//     a frame of the person at the machine. Under any `VELA_PAGE` override
//     (`PageOverride.isDevSession`) this scanner is `fixtureOnly`: `start()`
//     returns before it asks AVFoundation anything — no device discovery, no
//     permission prompt, no session, no `startRunning` — and the surface
//     draws a FIXTURE frame, a drawn viewfinder with the sample code
//     (`ScanSurfaceView.fixtureFrame`). The gate is here, at the one place
//     the camera is started, so no caller can forget it.
//

import AVFoundation
import Foundation

@MainActor
@Observable
final class CameraScanner: NSObject {

    /// Why the viewfinder is not showing. Four answers and they are four
    /// different sentences — a person who denied permission and a person on a
    /// device with no camera need different things said to them.
    enum Refusal: Equatable {
        case denied
        case restricted
        case noCamera
        case unavailable
    }

    private(set) var refusal: Refusal?
    /// `true` once frames are flowing, so the surface can swap the placeholder
    /// for the preview only when there is something to show.
    private(set) var running = false
    private(set) var torchOn = false
    /// Whether this device has a second lens worth offering.
    private(set) var canFlip = false
    private(set) var hasTorch = false

    /// What a decoded code becomes. Set by the owner; called on the main actor,
    /// exactly once per session.
    var onScan: (String) -> Void = { _ in }

    /// Rule 5: this session draws a fixture frame and never touches the
    /// camera. Decided once, when the scanner is made.
    let fixtureOnly: Bool
    /// How many times this scanner got PAST the gate, towards the capture
    /// hardware (device discovery, permission, the session). Always 0 for a
    /// `fixtureOnly` scanner — which is what its test holds.
    @ObservationIgnored private(set) var hardwareReaches = 0

    /// Made on first use, so a `fixtureOnly` scanner never makes one at all.
    @ObservationIgnored private(set) lazy var session: AVCaptureSession = {
        sessionMade = true
        return AVCaptureSession()
    }()
    /// Whether `session` was ever made — never, for a fixture scanner.
    @ObservationIgnored private(set) var sessionMade = false
    private let queue = DispatchQueue(label: "vela.scan")
    private var input: AVCaptureDeviceInput?
    private var output: AVCaptureVideoDataOutput?
    private var front = false
    /// Rule 3.
    private var reported = false

    /// `fixtureOnly` defaults to the session's own signal: any `VELA_PAGE`
    /// override is a gallery, a board or a dev flow.
    init(fixtureOnly: Bool = PageOverride.isDevSession()) {
        self.fixtureOnly = fixtureOnly
        super.init()
    }

    /// Ask for the camera and start, or record why not.
    ///
    /// Idempotent: the surface's `.task` runs again on every rebuild, and
    /// re-configuring a running session drops frames for a beat.
    func start() async {
        // Rule 5, before anything else: a gallery / board / dev session has
        // no camera to ask for. Not a refusal — the surface draws its
        // fixture frame — and nothing below this line runs.
        guard !fixtureOnly else { return }
        guard !running, refusal == nil else { return }
        hardwareReaches += 1
        // **Is there a camera at all, before asking to use one.** Asking
        // permission for hardware that does not exist is absurd on its face,
        // and on a simulator `requestAccess` simply never answers — a
        // viewfinder that waits forever with nothing on screen and no
        // explanation is the worst of the four refusals.
        guard !Self.cameras().isEmpty else {
            refusal = .noCamera
            return
        }
        switch AVCaptureDevice.authorizationStatus(for: .video) {
        case .authorized:
            break
        case .notDetermined:
            guard await AVCaptureDevice.requestAccess(for: .video) else {
                refusal = .denied
                return
            }
        case .denied:
            refusal = .denied
            return
        case .restricted:
            refusal = .restricted
            return
        @unknown default:
            refusal = .denied
            return
        }
        await configure()
    }

    func stop() {
        running = false
        // Nothing was ever started in a fixture session, and nothing is made
        // now just to be stopped.
        guard !fixtureOnly, sessionMade else { return }
        let session = session
        queue.async { if session.isRunning { session.stopRunning() } }
    }

    /// The torch, which only a back camera has.
    func toggleTorch() {
        guard !fixtureOnly, let device = input?.device, device.hasTorch else { return }
        do {
            try device.lockForConfiguration()
            device.torchMode = device.torchMode == .on ? .off : .on
            torchOn = device.torchMode == .on
            device.unlockForConfiguration()
        } catch {
            print("[vela-wallet] scan: torch \(error)")
        }
    }

    /// The other lens. The torch goes out with it — a front camera has none,
    /// and leaving the flag on would draw a control that does nothing.
    func flip() {
        guard !fixtureOnly, canFlip else { return }
        front.toggle()
        torchOn = false
        Task { await configure() }
    }

    // MARK: - The session

    /// Every wide-angle lens this device has. Empty on a simulator, and on a
    /// Mac whose camera is disabled by policy.
    private static func cameras() -> [AVCaptureDevice] {
        AVCaptureDevice.DiscoverySession(
            deviceTypes: [.builtInWideAngleCamera], mediaType: .video, position: .unspecified
        ).devices
    }

    private func configure() async {
        // Rule 5 again, at the one place a session is configured and
        // started: whatever called this, a fixture session stops here.
        guard !fixtureOnly else { return }
        hardwareReaches += 1
        let position: AVCaptureDevice.Position = front ? .front : .back
        let devices = Self.cameras()
        guard !devices.isEmpty else {
            // Rule 4. The simulator lands here, and so does a Mac with the
            // camera covered by policy.
            refusal = .noCamera
            return
        }
        canFlip = Set(devices.map(\.position)).count > 1
        guard let device = devices.first(where: { $0.position == position }) ?? devices.first,
              let deviceInput = try? AVCaptureDeviceInput(device: device)
        else {
            refusal = .unavailable
            return
        }
        hasTorch = device.hasTorch
        reported = false

        let session = self.session
        let previous = input
        let videoOutput = output ?? AVCaptureVideoDataOutput()
        videoOutput.alwaysDiscardsLateVideoFrames = true
        videoOutput.setSampleBufferDelegate(self, queue: queue)
        input = deviceInput
        output = videoOutput

        await withCheckedContinuation { continuation in
            // Rule 1: everything below blocks.
            queue.async {
                session.beginConfiguration()
                if let previous { session.removeInput(previous) }
                if session.canAddInput(deviceInput) { session.addInput(deviceInput) }
                if !session.outputs.contains(videoOutput), session.canAddOutput(videoOutput) {
                    session.addOutput(videoOutput)
                }
                session.sessionPreset = .high
                // Rule 2.
                if let connection = videoOutput.connection(with: .video) {
                    if connection.isVideoRotationAngleSupported(90) {
                        connection.videoRotationAngle = 90
                    }
                    connection.isVideoMirrored = deviceInput.device.position == .front
                        && connection.isVideoMirroringSupported
                }
                session.commitConfiguration()
                if !session.isRunning { session.startRunning() }
                continuation.resume()
            }
        }
        running = true
    }
}

extension CameraScanner: AVCaptureVideoDataOutputSampleBufferDelegate {
    nonisolated func captureOutput(
        _ output: AVCaptureOutput,
        didOutput sampleBuffer: CMSampleBuffer,
        from connection: AVCaptureConnection
    ) {
        // The frame is already rotated by the connection, so it is upright.
        guard let payload = QrDecoder.decode(sampleBuffer: sampleBuffer, orientation: 1)
        else { return }
        Task { @MainActor [weak self] in
            guard let self, !reported else { return }
            reported = true
            stop()
            onScan(payload)
        }
    }
}

extension CameraScanner.Refusal {
    /// The sentence for this refusal. One mapping for every scanner surface —
    /// 发送's and 探索's say the same thing about the same camera.
    @MainActor
    func text(_ loc: Loc) -> String {
        switch self {
        case .denied, .restricted: loc.t("componentsUi.scanner.permissionText")
        case .noCamera: loc.t("componentsUi.scanner.noCamera")
        case .unavailable: loc.t("componentsUi.scanner.cameraUnavailable")
        }
    }
}
