//
//  ScreenshotViewer.swift
//  VelaWallet
//
//  Tap a screenshot to preview it (spec C, founder 2026-09-27: "上传的截图要
//  能点击放大预览吧"). The phone's viewer is full screen, above the report
//  sheet, on pure black in both themes: the picture aspect-fit BETWEEN the
//  bars — never under them, so the white controls stay legible on a light
//  screenshot — the ✕ at the leading edge (iOS habit), "2 / 5" in the middle
//  when there is more than one, and Remove in the danger colour below.
//
//  It shows the PROCESSED image — the re-encoded JPEG that will be sent —
//  never the picked file: what the person inspects is exactly what goes
//  public (C5). It opens by flying the picture out of its tile and closes by
//  flying it back into the tile of the picture it is on (a fade under
//  reduced motion, or when there is no tile to go back to).
//

import SwiftUI
import UIKit

// MARK: - Which picture

/// Which screenshot the viewer shows, kept by id — so a removal, or a tile
/// finishing its processing while the viewer is up, never swaps the picture
/// under the person's eyes. Pure, so the rules are tested without a screen.
struct ScreenshotViewerState: Equatable {
    /// The screenshots that can be shown, in tile order: the processed ones.
    private(set) var ids: [UUID]
    private(set) var current: UUID

    var index: Int { ids.firstIndex(of: current) ?? 0 }
    var count: Int { ids.count }

    /// Open on `id` — or nothing, when there is nothing to open: the report
    /// is on its way (the form is inert, C5), the tile is still processing
    /// (C1), or it is gone.
    init?(opening id: UUID, shots: [FeedbackSender.Shot], sending: Bool) {
        guard !sending else { return nil }
        let ready = Self.viewable(shots)
        guard ready.contains(id) else { return nil }
        ids = ready
        current = id
    }

    static func viewable(_ shots: [FeedbackSender.Shot]) -> [UUID] {
        shots.filter { $0.prepared != nil }.map(\.id)
    }

    /// The person paged to `id`.
    mutating func show(_ id: UUID) {
        guard ids.contains(id) else { return }
        current = id
    }

    mutating func page(to index: Int) {
        guard ids.indices.contains(index) else { return }
        current = ids[index]
    }

    /// Follow the tray after a change. The picture on screen stays when it is
    /// still there; when it was removed the NEXT one takes its place, or the
    /// previous when it was the last (C2). `false` when nothing is left — the
    /// viewer closes.
    mutating func sync(_ shots: [FeedbackSender.Shot]) -> Bool {
        let fresh = Self.viewable(shots)
        guard !fresh.isEmpty else {
            ids = []
            return false
        }
        if !fresh.contains(current) {
            let old = ids.firstIndex(of: current) ?? 0
            let after = ids.dropFirst(old + 1).first { fresh.contains($0) }
            let before = ids.prefix(old).last { fresh.contains($0) }
            current = after ?? before ?? fresh[min(old, fresh.count - 1)]
        }
        ids = fresh
        return true
    }

    /// Where focus goes when the viewer closes (C4): the tile of the picture
    /// it closed on — the one that opened it, or the one that took its place
    /// after a removal — or the add target when no tile is left.
    enum Return: Equatable {
        case tile(UUID)
        case add
    }

    static func returnFocus(closingOn id: UUID?, shots: [FeedbackSender.Shot]) -> Return {
        if let id, shots.contains(where: { $0.id == id }) { return .tile(id) }
        return .add
    }

    /// A picture of `image` points, aspect-fit and centred in `size` minus
    /// `insets` — the area between the bars. The SwiftUI flight and the
    /// UIKit page both place the picture with this, so the hand-over between
    /// them is seamless.
    static func fitRect(image: CGSize, in size: CGSize, insets: UIEdgeInsets) -> CGRect {
        let area = CGRect(x: insets.left, y: insets.top,
                          width: max(0, size.width - insets.left - insets.right),
                          height: max(0, size.height - insets.top - insets.bottom))
        guard image.width > 0, image.height > 0, area.width > 0, area.height > 0 else {
            return CGRect(origin: CGPoint(x: area.midX, y: area.midY), size: .zero)
        }
        let scale = min(area.width / image.width, area.height / image.height)
        let fitted = CGSize(width: image.width * scale, height: image.height * scale)
        return CGRect(x: area.midX - fitted.width / 2, y: area.midY - fitted.height / 2,
                      width: fitted.width, height: fitted.height)
    }
}

