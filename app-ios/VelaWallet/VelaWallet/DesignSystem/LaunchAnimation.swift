//
//  LaunchAnimation.swift
//  VelaWallet
//
//  Launch-animation constants and geometry (spec 012).
//
//  Every value here is shared verbatim with the Android, desktop and web apps —
//  see specs/012-launch-animation-lottie/data-model.md §4. Repeating them in four
//  languages is deliberate: `LaunchGeometryTests` asserts research D1's table, so
//  a transcription slip surfaces there rather than on a user's screen. Nothing
//  else would catch it — the four apps never run the same code.
//

import CoreGraphics
import Foundation
import VelaCore

enum LaunchAnimation {
    enum Appearance: String {
        case dark, light
    }

    /// Which authored composition applies. Chosen by the predicate below, not by
    /// platform: an iPad in landscape gets the large-screen composition with no
    /// special-casing, which is the point of sharing the rule.
    enum FormFactor: String {
        case phone
        case largeScreen = "desktop"
    }

    // MARK: - Timing

    /// Authored length of the animation: 102 frames ÷ 60 fps.
    static let durationMs: Int = 1700

    /// Hold on the finished lockup before the hand-off, so the brand registers
    /// instead of flashing past (FR-012a). Skippable by input; bypassed under
    /// reduce-motion. Tried at 2000 ms and cut to 400 on seeing it run.
    static let holdMs: Int = 400

    /// Cross-dissolve into Welcome — `motion.durationSlow`. 180 ms
    /// (`motion.sheetOut`) was the first choice and reads as a cut at
    /// full-screen scale rather than a dissolve.
    static let exitCrossfadeMs: Int = 400

    /// FR-014: nothing presented by now → abandon the animation, show Welcome.
    static let firstFrameBudgetMs: Int = 400

    /// FR-015: measured from the first presented frame, not from construction.
    /// Nominal is 1700 play + 400 hold + 400 dissolve = 2500; the rest is slack.
    static let hardCeilingMs: Int = 3000

    static var duration: TimeInterval { Double(durationMs) / 1000 }
    static var hold: TimeInterval { Double(holdMs) / 1000 }
    static var exitCrossfade: TimeInterval { Double(exitCrossfadeMs) / 1000 }
    static var firstFrameBudget: TimeInterval { Double(firstFrameBudgetMs) / 1000 }
    static var hardCeiling: TimeInterval { Double(hardCeilingMs) / 1000 }

    /// Deterministic disable for tests (FR-029).
    ///
    /// Existing UI tests must not sit through the animation, and a `sleep` long
    /// enough to outlast it is exactly the flaky waiting this replaces. Set
    /// `VELA_SKIP_LAUNCH_ANIMATION=1` in the test runner's environment.
    static var isDisabled: Bool {
        ProcessInfo.processInfo.environment["VELA_SKIP_LAUNCH_ANIMATION"] == "1"
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
    static func claim(store: VelaStore, now: Date = Date(), disabled: Bool = isDisabled) -> Bool {
        let key = launchAnimationStorageKey()
        let nowMs = (now.timeIntervalSince1970 * 1000).rounded(.down)
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

    /// Core canvases — the cropped framings that ship (research D0). The
    /// full-bleed pair exists only to pin `boxWidthRatio` and is never loaded.
    static let phoneCanvas = CGSize(width: 350, height: 120)
    static let largeCanvas = CGSize(width: 680, height: 220)

    /// Form-factor threshold, in points. Deliberately not a layout breakpoint
    /// borrowed from elsewhere: this governs which animation was authored for
    /// the screen, not how a screen lays out.
    static let largeScreenMinWidth: CGFloat = 768

    /// Box width as a fraction of viewport width: the core canvas divided by the
    /// full-bleed canvas it was cropped from. NOT a judgement call — at 390 pt
    /// the phone lockup lands at exactly the authored 80.7 % of screen width,
    /// and `scripts/lint-lottie-assets.mjs` fails if a re-crop moves either.
    static let phoneBoxWidthRatio: CGFloat = 350.0 / 390.0
    static let largeBoxWidthRatio: CGFloat = 680.0 / 1920.0

    /// The shared predicate.
    static func formFactor(for size: CGSize) -> FormFactor {
        (size.width >= size.height || size.width >= largeScreenMinWidth) ? .largeScreen : .phone
    }

    /// Box size for a viewport, per the shared fit rule. Centred by the caller;
    /// nothing is clipped or clamped, because the shipped asset is cropped to
    /// the motion — the box *is* the artwork.
    static func boxSize(viewportWidth: CGFloat, formFactor: FormFactor) -> CGSize {
        let ratio = formFactor == .largeScreen ? largeBoxWidthRatio : phoneBoxWidthRatio
        let canvas = formFactor == .largeScreen ? largeCanvas : phoneCanvas
        let width = viewportWidth * ratio
        return CGSize(width: width, height: width * canvas.height / canvas.width)
    }

    /// Bundled resource name, copied in by the `Bundle launch animations` build
    /// phase — no extension, which is what `LottieAnimation.named` wants.
    static func assetName(_ formFactor: FormFactor, _ appearance: Appearance) -> String {
        "vela-wallet-launch-\(formFactor.rawValue)-core-\(appearance.rawValue)"
    }
}
