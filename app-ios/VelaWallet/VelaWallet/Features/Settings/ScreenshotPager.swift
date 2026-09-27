//
//  ScreenshotPager.swift
//  VelaWallet
//
//  The screenshot viewer's pages (spec C2), in UIKit on purpose: what they
//  must do together is exactly what UIScrollView already does, and does the
//  way Photos does —
//
//  - swipe left/right pages, snapping, with black between the pictures;
//  - pinch zooms about the fingers, a double tap goes to 2× at the tap and
//    back, and while zoomed a drag pans (and bounces at the edges);
//  - swipe DOWN, when not zoomed, drags the picture away with the black
//    fading behind it. One recogniser owns that, and the paging and panning
//    wait for it to fail, so a vertical drag closes and a horizontal one
//    pages — never both, and never a wobble of one inside the other.
//
//  The SwiftUI viewer owns everything around the pages: the black, the bars,
//  the flight to and from the tile. This view reports what the fingers did.
//

import SwiftUI
import UIKit

/// One picture in the viewer: the PROCESSED screenshot, what will be sent.
struct ScreenshotPagerItem {
    let id: UUID
    let image: UIImage
}

/// Where the viewer can ask the live pager about itself (the picture's frame
/// on screen when it closes) and hand VoiceOver its element.
@MainActor
final class ScreenshotPagerHandle {
    weak var view: ScreenshotPagerView?

    /// The current picture's frame in window coordinates, as the eye sees it
    /// now — zoomed, panned or dragged.
    func currentImageFrame() -> CGRect? { view?.currentImageFrameInWindow() }

    /// Zoomed, the picture is cut off at the bars; it cannot fly to its
    /// tile without jumping out to its full size first, so it fades.
    var isZoomedIn: Bool { view?.isZoomedIn ?? false }
}

final class ScreenshotPagerView: UIView, UIScrollViewDelegate {

    // MARK: - What it reports

    /// The picture on screen changed (a swipe, or VoiceOver's adjust).
    var onPage: ((UUID) -> Void)?
    /// A swipe down began: the controls go, as Photos hides them.
    var onDragBegan: (() -> Void)?
    /// How far the swipe down has gone, 0…1 — the black fades by it.
    var onDragChanged: ((CGFloat) -> Void)?
    /// Let go: the picture's frame in window coordinates when it should
    /// close, `nil` when it springs back.
    var onDragEnded: ((CGRect?) -> Void)?
    /// The accessible name of the picture at a 0-based index.
    var label: (Int) -> String = { _ in "" }
    var reduceMotion = false

    // MARK: - State

    private let pager = UIScrollView()
    private var pages: [ScreenshotZoomPage] = []
    private(set) var items: [ScreenshotPagerItem] = []
    private(set) var currentIndex = 0
    private let dismissPan = UIPanGestureRecognizer()
    private let dismissGate = DismissGate()
    private var insets: UIEdgeInsets = .zero
    private var laidOutSize: CGSize = .zero
    /// Set while this view moves the pager itself, so the move is not taken
    /// for the person paging.
    private var placing = false

    var currentPage: ScreenshotZoomPage? { pages.indices.contains(currentIndex) ? pages[currentIndex] : nil }

