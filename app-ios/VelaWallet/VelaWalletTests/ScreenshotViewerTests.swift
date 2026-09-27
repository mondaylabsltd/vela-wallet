//
//  ScreenshotViewerTests.swift
//  VelaWalletTests
//
//  Tap a screenshot to preview it (spec C, 2026-09-27). The rules the viewer
//  lives by, without a screen: which tile opens it and at which picture,
//  paging, what a removal leaves on screen (middle / last / only), where
//  focus goes back to, and that nothing opens while a tile is processing or
//  while the report is on its way. Then the UIKit pager itself: the picture
//  it shows, what VoiceOver hears, the zoom, and the swipe-down line.
//
//  "A scroll that starts on a tile never opens it" is a property of real
//  touches, which no unit test can make: it is proved on the iPhone by
//  `FeedbackReportDeviceTests.testCIScrollFromTileNeverOpens`.
//
//  The network is never touched: every sender here has a transport that
//  refuses.
//

import Foundation
import Testing
import UIKit
@testable import VelaWallet

@MainActor
struct ScreenshotViewerTests {

    private func png(_ width: Int, _ height: Int) -> Data {
        let format = UIGraphicsImageRendererFormat()
        format.scale = 1
        format.opaque = true
        return UIGraphicsImageRenderer(size: CGSize(width: width, height: height), format: format).image { context in
            UIColor(hue: CGFloat(width % 360) / 360, saturation: 0.5, brightness: 0.9, alpha: 1).setFill()
            context.fill(CGRect(x: 0, y: 0, width: width, height: height))
        }.pngData()!
    }

    private func sender() -> FeedbackSender {
        FeedbackSender(endpoint: "https://example.test/api/bug-report", transport: { _ in throw URLError(.cancelled) })
    }

    /// A tray of `count` processed screenshots, each a different width so a
    /// test can tell them apart.
    private func tray(_ count: Int) async -> FeedbackSender {
        let sender = sender()
        await sender.add(datas: (0..<count).map { png(100 * ($0 + 1), 200) })
        #expect(sender.shots.count == count)
        #expect(sender.shots.allSatisfy { $0.prepared != nil })
        return sender
    }

    private func shot(ready: Bool) -> FeedbackSender.Shot {
        FeedbackSender.Shot(
            id: UUID(),
            prepared: ready ? ScreenshotPrep.Prepared(jpeg: Data([0xFF, 0xD8, 0xFF]), pixelSize: CGSize(width: 1, height: 1)) : nil
        )
    }

    // MARK: - Opening

    @Test func aTileOpensTheViewerAtItsOwnPicture() async throws {
        let sender = await tray(3)
        let ids = sender.shots.map(\.id)
        for (index, id) in ids.enumerated() {
            let state = try #require(ScreenshotViewerState(opening: id, shots: sender.shots, sending: false))
            #expect(state.current == id)
            #expect(state.index == index)
            #expect(state.count == 3)
            #expect(state.ids == ids, "in tile order")
        }
    }

    @Test func aProcessingTileDoesNotOpenAndIsNotAPage() {
        let ready = shot(ready: true)
        let processing = shot(ready: false)
        let later = shot(ready: true)
        let shots = [ready, processing, later]
        #expect(ScreenshotViewerState(opening: processing.id, shots: shots, sending: false) == nil)
        let state = ScreenshotViewerState(opening: later.id, shots: shots, sending: false)
        #expect(state?.ids == [ready.id, later.id], "a processing tile is not one of the viewer's pictures")
        #expect(state?.index == 1)
        #expect(ScreenshotViewerState(opening: UUID(), shots: shots, sending: false) == nil, "a tile that is gone")
    }