// MARK: - Decoded pictures

/// The processed JPEGs, decoded once and kept for the tiles and the viewer:
/// the tile, the flight and the page then draw the same pixels.
enum ScreenshotImages {
    private static let cache = NSCache<NSUUID, UIImage>()

    @MainActor static func image(for shot: FeedbackSender.Shot) -> UIImage? {
        if let hit = cache.object(forKey: shot.id as NSUUID) { return hit }
        guard let jpeg = shot.prepared?.jpeg, let image = UIImage(data: jpeg) else { return nil }
        let decoded = image.preparingForDisplay() ?? image
        cache.setObject(decoded, forKey: shot.id as NSUUID)
        return decoded
    }

    /// A picture already decoded — the one just removed, as it fades.
    static func cached(_ id: UUID) -> UIImage? {
        cache.object(forKey: id as NSUUID)
    }
}

// MARK: - Where the tiles are

/// Each tile's frame on screen, read from a plain UIView behind it — the
/// sheet and the viewer are different presentations, so only window
/// coordinates are common to both.
///
/// A tile can briefly have TWO probes: when it finishes processing its
/// accessibility changes shape, SwiftUI makes a new probe, and the old one is
/// updated once more before it is taken down. So every live probe is kept
/// and the answer comes from one that is on screen — a registry holding only
/// the last one registered pointed at the dying view, and the viewer faded
/// in instead of flying out of the tile (device run, 2026-09-27).
@MainActor
final class TileProbes {
    private final class Weak {
        weak var view: UIView?
        init(_ view: UIView) { self.view = view }
    }

    private var views: [UUID: [Weak]] = [:]

    func register(_ view: UIView, for id: UUID) {
        var live = (views[id] ?? []).filter { $0.view != nil }
        if !live.contains(where: { $0.view === view }) { live.append(Weak(view)) }
        views[id] = live
    }

    func frameInWindow(_ id: UUID) -> CGRect? {
        guard let view = views[id]?.compactMap(\.view).last(where: { $0.window != nil }) else { return nil }
        return view.convert(view.bounds, to: nil)
    }
}

struct TileFrameProbe: UIViewRepresentable {
    let id: UUID
    let probes: TileProbes

    func makeUIView(context: Context) -> UIView {
        let view = UIView()
        view.isUserInteractionEnabled = false
        view.isAccessibilityElement = false
        view.backgroundColor = .clear
        probes.register(view, for: id)
        return view
    }

    func updateUIView(_ view: UIView, context: Context) {
        probes.register(view, for: id)
    }
}

// MARK: - The viewer

/// The swipe down's progress, 0…1, in its own observable so a finger moving
/// at 120 Hz redraws only the black behind the picture.
@MainActor
@Observable
final class ScreenshotViewerDrag {
    var progress: CGFloat = 0
}

struct ScreenshotViewer: View {
    /// What the sheet opens the viewer with.
    struct Launch: Identifiable {
        let id = UUID()
        let state: ScreenshotViewerState
        /// The opened tile's frame in window coordinates — where the picture
        /// flies from. `nil` fades in.
        let from: CGRect?
    }

    struct Words {
        /// "View screenshot {{index}}" — the picture's accessible name.
        let viewScreenshot: String
        let close: String
        let remove: String

        func name(_ index: Int) -> String {
            viewScreenshot.replacingOccurrences(of: "{{index}}", with: String(index + 1))
        }
    }

    private enum Phase { case opening, open, closing }

    /// The picture while it flies between the tile and the screen.
    private struct Flight {
        var frame: CGRect
        var corner: CGFloat
        var scale: CGFloat = 1
        var opacity: Double = 1
        let image: UIImage
    }

    private typealias G = ScreenshotViewerGeometry

    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let launch: Launch
    let sender: FeedbackSender
    let words: Words
    /// A tile's frame in window coordinates, now — where the picture flies
    /// back to. `nil` fades out.
    let tileFrame: (UUID) -> CGRect?
    /// The picture on screen (its tile hides under it, as in Photos).
    let onCurrent: (UUID) -> Void
    /// Remove this screenshot from the report — the tray's own rule.
    let onRemove: (UUID) -> Void
    /// Gone; the picture it closed on, if it still exists.
    let onClosed: (UUID?) -> Void