    override init(frame: CGRect) {
        super.init(frame: frame)
        backgroundColor = .clear
        pager.isPagingEnabled = true
        pager.showsHorizontalScrollIndicator = false
        pager.showsVerticalScrollIndicator = false
        pager.contentInsetAdjustmentBehavior = .never
        pager.alwaysBounceHorizontal = true
        pager.alwaysBounceVertical = false
        pager.scrollsToTop = false
        pager.delegate = self
        addSubview(pager)

        dismissPan.addTarget(self, action: #selector(dismissDrag(_:)))
        dismissPan.maximumNumberOfTouches = 1
        dismissGate.shouldBegin = { [weak self] in self?.dismissMayBegin() ?? false }
        dismissPan.delegate = dismissGate
        addGestureRecognizer(dismissPan)
        // A horizontal drag pages only once the swipe down has said no.
        pager.panGestureRecognizer.require(toFail: dismissPan)

        // One element for VoiceOver: the picture on screen, adjustable to
        // the next and previous (the pages themselves are not elements).
        isAccessibilityElement = true
        accessibilityIdentifier = "feedback.viewer.image"
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

    // MARK: - Layout

    override func layoutSubviews() {
        super.layoutSubviews()
        // Wider than the screen by the gap, so the gap is black between pages
        // and never inside one.
        pager.frame = bounds.insetBy(dx: -ScreenshotViewerGeometry.pageGap / 2, dy: 0)
        guard bounds.size != laidOutSize, bounds.width > 0 else { return }
        laidOutSize = bounds.size
        layoutPages()
    }

    /// Each page IS the area between the bars, so a zoomed picture is cut
    /// off at the bars rather than running under them — white controls on
    /// a light screenshot vanished (device run, 2026-09-27).
    private func layoutPages() {
        let width = pager.bounds.width
        let area = CGSize(width: max(0, bounds.width - insets.left - insets.right),
                          height: max(0, bounds.height - insets.top - insets.bottom))
        for (index, page) in pages.enumerated() {
            page.transform = .identity
            page.frame = CGRect(x: CGFloat(index) * width + ScreenshotViewerGeometry.pageGap / 2 + insets.left,
                                y: insets.top, width: area.width, height: area.height)
            page.fit()
        }
        pager.contentSize = CGSize(width: width * CGFloat(pages.count), height: pager.bounds.height)
        place(at: currentIndex)
    }

    private func place(at index: Int) {
        placing = true
        pager.contentOffset = CGPoint(x: CGFloat(index) * pager.bounds.width, y: 0)
        placing = false
    }

    /// The area a picture fits into at 1× — between the bars (C2).
    func setInsets(_ new: UIEdgeInsets) {
        guard new != insets else { return }
        insets = new
        laidOutSize = .zero
        setNeedsLayout()
    }

    var isZoomedIn: Bool { currentPage?.isZoomedIn ?? false }

    // MARK: - The pictures

    /// Follow the tray. The same pictures in the same order only move to
    /// `current`; a different set rebuilds the pages — keeping the ones that
    /// stayed, zoom and all — and when the picture on screen was the one that
    /// went, it fades away as the next comes up. An empty list is ignored:
    /// the viewer is closing, and the last picture stays until it has gone.
    func update(items new: [ScreenshotPagerItem], current: UUID) {
        guard !new.isEmpty else { return }
        let oldIds = items.map(\.id)
        let newIds = new.map(\.id)
        let target = newIds.firstIndex(of: current) ?? min(currentIndex, new.count - 1)
        if oldIds == newIds {
            if target != currentIndex {
                currentIndex = target
                place(at: target)
                refreshAccessibility()
            }
            return
        }
        let outgoing = oldIds.indices.contains(currentIndex) ? oldIds[currentIndex] : nil
        let replaced = outgoing.map { !newIds.contains($0) } ?? false
        var snapshot: UIView?
        if replaced, let page = currentPage, let copy = page.snapshotView(afterScreenUpdates: false) {
            copy.frame = page.convert(page.bounds, to: self)
            copy.isUserInteractionEnabled = false
            addSubview(copy)
            snapshot = copy
        }

        var kept: [UUID: ScreenshotZoomPage] = [:]
        for page in pages { kept[page.id] = page }
        pages = new.map { item in
            if let page = kept.removeValue(forKey: item.id) { return page }
            let page = ScreenshotZoomPage(id: item.id, image: item.image)
            page.panGestureRecognizer.require(toFail: dismissPan)
            pager.addSubview(page)
            return page
        }
        for page in kept.values { page.removeFromSuperview() }
        items = new
        currentIndex = target
        laidOutSize = .zero
        setNeedsLayout()
        layoutIfNeeded()
        refreshAccessibility()

        if let snapshot, let incoming = currentPage {
            incoming.alpha = 0
            if !reduceMotion {
                let scale = ScreenshotViewerGeometry.removeIncomingScale
                incoming.transform = CGAffineTransform(scaleX: scale, y: scale)
            }
            UIView.animate(withDuration: ScreenshotViewerGeometry.removeDuration, delay: 0, options: [.curveEaseOut]) {
                snapshot.alpha = 0
                if !self.reduceMotion {
                    let scale = ScreenshotViewerGeometry.removeOutgoingScale
                    snapshot.transform = CGAffineTransform(scaleX: scale, y: scale)
                }
                incoming.alpha = 1
                incoming.transform = .identity
            } completion: { _ in
                snapshot.removeFromSuperview()
            }
            // VoiceOver reads the picture that took its place.
            UIAccessibility.post(notification: .layoutChanged, argument: self)
        }
    }

    /// Page programmatically (VoiceOver), animated like a swipe.
    func show(index: Int) {
        guard items.indices.contains(index), index != currentIndex else { return }
        pager.setContentOffset(CGPoint(x: CGFloat(index) * pager.bounds.width, y: 0), animated: !reduceMotion)
        if reduceMotion { scrollViewDidScroll(pager) }
    }

    func currentImageFrameInWindow() -> CGRect? {
        guard let page = currentPage, window != nil else { return nil }
        return page.imageView.convert(page.imageView.bounds, to: nil)
    }

    // MARK: - Paging

    func scrollViewDidScroll(_ scrollView: UIScrollView) {
        guard scrollView === pager, !placing, pager.bounds.width > 0, !items.isEmpty else { return }
        let index = Int((pager.contentOffset.x / pager.bounds.width).rounded())
        let clamped = max(0, min(items.count - 1, index))
        guard clamped != currentIndex else { return }
        currentIndex = clamped
        refreshAccessibility()
        onPage?(items[clamped].id)
    }

    func scrollViewDidEndDecelerating(_ scrollView: UIScrollView) { settlePages(scrollView) }

    func scrollViewDidEndScrollingAnimation(_ scrollView: UIScrollView) { settlePages(scrollView) }

    /// A page that went off screen comes back at 1×, as in Photos.
    private func settlePages(_ scrollView: UIScrollView) {
        guard scrollView === pager else { return }
        for (index, page) in pages.enumerated() where index != currentIndex {
            page.resetZoom(animated: false)
        }
    }

    // MARK: - Swipe down to close

    /// Only a drag that starts DOWNWARD, on a picture at 1×, between pages
    /// — zoomed, a drag pans; mid-page, it is still paging; sideways, it pages.
    private func dismissMayBegin() -> Bool {
        guard let page = currentPage, !page.isZoomedIn, !page.isZooming, !page.isZoomBouncing else { return false }
        guard abs(pager.contentOffset.x - CGFloat(currentIndex) * pager.bounds.width) < 1 else { return false }
        let velocity = dismissPan.velocity(in: self)
        return velocity.y > abs(velocity.x)
    }

    @objc private func dismissDrag(_ pan: UIPanGestureRecognizer) {
        guard let page = currentPage else { return }
        let translation = pan.translation(in: self)
        let height = max(1, bounds.height)
        let down = max(0, translation.y)
        switch pan.state {
        case .began:
            onDragBegan?()
        case .changed:
            let scale = 1 - ScreenshotViewerGeometry.dragShrink * min(1, down / height)
            page.transform = CGAffineTransform(translationX: translation.x, y: translation.y)
                .scaledBy(x: scale, y: scale)
            onDragChanged?(min(1, down / (height * ScreenshotViewerGeometry.fadeDistance)))
        case .ended, .cancelled, .failed:
            let velocity = pan.velocity(in: self).y
            let close = pan.state == .ended && Self.releaseCloses(
                travelled: translation.y, height: height, velocity: velocity
            )
            if close {
                onDragEnded?(currentImageFrameInWindow())
            } else {
                UIView.animate(withDuration: 0.4, delay: 0, usingSpringWithDamping: 0.84,
                               initialSpringVelocity: 0, options: [.allowUserInteraction, .beginFromCurrentState]) {
                    page.transform = .identity
                }
                onDragEnded?(nil)
            }
        default:
            break
        }
    }

    /// Past ~20 % of the height, or a fast flick down, closes (C2); a flick
    /// back up keeps it open whatever the distance.
    static func releaseCloses(travelled: CGFloat, height: CGFloat, velocity: CGFloat) -> Bool {
        guard travelled > 0 else { return false }
        if velocity > ScreenshotViewerGeometry.flickVelocity { return true }
        return travelled > height * ScreenshotViewerGeometry.closeFraction
            && velocity > ScreenshotViewerGeometry.cancelVelocity
    }

    // MARK: - Accessibility

    private func refreshAccessibility() {
        accessibilityLabel = items.isEmpty ? nil : label(currentIndex)
        accessibilityValue = items.count > 1 ? "\(currentIndex + 1) / \(items.count)" : nil
        accessibilityTraits = items.count > 1 ? [.image, .adjustable] : [.image]
    }

    override var accessibilityFrame: CGRect {
        get {
            guard let page = currentPage else { return super.accessibilityFrame }
            return UIAccessibility.convertToScreenCoordinates(page.imageView.bounds, in: page.imageView)
        }
        set { super.accessibilityFrame = newValue }
    }

    override func accessibilityIncrement() { show(index: currentIndex + 1) }

    override func accessibilityDecrement() { show(index: currentIndex - 1) }

    override func accessibilityScroll(_ direction: UIAccessibilityScrollDirection) -> Bool {
        let next: Int
        switch direction {
        case .left, .next: next = currentIndex + 1
        case .right, .previous: next = currentIndex - 1
        default: return false
        }
        guard items.indices.contains(next) else { return false }
        show(index: next)
        UIAccessibility.post(notification: .pageScrolled, argument: "\(next + 1) / \(items.count)")
        return true
    }
}

/// Whether the swipe down may start. UIKit asks a recogniser's DELEGATE on
/// every attempt; the view it is attached to is asked only when it is the
/// view under the finger — here that is the page, so a filter on the pager
/// view itself was never consulted and every drag, sideways ones included,
/// became a swipe down (device run, 2026-09-27: paging did nothing).
private final class DismissGate: NSObject, UIGestureRecognizerDelegate {
    var shouldBegin: () -> Bool = { false }

    func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool { shouldBegin() }
}

/// One picture: aspect-fit in the page — the area between the bars — at
/// 1×, zoomable to 4×, and pannable while zoomed, clipped at the bars.
final class ScreenshotZoomPage: UIScrollView, UIScrollViewDelegate {
    let id: UUID
    let imageView = UIImageView()
    private var fittedFor: CGSize?
    private let doubleTap = UITapGestureRecognizer()

    init(id: UUID, image: UIImage) {
        self.id = id
        super.init(frame: .zero)
        imageView.image = image
        imageView.contentMode = .scaleAspectFill
        imageView.clipsToBounds = true
        addSubview(imageView)
        delegate = self
        showsHorizontalScrollIndicator = false
        showsVerticalScrollIndicator = false
        contentInsetAdjustmentBehavior = .never
        decelerationRate = .fast
        bouncesZoom = true
        alwaysBounceVertical = false
        alwaysBounceHorizontal = false
        scrollsToTop = false
        minimumZoomScale = 1
        maximumZoomScale = ScreenshotViewerGeometry.maxZoom
        doubleTap.numberOfTapsRequired = 2
        doubleTap.addTarget(self, action: #selector(doubleTapped(_:)))
        addGestureRecognizer(doubleTap)
        isAccessibilityElement = false
        accessibilityElementsHidden = true
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

    var isZoomedIn: Bool { zoomScale > minimumZoomScale + 0.01 }

    /// Fit the picture into the page at 1× — only when the page's size
    /// changed, so a page kept across a removal keeps its zoom.
    func fit() {
        guard bounds.width > 0, bounds.height > 0, let image = imageView.image else { return }
        if fittedFor == bounds.size { return }
        fittedFor = bounds.size
        zoomScale = 1
        let frame = ScreenshotViewerState.fitRect(image: image.size, in: bounds.size, insets: .zero)
        imageView.frame = CGRect(origin: .zero, size: frame.size)
        contentSize = frame.size
        centre()
        contentOffset = .zero
    }

    func resetZoom(animated: Bool) {
        guard isZoomedIn else { return }
        setZoomScale(minimumZoomScale, animated: animated)
    }

    /// Double tap: to 2× about the tap, or back to 1×.
    func toggleZoom(at point: CGPoint, animated: Bool = true) {
        if isZoomedIn {
            setZoomScale(minimumZoomScale, animated: animated)
            return
        }
        let scale = min(maximumZoomScale, ScreenshotViewerGeometry.doubleTapZoom)
        let size = CGSize(width: bounds.width / scale, height: bounds.height / scale)
        zoom(to: CGRect(x: point.x - size.width / 2, y: point.y - size.height / 2,
                        width: size.width, height: size.height), animated: animated)
    }

    @objc private func doubleTapped(_ tap: UITapGestureRecognizer) {
        toggleZoom(at: tap.location(in: imageView))
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        centre()
    }

    /// Smaller than the page, the picture sits in its middle; larger, it
    /// starts at the page's edge and scrolls.
    private func centre() {
        var frame = imageView.frame
        frame.origin.x = frame.width < bounds.width ? (bounds.width - frame.width) / 2 : 0
        frame.origin.y = frame.height < bounds.height ? (bounds.height - frame.height) / 2 : 0
        imageView.frame = frame
    }

    func viewForZooming(in scrollView: UIScrollView) -> UIView? { imageView }

    func scrollViewDidZoom(_ scrollView: UIScrollView) { centre() }
}

/// The pager inside SwiftUI.
struct ScreenshotPager: UIViewRepresentable {
    let items: [ScreenshotPagerItem]
    let current: UUID
    let insets: UIEdgeInsets
    let label: (Int) -> String
    let reduceMotion: Bool
    let handle: ScreenshotPagerHandle
    let onPage: (UUID) -> Void
    let onDragBegan: () -> Void
    let onDragChanged: (CGFloat) -> Void
    let onDragEnded: (CGRect?) -> Void

    func makeUIView(context: Context) -> ScreenshotPagerView {
        let view = ScreenshotPagerView()
        handle.view = view
        return view
    }

    func updateUIView(_ view: ScreenshotPagerView, context: Context) {
        handle.view = view
        view.onPage = onPage
        view.onDragBegan = onDragBegan
        view.onDragChanged = onDragChanged
        view.onDragEnded = onDragEnded
        view.label = label
        view.reduceMotion = reduceMotion
        view.setInsets(insets)
        view.update(items: items, current: current)
    }
}