    /// C5: the form is inert while sending — no tile opens.
    @Test func nothingOpensWhileTheReportIsSending() async {
        var release: CheckedContinuation<Void, Never>?
        let sender = FeedbackSender(endpoint: "https://example.test/api/bug-report", transport: { request in
            await withCheckedContinuation { release = $0 }
            return (Data(#"{"number":1,"url":"https://github.com/o/r/issues/1"}"#.utf8),
                    HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: nil)!)
        })
        await sender.add(datas: [png(300, 600), png(400, 600)])
        let id = sender.shots[0].id
        #expect(ScreenshotViewerState(opening: id, shots: sender.shots, sending: sender.sending) != nil)
        let sending = Task { await sender.send(what: "Froze", steps: "", previewLines: [], version: "1") }
        while release == nil { await Task.yield() }
        #expect(sender.sending)
        #expect(ScreenshotViewerState(opening: id, shots: sender.shots, sending: sender.sending) == nil)
        release?.resume()
        await sending.value
        #expect(ScreenshotViewerState(opening: id, shots: sender.shots, sending: sender.sending) != nil,
                "after the send the tiles open again")
    }

    // MARK: - Paging

    @Test func pagingMovesWithinTheTray() async throws {
        let sender = await tray(3)
        let ids = sender.shots.map(\.id)
        var state = try #require(ScreenshotViewerState(opening: ids[0], shots: sender.shots, sending: false))
        state.page(to: 2)
        #expect(state.current == ids[2] && state.index == 2)
        state.page(to: 3)
        #expect(state.current == ids[2], "past the end is no page")
        state.page(to: -1)
        #expect(state.current == ids[2])
        state.show(ids[1])
        #expect(state.index == 1)
        state.show(UUID())
        #expect(state.index == 1, "an unknown id is ignored")
    }

    // MARK: - Removing from the viewer

    @Test func removingTheMiddleShowsTheNext() async throws {
        let sender = await tray(3)
        let ids = sender.shots.map(\.id)
        var state = try #require(ScreenshotViewerState(opening: ids[1], shots: sender.shots, sending: false))
        sender.remove(state.current)
        let alive = state.sync(sender.shots)
        #expect(alive)
        #expect(state.current == ids[2], "the next one takes its place")
        #expect(state.index == 1 && state.count == 2)
        #expect(ScreenshotViewerState.returnFocus(closingOn: state.current, shots: sender.shots) == .tile(ids[2]))
    }

    @Test func removingTheLastShowsThePrevious() async throws {
        let sender = await tray(3)
        let ids = sender.shots.map(\.id)
        var state = try #require(ScreenshotViewerState(opening: ids[2], shots: sender.shots, sending: false))
        sender.remove(state.current)
        let alive = state.sync(sender.shots)
        #expect(alive)
        #expect(state.current == ids[1], "the previous one, when the last went")
        #expect(state.index == 1 && state.count == 2)
    }

    @Test func removingTheOnlyOneClosesAndFocusGoesToTheAddTarget() async throws {
        let sender = await tray(1)
        let id = sender.shots[0].id
        var state = try #require(ScreenshotViewerState(opening: id, shots: sender.shots, sending: false))
        sender.remove(state.current)
        let alive = state.sync(sender.shots)
        #expect(!alive, "nothing left: the viewer closes")
        #expect(sender.shots.isEmpty)
        #expect(ScreenshotViewerState.returnFocus(closingOn: nil, shots: sender.shots) == .add)
        #expect(ScreenshotViewerState.returnFocus(closingOn: id, shots: sender.shots) == .add)
    }

    /// Removing from the viewer is the tray's own rule: order kept, the
    /// picture next to it moves up, the counter follows.
    @Test func removingTwiceWalksTheTrayLikeTheTiles() async throws {
        let sender = await tray(4)
        let ids = sender.shots.map(\.id)
        var state = try #require(ScreenshotViewerState(opening: ids[1], shots: sender.shots, sending: false))
        sender.remove(state.current)
        let first = state.sync(sender.shots)
        #expect(first && state.current == ids[2])
        sender.remove(state.current)
        let second = state.sync(sender.shots)
        #expect(second && state.current == ids[3])
        #expect(state.ids == [ids[0], ids[3]])
        #expect(sender.shots.map(\.id) == [ids[0], ids[3]])
    }

    /// A tile that finishes while the viewer is up joins it without moving
    /// the picture on screen.
    @Test func aTileFinishingWhileOpenJoinsWithoutMovingThePicture() {
        let first = shot(ready: true)
        var pending = shot(ready: false)
        let last = shot(ready: true)
        var state = ScreenshotViewerState(opening: last.id, shots: [first, pending, last], sending: false)!
        #expect(state.index == 1 && state.count == 2)
        pending.prepared = first.prepared
        let alive = state.sync([first, pending, last])
        #expect(alive)
        #expect(state.current == last.id)
        #expect(state.index == 2 && state.count == 3)
    }

    // MARK: - Closing returns focus

    @Test func closingReturnsFocusToTheTileOfThePictureOnScreen() async throws {
        let sender = await tray(3)
        let ids = sender.shots.map(\.id)
        var state = try #require(ScreenshotViewerState(opening: ids[0], shots: sender.shots, sending: false))
        #expect(ScreenshotViewerState.returnFocus(closingOn: state.current, shots: sender.shots) == .tile(ids[0]),
                "closed where it opened: back to the tile that opened it")
        state.page(to: 2)
        #expect(ScreenshotViewerState.returnFocus(closingOn: state.current, shots: sender.shots) == .tile(ids[2]))
    }

    // MARK: - Geometry

    /// The picture sits aspect-fit BETWEEN the bars, centred in that area —
    /// never under them.
    @Test func thePictureFitsBetweenTheBars() {
        let insets = UIEdgeInsets(top: 108, left: 0, bottom: 94, right: 0)
        let screen = CGSize(width: 414, height: 896)
        // A phone screenshot (tall): the height is the limit.
        let tall = ScreenshotViewerState.fitRect(image: CGSize(width: 887, height: 1920), in: screen, insets: insets)
        #expect(abs(tall.minY - 108) < 0.001 && abs(tall.maxY - (896 - 94)) < 0.001)
        #expect(abs(tall.midX - 207) < 0.001)
        #expect(abs(tall.width / tall.height - 887.0 / 1920.0) < 0.0001)
        // A landscape one: the width is the limit, centred between the bars.
        let wide = ScreenshotViewerState.fitRect(image: CGSize(width: 1600, height: 1200), in: screen, insets: insets)
        #expect(abs(wide.width - 414) < 0.001 && abs(wide.height - 310.5) < 0.001)
        #expect(abs(wide.midY - (108 + (896 - 108 - 94) / 2)) < 0.001)
        #expect(wide.minY > 108 && wide.maxY < 896 - 94)
    }

    /// Past ~20 % of the height, or a fast flick down, closes; a flick back
    /// up keeps it open; a drag up never closes.
    @Test func theSwipeDownLine() {
        let height: CGFloat = 896
        #expect(!ScreenshotPagerView.releaseCloses(travelled: 100, height: height, velocity: 0))
        #expect(ScreenshotPagerView.releaseCloses(travelled: 200, height: height, velocity: 0))
        #expect(ScreenshotPagerView.releaseCloses(travelled: 30, height: height, velocity: 1400), "a fast flick")
        #expect(!ScreenshotPagerView.releaseCloses(travelled: 300, height: height, velocity: -800), "flicked back up")
        #expect(!ScreenshotPagerView.releaseCloses(travelled: -300, height: height, velocity: 2000), "dragged up")
    }

    // MARK: - The pager

    private func pager(_ sender: FeedbackSender, current: Int) -> ScreenshotPagerView {
        let view = ScreenshotPagerView(frame: CGRect(x: 0, y: 0, width: 414, height: 896))
        view.label = { "View screenshot \($0 + 1)" }
        view.reduceMotion = true
        view.setInsets(UIEdgeInsets(top: 108, left: 0, bottom: 94, right: 0))
        view.update(items: items(sender), current: sender.shots[current].id)
        view.layoutIfNeeded()
        return view
    }

    private func items(_ sender: FeedbackSender) -> [ScreenshotPagerItem] {
        sender.shots.compactMap { shot in
            ScreenshotImages.image(for: shot).map { ScreenshotPagerItem(id: shot.id, image: $0) }
        }
    }

    @Test func thePagerOpensOnThePictureAndTellsVoiceOverWhichOne() async throws {
        let sender = await tray(3)
        let view = pager(sender, current: 1)
        #expect(view.currentIndex == 1)
        #expect(view.accessibilityLabel == "View screenshot 2")
        #expect(view.accessibilityValue == "2 / 3")
        #expect(view.accessibilityTraits.contains(.adjustable))
        #expect(view.accessibilityIdentifier == "feedback.viewer.image")
        // The page puts the picture exactly where the SwiftUI flight lands.
        let page = try #require(view.currentPage)
        let image = try #require(page.imageView.image)
        let expected = ScreenshotViewerState.fitRect(image: image.size, in: view.bounds.size,
                                                     insets: UIEdgeInsets(top: 108, left: 0, bottom: 94, right: 0))
        let drawn = page.imageView.convert(page.imageView.bounds, to: view)
        #expect(abs(drawn.minX - expected.minX) < 0.5 && abs(drawn.minY - expected.minY) < 0.5, "\(drawn) vs \(expected)")
        #expect(abs(drawn.width - expected.width) < 0.5 && abs(drawn.height - expected.height) < 0.5)
    }

    /// Zoomed, the picture is cut off at the bars — the page IS the area
    /// between them — so it never runs under the white controls.
    @Test func aZoomedPictureStaysBetweenTheBars() async throws {
        let sender = await tray(1)
        let view = pager(sender, current: 0)
        let page = try #require(view.currentPage)
        let pageFrame = page.convert(page.bounds, to: view)
        #expect(pageFrame == CGRect(x: 0, y: 108, width: 414, height: 896 - 108 - 94))
        #expect(page.clipsToBounds)
        page.toggleZoom(at: CGPoint(x: page.imageView.bounds.midX, y: page.imageView.bounds.midY), animated: false)
        #expect(view.isZoomedIn)
        #expect(page.imageView.frame.height > page.bounds.height, "zoomed past the page, and clipped by it")
    }

    @Test func voiceOverAdjustPagesAndReportsIt() async throws {
        let sender = await tray(3)
        let view = pager(sender, current: 0)
        var paged: [UUID] = []
        view.onPage = { paged.append($0) }
        view.accessibilityIncrement()
        #expect(view.currentIndex == 1)
        #expect(paged == [sender.shots[1].id])
        #expect(view.accessibilityValue == "2 / 3")
        view.accessibilityDecrement()
        view.accessibilityDecrement()
        #expect(view.currentIndex == 0, "no page before the first")
    }

    @Test func thePagerFollowsARemovalToTheNextPicture() async throws {
        let sender = await tray(3)
        let view = pager(sender, current: 1)
        let removed = sender.shots[1].id
        let third = sender.shots[2].id
        sender.remove(removed)
        // As the viewer does: the tray first (still pointing at the removed
        // one), then the state's replacement.
        view.update(items: items(sender), current: removed)
        #expect(view.currentIndex == 1)
        #expect(view.items.map(\.id) == sender.shots.map(\.id))
        #expect(view.accessibilityValue == "2 / 2")
        view.update(items: items(sender), current: third)
        #expect(view.currentIndex == 1)
        // The last one removed: the previous shows.
        sender.remove(third)
        view.update(items: items(sender), current: third)
        #expect(view.currentIndex == 0)
        #expect(view.accessibilityValue == nil, "one picture has no counter")
        #expect(!view.accessibilityTraits.contains(.adjustable))
        // The only one removed: the pager keeps it while the viewer fades.
        let before = view.items.map(\.id)
        view.update(items: [], current: UUID())
        #expect(view.items.map(\.id) == before)
    }

    @Test func aDoubleTapZoomsToTwiceAndBack() async throws {
        let sender = await tray(1)
        let view = pager(sender, current: 0)
        let page = try #require(view.currentPage)
        #expect(!page.isZoomedIn)
        let centre = CGPoint(x: page.imageView.bounds.midX, y: page.imageView.bounds.midY)
        page.toggleZoom(at: centre, animated: false)
        #expect(abs(page.zoomScale - ScreenshotViewerGeometry.doubleTapZoom) < 0.01, "\(page.zoomScale)")
        #expect(page.isZoomedIn)
        page.toggleZoom(at: centre, animated: false)
        #expect(!page.isZoomedIn, "a second double tap resets")
        page.toggleZoom(at: centre, animated: false)
        page.resetZoom(animated: false)
        #expect(!page.isZoomedIn)
        #expect(page.maximumZoomScale == ScreenshotViewerGeometry.maxZoom)
    }

    // MARK: - Copy

    @Test func theViewersWordsAreFilled() {
        let en = SettingsFixtures.build(.st15, loc: Loc(overrideTag: "en")).feedback
        #expect(en.viewScreenshot == "View screenshot {{index}}")
        #expect(en.closeViewer == "Close")
        #expect(en.removeFromViewer == "Remove")
        let zh = SettingsFixtures.build(.st15, loc: Loc(overrideTag: "zh")).feedback
        for word in [zh.viewScreenshot, zh.closeViewer, zh.removeFromViewer] {
            #expect(!word.isEmpty && !word.contains("componentsUi."), "untranslated: \(word)")
        }
        #expect(zh.viewScreenshot.contains("{{index}}"))
        #expect(ScreenshotViewer.Words(viewScreenshot: en.viewScreenshot, close: "", remove: "").name(1)
                == "View screenshot 2")
    }
}