    @State private var state: ScreenshotViewerState
    @State private var phase: Phase = .opening
    @State private var started = false
    @State private var flight: Flight?
    @State private var backdrop: Double = 0
    @State private var chrome = false
    @State private var pagerShown = false
    @State private var screen: CGSize = .zero
    /// The full-screen layers' origin in global coordinates. Window and
    /// global coincide in a full-screen cover; this keeps the arithmetic
    /// honest if they ever do not.
    @State private var layerOrigin: CGPoint = .zero
    @State private var topBar: CGRect = .zero
    @State private var bottomBar: CGRect = .zero
    @State private var drag = ScreenshotViewerDrag()
    @State private var handle = ScreenshotPagerHandle()

    init(launch: Launch, sender: FeedbackSender, words: Words,
         tileFrame: @escaping (UUID) -> CGRect?,
         onCurrent: @escaping (UUID) -> Void,
         onRemove: @escaping (UUID) -> Void,
         onClosed: @escaping (UUID?) -> Void) {
        self.launch = launch
        self.sender = sender
        self.words = words
        self.tileFrame = tileFrame
        self.onCurrent = onCurrent
        self.onRemove = onRemove
        self.onClosed = onClosed
        _state = State(initialValue: launch.state)
    }

    var body: some View {
        ZStack {
            Backdrop(base: backdrop, drag: drag)
                .ignoresSafeArea()
            pager
                .ignoresSafeArea()
                .opacity(pagerShown ? 1 : 0)
                .allowsHitTesting(phase == .open)
            flightLayer
                .ignoresSafeArea()
            bars
        }
        .onChange(of: trayKeys) { _, _ in followTray() }
        // The sheet stays in the window behind the black (it shows through
        // a swipe down), so VoiceOver is held here explicitly: focus stays in
        // the viewer until it closes (C4).
        .accessibilityAddTraits(.isModal)
        .accessibilityAction(.escape) { close() }
        // Light status bar on the black; gone while a swipe carries the
        // picture, with the controls, as in Photos.
        .preferredColorScheme(.dark)
        .statusBarHidden(phase == .open && !chrome)
    }

    // MARK: Layers

    /// The pages, over the black.
    private var pager: some View {
        ScreenshotPager(
            items: items,
            current: state.current,
            insets: imageInsets,
            label: { [words] index in words.name(index) },
            reduceMotion: reduceMotion,
            handle: handle,
            onPage: { id in
                state.show(id)
                onCurrent(id)
            },
            onDragBegan: {
                withAnimation(.easeOut(duration: G.chromeFade)) { chrome = false }
            },
            onDragChanged: { progress in drag.progress = progress },
            onDragEnded: { frame in
                if let frame {
                    close(from: frame)
                } else {
                    withAnimation(G.springBack) { drag.progress = 0 }
                    withAnimation(.easeOut(duration: G.chromeFade)) { chrome = true }
                }
            }
        )
    }

