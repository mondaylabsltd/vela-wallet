//
//  LaunchAnimationView.swift
//  VelaWallet
//
//  The launch animation, drawn by SwiftUI itself: the scene (`LaunchScene`,
//  generated from docs/design/onboarding/launch/launch-scene.json) is filled
//  into a `Canvas`, and a `TimelineView` moves it by evaluating
//  `LaunchAnimation.frame(at:)` once per display frame. No animation runtime,
//  nothing loaded — the lockup exists the moment this view does.
//  Contract: specs/012-launch-animation-lottie/contracts/launch-animation-api.md
//
//  Same shape as the other three platforms: play once → hold the finished
//  lockup → cross-dissolve into Welcome, with any input cutting straight to the
//  dissolve and every outcome ending silently on Welcome.
//

import SwiftUI

struct LaunchAnimationView: View {
    let appearance: LaunchAnimation.Appearance
    let formFactor: LaunchAnimation.FormFactor
    let reduceMotion: Bool
    /// Fired once, the instant the cross-dissolve starts. The HOST animates its
    /// own Welcome opacity 0→1 with the same curve and duration, so the two
    /// layers dissolve into each other.
    ///
    /// A start SIGNAL rather than streamed progress, because SwiftUI cannot
    /// stream it: `withAnimation { dissolve = 1 }` sets the state to its final
    /// value immediately and only interpolates when rendering, so `onChange`
    /// fires exactly once, with 1.0. Driving the host from that made Welcome
    /// snap to full opacity and the overlay vanish in the same instant — the
    /// abrupt hand-off this parameter now exists to prevent. (Compose's
    /// `animateFloatAsState` DOES tick per frame, which is why Android streams.)
    let onDissolveStart: () -> Void
    /// Called EXACTLY once, for every outcome — completion, skip, ceiling,
    /// reduce-motion. The host does not distinguish between them; it only
    /// removes the overlay.
    let onFinished: () -> Void

    @Environment(\.theme) private var theme

    /// When playback started: the first frame this overlay presented. `nil`
    /// until then, which draws the opening frame.
    @State private var start: Date?
    /// The timeline has reached its end; the clock can stop ticking.
    @State private var played = false
    @State private var dissolve: Double = 0
    @State private var exiting = false
    @State private var finished = false

    var body: some View {
        GeometryReader { proxy in
            let box = LaunchAnimation.boxSize(
                viewportWidth: proxy.size.width,
                formFactor: formFactor
            )

            ZStack {
                // Opaque until the dissolve starts — Welcome must not be visible
                // through it (FR-013). During the dissolve the host's
                // identically coloured background takes over, which is what
                // keeps the backdrop continuous instead of washing out.
                theme.bgBase.ignoresSafeArea()

                if reduceMotion {
                    // FR-019: the finished lockup as a still, not a build. No
                    // clock at all, so there is genuinely no motion to perceive.
                    LaunchSceneCanvas(appearance: appearance, formFactor: formFactor, ms: LaunchScene.durationMs)
                        .frame(width: box.width, height: box.height)
                } else {
                    // `.animation` ticks with the display; paused once the
                    // timeline is over, so the held lockup costs no frames.
                    TimelineView(.animation(minimumInterval: nil, paused: played)) { timeline in
                        LaunchSceneCanvas(appearance: appearance, formFactor: formFactor, ms: elapsedMs(at: timeline.date))
                    }
                    .frame(width: box.width, height: box.height)
                }
            }
            .frame(width: proxy.size.width, height: proxy.size.height)
        }
        .ignoresSafeArea()
        .opacity(1 - dissolve)
        // FR-021: decoration. Hidden from assistive technology; takes no focus.
        .accessibilityHidden(true)
        // FR-016: any tap cuts to the dissolve.
        .contentShape(Rectangle())
        .onTapGesture { beginExit() }
        .task { await run() }
    }

    private func elapsedMs(at date: Date) -> Double {
        guard let start else { return 0 }
        return min(max(date.timeIntervalSince(start) * 1000, 0), LaunchScene.durationMs)
    }

    // MARK: - Lifecycle

