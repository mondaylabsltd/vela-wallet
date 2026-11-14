//
//  LaunchAnimation.swift
//  VelaWallet
//
//  Launch-animation constants, geometry and timeline (spec 012).
//
//  The scene itself — the mark's paths, the wordmark's letters, their colours
//  and every tween's timing — is `LaunchScene`, generated from
//  docs/design/onboarding/launch/launch-scene.json for all four apps. What is
//  here is what the scene does not say: how long to hold, how to hand off, how
//  big the box is, and the timeline evaluated at a given millisecond.
//
//  The hand-off numbers are shared verbatim with the Android, desktop and web
//  apps — see specs/012-launch-animation-lottie/data-model.md §4.
//  `LaunchGeometryTests` asserts them, and `LaunchTimelineTests` holds the
//  timeline to the scene's own reference vectors, so a slip surfaces there
//  rather than on a user's screen.
//

import CoreGraphics
import Foundation
import SwiftUI
import VelaCore

enum LaunchAnimation {
    enum Appearance: String {
        case dark, light
    }

    /// Which framing of the scene applies. Chosen by the predicate below, not by
    /// platform: an iPad in landscape gets the large-screen framing with no
    /// special-casing, which is the point of sharing the rule.
    enum FormFactor: String {
        case phone
        case largeScreen = "desktop"
    }

    // MARK: - Timing

    /// Authored length of the animation (the scene's 102 frames at 60 fps). The
    /// last tween ends at `LaunchScene.settledMs`; the rest is the finished
    /// lockup, still.
    static let durationMs = Int(LaunchScene.durationMs)

    /// Hold on the finished lockup before the hand-off, so the brand registers
    /// instead of flashing past (FR-012a). Skippable by input; bypassed under
    /// reduce-motion. Tried at 2000 ms and cut to 400 on seeing it run.
    static let holdMs: Int = 400

    /// Cross-dissolve into Welcome — `motion.durationSlow`. 180 ms
    /// (`motion.sheetOut`) was the first choice and reads as a cut at
    /// full-screen scale rather than a dissolve.
    static let exitCrossfadeMs: Int = 400

    /// FR-015: measured from the moment the overlay appears.
    /// Nominal is 1700 play + 400 hold + 400 dissolve = 2500; the rest is slack.
    static let hardCeilingMs: Int = 3000

    static var duration: TimeInterval { Double(durationMs) / 1000 }
    static var hold: TimeInterval { Double(holdMs) / 1000 }
    static var exitCrossfade: TimeInterval { Double(exitCrossfadeMs) / 1000 }
    static var hardCeiling: TimeInterval { Double(hardCeilingMs) / 1000 }

    /// Deterministic disable for tests (FR-029).
    ///
    /// Existing UI tests must not sit through the animation, and a `sleep` long
    /// enough to outlast it is exactly the flaky waiting this replaces. Set
    /// `VELA_SKIP_LAUNCH_ANIMATION=1` in the test runner's environment.
    static var isDisabled: Bool {
        ProcessInfo.processInfo.environment["VELA_SKIP_LAUNCH_ANIMATION"] == "1"
    }

    /// Debug builds only: play it on this cold start whatever the weekly rule
    /// says, to look at it on a device without erasing the app.
    /// `VELA_FORCE_LAUNCH_ANIMATION=1` in the launch environment (e.g.
    /// `xcrun devicectl device process launch --environment-variables
    /// '{"VELA_FORCE_LAUNCH_ANIMATION":"1"}' …`). A Release binary never reads it.
    static var isForced: Bool {
        #if DEBUG
        ProcessInfo.processInfo.environment["VELA_FORCE_LAUNCH_ANIMATION"] == "1"
        #else
        false
        #endif
    }

    // MARK: - Replay rule

    /// Does this cold start play the lockup? The core's rule
    /// (`vela_core::launch`, the web's and the desktop's too): on a first
    /// start, then not again for a week. It played on every cold start here
    /// before — 2.5 s of somebody's morning, every morning.
    ///
    /// When it plays, the time is written under the shared key at once (epoch
    /// milliseconds as decimal text, as the web keeps it), so a start that is
    /// killed mid-animation still counts as having seen it.
    @MainActor
    static func claim(
        store: VelaStore, now: Date = Date(), disabled: Bool = isDisabled, forced: Bool = isForced
    ) -> Bool {
        let key = launchAnimationStorageKey()
        let nowMs = (now.timeIntervalSince1970 * 1000).rounded(.down)
        if forced && !disabled {
            store.writeString(key, String(Int64(nowMs)))
            return true
        }
        // Text that does not read as a number is "never played" (the core
        // treats a non-finite one the same): a bad write must not silence it
        // for good.
        let stored = store.readString(key).flatMap(Double.init)
        guard launchAnimationDue(storedLastPlayedMs: stored, nowMs: nowMs, disabled: disabled) else {
            return false
        }
        store.writeString(key, String(Int64(nowMs)))
        return true
    }