    /// The picture in flight, in window coordinates.
    private var flightLayer: some View {
        GeometryReader { proxy in
            if let flight {
                Image(uiImage: flight.image)
                    .resizable()
                    .scaledToFill()
                    .frame(width: flight.frame.width, height: flight.frame.height)
                    .clipShape(RoundedRectangle(cornerRadius: flight.corner, style: .continuous))
                    .scaleEffect(flight.scale)
                    .opacity(flight.opacity)
                    .position(x: flight.frame.midX - layerOrigin.x, y: flight.frame.midY - layerOrigin.y)
            }
            Color.clear
                .onAppear { measured(screen: proxy.size, origin: proxy.frame(in: .global).origin) }
                .onChange(of: proxy.size) { _, size in measured(screen: size, origin: proxy.frame(in: .global).origin) }
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }

    /// The ✕ and the counter above the picture; Remove below it.
    private var bars: some View {
        VStack(spacing: 0) {
            ZStack {
                if state.count > 1 {
                    Text("\(state.index + 1) / \(state.count)")
                        .monospacedDigit()
                        .typeRole(Typography.fieldLabel)
                        .foregroundStyle(G.control.opacity(G.counterOpacity))
                        // The picture says it too, as its value — once is enough.
                        .accessibilityHidden(true)
                }
                HStack {
                    Button(action: { close() }) {
                        LucideIcon(.close, size: G.closeGlyph)
                            .foregroundStyle(G.control)
                            .frame(width: G.closeTarget, height: G.closeTarget)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(ViewerPressStyle())
                    .accessibilityLabel(words.close)
                    .accessibilityIdentifier("feedback.viewer.close")
                    .accessibilityHidden(!chrome)
                    Spacer(minLength: 0)
                }
            }
            .frame(minHeight: G.barHeight)
            .padding(.horizontal, G.barPadding)
            .padding(.vertical, G.barPadding)
            .onGeometryChange(for: CGRect.self) { $0.frame(in: .global) } action: { frame in
                topBar = frame
                startIfReady()
            }
            Spacer(minLength: 0)
            Button(action: remove) {
                Text(words.remove)
                    .typeRole(Typography.button)
                    .foregroundStyle(G.danger)
                    .padding(.horizontal, G.removePadding)
                    .frame(minHeight: G.barHeight)
                    .contentShape(Rectangle())
            }
            .buttonStyle(ViewerPressStyle())
            .accessibilityIdentifier("feedback.viewer.remove")
            // Per control, never on the whole column: a container's
            // `accessibilityHidden(false)` un-hides the counter inside it.
            .accessibilityHidden(!chrome)
            .frame(maxWidth: .infinity)
            .padding(.vertical, G.barPadding)
            .onGeometryChange(for: CGRect.self) { $0.frame(in: .global) } action: { frame in
                bottomBar = frame
                startIfReady()
            }
        }
        .opacity(chrome ? 1 : 0)
        .allowsHitTesting(chrome && phase == .open)
    }

    // MARK: Model

    private var items: [ScreenshotPagerItem] {
        sender.shots.compactMap { shot in
            guard state.ids.contains(shot.id), let image = ScreenshotImages.image(for: shot) else { return nil }
            return ScreenshotPagerItem(id: shot.id, image: image)
        }
    }

    /// What the tray holds, cheaply — ids and whether each is ready — so a
    /// change is noticed without comparing megabytes of JPEG.
    private var trayKeys: [String] {
        sender.shots.map { "\($0.id.uuidString):\($0.prepared != nil)" }
    }

    /// The area between the bars, as insets of the full-screen layers.
    private var imageInsets: UIEdgeInsets {
        guard screen != .zero, topBar != .zero, bottomBar != .zero else { return .zero }
        return UIEdgeInsets(top: topBar.maxY - layerOrigin.y,
                            left: max(0, topBar.minX - layerOrigin.x),
                            bottom: max(0, screen.height - (bottomBar.minY - layerOrigin.y)),
                            right: max(0, screen.width - (topBar.maxX - layerOrigin.x)))
    }

    private func picture(_ id: UUID) -> UIImage? {
        sender.shots.first { $0.id == id }.flatMap(ScreenshotImages.image(for:)) ?? ScreenshotImages.cached(id)
    }

    /// Where the pages put the picture at 1×, in window coordinates.
    private func fitFrame(_ id: UUID) -> CGRect? {
        guard let image = picture(id) else { return nil }
        return ScreenshotViewerState.fitRect(image: image.size, in: screen, insets: imageInsets)
            .offsetBy(dx: layerOrigin.x, dy: layerOrigin.y)
    }

    // MARK: Open

    private func measured(screen size: CGSize, origin: CGPoint) {
        screen = size
        layerOrigin = origin
        startIfReady()
    }

    /// Once the bars are measured the picture's place is known: fly it out
    /// of its tile (or fade in), then hand over to the pages.
    private func startIfReady() {
        guard !started, phase == .opening, screen != .zero, topBar != .zero, bottomBar != .zero else { return }
        started = true
        let current = state.current
        onCurrent(current)
        guard !reduceMotion, let from = launch.from, let image = picture(current), let fit = fitFrame(current) else {
            withAnimation(.easeOut(duration: G.fadeDuration), completionCriteria: .removed) {
                backdrop = 1
                pagerShown = true
                chrome = true
            } completion: {
                opened()
            }
            return
        }
        flight = Flight(frame: from, corner: G.tileCorner, image: image)
        // One frame at the tile, then away: the flight must be drawn where the
        // tile is before it moves, or it would start from nowhere.
        DispatchQueue.main.async {
            withAnimation(G.flight, completionCriteria: .removed) {
                flight?.frame = fit
                flight?.corner = 0
                backdrop = 1
            } completion: {
                pagerShown = true
                flight = nil
                withAnimation(.easeOut(duration: G.chromeFade)) { chrome = true }
                opened()
            }
        }
    }

    private func opened() {
        phase = .open
        // VoiceOver goes into the viewer and says which picture (C4).
        DispatchQueue.main.asyncAfter(deadline: .now() + FeedbackGeometry.focusDelay) {
            UIAccessibility.post(notification: .screenChanged, argument: handle.view)
        }
    }

    // MARK: Remove

    private func remove() {
        guard phase == .open else { return }
        onRemove(state.current)
    }

    /// The tray changed under the viewer (a removal here, or a tile that
    /// finished processing): follow it, and close when nothing is left.
    private func followTray() {
        guard phase != .closing else { return }
        let before = state.current
        if state.sync(sender.shots) {
            if state.current != before { onCurrent(state.current) }
        } else {
            closeEmptied(last: before)
        }
    }

    /// The only picture was removed: nothing to fly back to — it fades.
    private func closeEmptied(last: UUID) {
        phase = .closing
        let frame = handle.currentImageFrame()
        let image = ScreenshotImages.cached(last)
        withAnimation(.easeOut(duration: G.chromeFade)) { chrome = false }
        fadeOut(from: frame, image: image, closingOn: nil)
    }

    // MARK: Close

    /// ✕, a swipe down released past the line, or VoiceOver's escape.
    private func close(from dragged: CGRect? = nil) {
        guard phase == .open else { return }
        phase = .closing
        let current = state.current
        withAnimation(.easeOut(duration: G.chromeFade)) { chrome = false }
        // Zoomed, the picture is cut off at the bars: the pages fade where
        // they are rather than the picture jumping to its full size to fly.
        if dragged == nil, handle.isZoomedIn {
            fadeOut(from: nil, image: nil, closingOn: current)
            return
        }
        let from = dragged ?? handle.currentImageFrame() ?? fitFrame(current)
        let target = tileFrame(current)
        guard !reduceMotion, let from, let target, let image = picture(current) else {
            fadeOut(from: from, image: picture(current), closingOn: current)
            return
        }
        // The swipe's fade carries on into the flight.
        var still = Transaction()
        still.disablesAnimations = true
        withTransaction(still) {
            backdrop *= Double(1 - drag.progress)
            drag.progress = 0
            flight = Flight(frame: from, corner: 0, image: image)
            pagerShown = false
        }
        DispatchQueue.main.async {
            withAnimation(G.flight, completionCriteria: .removed) {
                flight?.frame = target
                flight?.corner = G.tileCorner
                backdrop = 0
            } completion: {
                onClosed(current)
            }
        }
    }

    /// No tile to land on, or reduced motion: the picture fades (and, with
    /// motion, shrinks a little) where it is.
    private func fadeOut(from frame: CGRect?, image: UIImage?, closingOn id: UUID?) {
        var still = Transaction()
        still.disablesAnimations = true
        withTransaction(still) {
            backdrop *= Double(1 - drag.progress)
            drag.progress = 0
            if !reduceMotion, let frame, let image {
                flight = Flight(frame: frame, corner: 0, image: image)
                pagerShown = false
            }
        }
        DispatchQueue.main.async {
            withAnimation(.easeOut(duration: G.fadeDuration), completionCriteria: .removed) {
                backdrop = 0
                pagerShown = false
                flight?.opacity = 0
                flight?.scale = G.fadeScale
            } completion: {
                onClosed(id)
            }
        }
    }
}

/// The black behind the picture: the viewer's own fade times the swipe's.
private struct Backdrop: View {
    let base: Double
    let drag: ScreenshotViewerDrag

    var body: some View {
        ScreenshotViewerGeometry.backdrop
            .opacity(base * Double(1 - drag.progress))
    }
}

/// The viewer's controls answer the finger like every other button: they
/// dim, shrink a little and tick (the founder's rule).
private struct ViewerPressStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .opacity(configuration.isPressed ? Interaction.pressedOpacity : 1)
            .scaleEffect(configuration.isPressed ? Interaction.pressScaleButton : 1)
            .animation(Interaction.pressSpring, value: configuration.isPressed)
            .onChange(of: configuration.isPressed) { _, pressed in
                if pressed { VelaHaptic.press.play() }
            }
    }
}