    private func run() async {
        if reduceMotion {
            // Static final frame, no hold — the point of the setting is less
            // time spent on motion, not a longer still.
            beginExit()
            return
        }

        start = Date()

        // Play once, then hold the finished lockup (FR-012a) — on its own task,
        // so the ceiling below runs alongside it rather than after it.
        Task {
            try? await Task.sleep(for: .seconds(LaunchAnimation.duration))
            played = true
            try? await Task.sleep(for: .seconds(LaunchAnimation.hold))
            beginExit()
        }

        // FR-015: hard ceiling, from the moment playback started.
        try? await Task.sleep(for: .seconds(LaunchAnimation.hardCeiling))
        beginExit()
    }

    private func beginExit() {
        guard !exiting, !finished else { return }
        exiting = true

        // Both sides start fading in the same instant, with the same curve.
        onDissolveStart()
        withAnimation(.easeInOut(duration: LaunchAnimation.exitCrossfade)) {
            dissolve = 1
        }

        // The overlay is removed only AFTER the fade has had time to render.
        // Removing it on the state change would delete the very thing that is
        // supposed to be fading.
        Task {
            try? await Task.sleep(for: .seconds(LaunchAnimation.exitCrossfade))
            finishOnce()
        }
    }

    /// Latched: a completion racing a timeout must not dismiss twice
    /// (contract behaviour 1).
    private func finishOnce() {
        guard !finished else { return }
        finished = true
        onFinished()
    }
}

/// One frame of the launch scene, filled to whatever size it is given: the
/// form factor's canvas, scaled uniformly to fit. Pure drawing — what is drawn
/// is `LaunchAnimation.frame(at: ms)`, so a still (reduce-motion, a test, a
/// snapshot) and a playing frame are the same code.
struct LaunchSceneCanvas: View {
    let appearance: LaunchAnimation.Appearance
    let formFactor: LaunchAnimation.FormFactor
    let ms: Double

    var body: some View {
        let frame = LaunchAnimation.frame(at: ms)
        let colors = LaunchAnimation.colors(appearance)
        Canvas { context, size in
            let canvas = LaunchAnimation.canvas(formFactor)
            let fit = min(size.width / canvas.width, size.height / canvas.height)
            context.scaleBy(x: fit, y: fit)
            if formFactor == .largeScreen {
                // The large canvas is the same scene, framed: translate, then scale.
                context.translateBy(x: LaunchScene.largeOffset.x, y: LaunchScene.largeOffset.y)
                context.scaleBy(x: LaunchScene.largeScale, y: LaunchScene.largeScale)
            }

            // Paint order is the scene's (and the web's SVG): the mark, then
            // the letters over it.
            var mark = context
            mark.translateBy(x: frame.mark.x, y: frame.mark.y)
            for (role, path) in LaunchScenePaths.mark {
                mark.fill(path, with: .color(Color(rgb: colors[role])))
            }

            // Default `FillStyle` is NONZERO, which the scene requires: the
            // crossbars of "e" and "t" are separate contours wound the same way
            // as the body, and even-odd would punch them out.
            let wordmark = Color(rgb: colors.wordmark)
            for (index, path) in LaunchScenePaths.letters.enumerated() {
                let opacity = frame.letters[index]
                guard opacity > 0 else { continue }
                context.fill(path, with: .color(wordmark.opacity(opacity)))
            }
        }
    }
}

/// The scene's ops turned into `Path`s, once.
enum LaunchScenePaths {
    static let mark: [(LaunchScene.ColorRole, Path)] = LaunchScene.markPaths.map { ($0.fill, path($0.ops)) }
    static let letters: [Path] = LaunchScene.letters.map { path($0.ops) }

    static func path(_ ops: [LaunchScene.Op]) -> Path {
        var path = Path()
        for op in ops {
            switch op {
            case let .move(x, y): path.move(to: CGPoint(x: x, y: y))
            case let .line(x, y): path.addLine(to: CGPoint(x: x, y: y))
            case let .curve(c1x, c1y, c2x, c2y, x, y):
                path.addCurve(
                    to: CGPoint(x: x, y: y),
                    control1: CGPoint(x: c1x, y: c1y),
                    control2: CGPoint(x: c2x, y: c2y)
                )
            case .close: path.closeSubpath()
            }
        }
        return path
    }
}

private extension Color {
    /// 0xRRGGBB, sRGB — the scene's colour format.
    init(rgb: UInt32) {
        self.init(
            .sRGB,
            red: Double((rgb >> 16) & 0xFF) / 255,
            green: Double((rgb >> 8) & 0xFF) / 255,
            blue: Double(rgb & 0xFF) / 255
        )
    }
}