    // MARK: - Geometry

    /// The scene's canvases — the cropped framings (research D0). The large one
    /// draws the same scene as the phone's, scaled and offset
    /// (`LaunchScene.largeScale`, `largeOffset`).
    static let phoneCanvas = LaunchScene.canvas
    static let largeCanvas = LaunchScene.largeCanvas

    /// Form-factor threshold, in points. Deliberately not a layout breakpoint
    /// borrowed from elsewhere: this governs which framing was authored for
    /// the screen, not how a screen lays out.
    static let largeScreenMinWidth: CGFloat = 768

    /// Box width as a fraction of viewport width: the canvas divided by the
    /// full-bleed canvas it was cropped from. NOT a judgement call — at 390 pt
    /// the phone lockup lands at exactly the authored 80.7 % of screen width.
    static let phoneBoxWidthRatio: CGFloat = LaunchScene.canvas.width / LaunchScene.phoneFullBleed.width
    static let largeBoxWidthRatio: CGFloat = LaunchScene.largeCanvas.width / LaunchScene.largeFullBleed.width

    /// The shared predicate.
    static func formFactor(for size: CGSize) -> FormFactor {
        (size.width >= size.height || size.width >= largeScreenMinWidth) ? .largeScreen : .phone
    }

    /// The canvas a form factor draws on — the box's aspect ratio.
    static func canvas(_ formFactor: FormFactor) -> CGSize {
        formFactor == .largeScreen ? largeCanvas : phoneCanvas
    }

    /// Box size for a viewport, per the shared fit rule. Centred by the caller;
    /// nothing is clipped or clamped, because the canvas is cropped to the
    /// motion — the box *is* the artwork.
    static func boxSize(viewportWidth: CGFloat, formFactor: FormFactor) -> CGSize {
        let ratio = formFactor == .largeScreen ? largeBoxWidthRatio : phoneBoxWidthRatio
        let canvas = canvas(formFactor)
        let width = viewportWidth * ratio
        return CGSize(width: width, height: width * canvas.height / canvas.width)
    }

    // MARK: - Timeline

    /// Where everything is at one instant: the mark's moving point and each
    /// letter's opacity, in `LaunchScene.letters` order. Phone-canvas
    /// coordinates; the large framing is applied when drawing.
    struct Frame: Equatable {
        var mark: CGPoint
        var letters: [Double]
    }

    /// The scene's easing — CSS `cubic-bezier(x1, y1, x2, y2)` — as SwiftUI's
    /// own curve, which solves for x the way CSS does.
    static let easing: UnitCurve = {
        let (x1, y1, x2, y2) = LaunchScene.easing
        return .bezier(
            startControlPoint: UnitPoint(x: x1, y: y1),
            endControlPoint: UnitPoint(x: x2, y: y2)
        )
    }()

    /// Eased progress of a tween running from `start` to `end`, at `ms`. Held
    /// at 0 before and 1 after, as a CSS fill-mode `both` would.
    static func progress(at ms: Double, from start: Double, to end: Double) -> Double {
        guard end > start else { return ms >= end ? 1 : 0 }
        let linear = min(max((ms - start) / (end - start), 0), 1)
        return easing.value(at: linear)
    }

    /// The timeline, as a pure function of time — what is drawn at `ms`.
    /// Anything outside 0…`durationMs` is the nearest end.
    static func frame(at ms: Double) -> Frame {
        let markT = progress(at: ms, from: LaunchScene.markStartMs, to: LaunchScene.markEndMs)
        let from = LaunchScene.markFrom, to = LaunchScene.markTo
        let mark = CGPoint(
            x: from.x + (to.x - from.x) * markT,
            y: from.y + (to.y - from.y) * markT
        )
        let letters = LaunchScene.letters.map { progress(at: ms, from: $0.startMs, to: $0.endMs) }
        return Frame(mark: mark, letters: letters)
    }

    /// The finished lockup — what reduce-motion shows, and where every play ends.
    static var finished: Frame { frame(at: LaunchScene.durationMs) }

    /// The scene's colours for an appearance.
    static func colors(_ appearance: Appearance) -> LaunchScene.Colors {
        appearance == .dark ? LaunchScene.dark : LaunchScene.light
    }
}
