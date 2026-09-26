//
//  TextScaleSliderTests.swift
//  VelaWalletTests
//
//  The text-size slider (the founder: 没有滑动的感觉): where a finger's x
//  lands among the six stops, the one detent each new stop plays, and how
//  far the thumb lifts under a finger or a pointer. A simulator cannot
//  vibrate, so what is pinned is the call — through `VelaHaptic.recording`.
//

import CoreGraphics
import Testing
@testable import VelaWallet

@MainActor
struct TextScaleSliderTests {

    /// Six stops, as `TextScaleLevel` has; a 300pt track with the stops kept
    /// 10pt in from each end — stops at 10, 66, 122, 178, 234, 290.
    private let steps = TextScaleLevel.allCases.count
    private let width: CGFloat = 300
    private let inset: CGFloat = 10

    private func stop(_ x: CGFloat) -> Int {
        TextScaleTrack.stop(at: x, width: width, steps: steps, inset: inset)
    }

    @Test func everyStopIsWhereItIsDrawn() {
        #expect(steps == 6)
        for index in 0..<steps {
            let center = TextScaleTrack.center(of: index, width: width, steps: steps, inset: inset)
            #expect(stop(center) == index, "stop \(index)'s own centre reads as another stop")
        }
        #expect(TextScaleTrack.center(of: 0, width: width, steps: steps, inset: inset) == inset)
        #expect(TextScaleTrack.center(of: steps - 1, width: width, steps: steps, inset: inset) == width - inset)
    }

    @Test func aFingerBetweenTwoStopsGetsTheNearer() {
        #expect(stop(37) == 0)   // 27 past stop 0, 29 short of stop 1
        #expect(stop(39) == 1)
        #expect(stop(149) == 2)  // 150 is the midpoint of 122 and 178
        #expect(stop(151) == 3)
    }

    @Test func aFingerPastEitherEndStaysOnTheEndStop() {
        #expect(stop(-40) == 0)
        #expect(stop(0) == 0)
        #expect(stop(width) == steps - 1)
        #expect(stop(width + 80) == steps - 1)
    }

    @Test func aTrackWithNoRoomOrOneStopIsStopZero() {
        #expect(TextScaleTrack.stop(at: 50, width: 0, steps: 6, inset: 10) == 0)
        #expect(TextScaleTrack.stop(at: 50, width: 300, steps: 1, inset: 10) == 0)
    }

    /// A drag from the small end to the large end, one point at a time: five
    /// stops crossed, five detents — and the size stored once, on release.
    @Test func aDragAcrossTheTrackPlaysOneDetentPerStop() {
        var drag = TextScaleDrag()
        var committed: Int?
        let played = VelaHaptic.recording {
            for x in stride(from: CGFloat(0), through: width, by: 1) {
                drag.move(to: stop(x), committed: 0)
            }
            committed = drag.release(committed: 0)
        }
        #expect(played == Array(repeating: .detent, count: steps - 1))
        #expect(committed == steps - 1)
        #expect(drag.shown == nil)
    }

    /// There and back: every stop entered is one detent, in both directions.
    @Test func aDragThereAndBackPlaysADetentOnEveryStopEntered() {
        var drag = TextScaleDrag()
        var committed: Int?
        let played = VelaHaptic.recording {
            for x in stride(from: CGFloat(122), through: 290, by: 2) { drag.move(to: stop(x), committed: 2) }
            for x in stride(from: CGFloat(290), through: 122, by: -2) { drag.move(to: stop(x), committed: 2) }
            committed = drag.release(committed: 2)
        }
        // 2→3→4→5, then 5→4→3→2: six stops entered.
        #expect(played.count == 6)
        #expect(played.allSatisfy { $0 == .detent })
        // Back where it started: nothing to store.
        #expect(committed == nil)
    }

    @Test func wobblingInsideOneStopPlaysNothing() {
        var drag = TextScaleDrag()
        let played = VelaHaptic.recording {
            for x: CGFloat in [170, 175, 182, 186, 179, 171] { drag.move(to: stop(x), committed: 3) }
            #expect(drag.release(committed: 3) == nil)
        }
        #expect(played.isEmpty)
    }

    /// A tap is a move and a release: one detent to the tapped stop, stored at once.
    @Test func aTapJumpsToTheTappedStop() {
        var drag = TextScaleDrag()
        var committed: Int?
        let played = VelaHaptic.recording {
            drag.move(to: stop(234), committed: 1)
            committed = drag.release(committed: 1)
        }
        #expect(played == [.detent])
        #expect(committed == 4)
    }

    @Test func aTapOnTheStoredStopStoresNothingAndPlaysNothing() {
        var drag = TextScaleDrag()
        var committed: Int?
        let played = VelaHaptic.recording {
            drag.move(to: stop(122), committed: 2)
            committed = drag.release(committed: 2)
        }
        #expect(played.isEmpty)
        #expect(committed == nil)
    }

    // MARK: - The thumb answers the finger (the desktop's slider, 2026-09)

    /// The inset IS the room the drawing has: at every lift the thumb and its
    /// ring fit inside the track on the end stops, and a lifted thumb has a
    /// ring that shows around it.
    @Test func theThumbAndItsRingAreWholeOnEitherEndStop() {
        for lift in [TextScaleLift.rest, .hover, .pressed] {
            #expect(lift.ring / 2 <= TextScaleSlider.inset, "the \(lift) ring is cut off at the track's ends")
            #expect(lift.thumb / 2 <= TextScaleSlider.inset, "the \(lift) thumb is cut off at the track's ends")
        }
        #expect(TextScaleLift.rest.ringOpacity == 0)
        #expect(TextScaleLift.rest.thumb < TextScaleLift.hover.thumb)
        #expect(TextScaleLift.hover.thumb < TextScaleLift.pressed.thumb)
        #expect(TextScaleLift.hover.ring > TextScaleLift.hover.thumb)
        #expect(TextScaleLift.pressed.ring > TextScaleLift.hover.ring)
        #expect(TextScaleLift.pressed.ringOpacity > TextScaleLift.hover.ringOpacity)
    }

    /// A finger landing lifts the thumb before the tap or the pan has said
    /// anything; lifting it puts the thumb back, storing and playing nothing.
    @Test func aFingerLiftsTheThumbTheMomentItLands() {
        var drag = TextScaleDrag()
        let played = VelaHaptic.recording {
            #expect(drag.lift == .rest)
            drag.press(true)
            #expect(drag.lift == .pressed)
            drag.press(false)
            #expect(drag.release(committed: 2) == nil)
        }
        #expect(drag.lift == .rest)
        #expect(played.isEmpty)
    }

    /// A tap is down, move, release, up — pressed until the finger is gone.
    @Test func aTapStaysPressedUntilTheFingerLifts() {
        var drag = TextScaleDrag()
        drag.press(true)
        drag.move(to: stop(234), committed: 1)
        #expect(drag.lift == .pressed)
        #expect(drag.release(committed: 1) == 4)
        #expect(drag.lift == .pressed, "the thumb settled before the finger lifted")
        drag.press(false)
        #expect(drag.lift == .rest)
    }

    /// A drag is pressed until BOTH the finger and the pan have let go: the
    /// two reports arrive in either order, and neither alone settles it.
    @Test func aDragStaysPressedUntilThePanAndTheFingerHaveLetGo() {
        var fingerFirst = TextScaleDrag()
        fingerFirst.press(true)
        fingerFirst.move(to: stop(122), committed: 0)
        fingerFirst.press(false)
        #expect(fingerFirst.lift == .pressed, "the thumb settled mid-drag")
        #expect(fingerFirst.release(committed: 0) == 2)
        #expect(fingerFirst.lift == .rest)

        var panFirst = TextScaleDrag()
        panFirst.press(true)
        panFirst.move(to: stop(122), committed: 0)
        _ = panFirst.release(committed: 0)
        #expect(panFirst.lift == .pressed)
        panFirst.press(false)
        #expect(panFirst.lift == .rest)
    }

    /// A vertical swipe that starts on the slider: the page's scroll takes the
    /// touch, the press ends, and nothing moved.
    @Test func theScrollTakingTheTouchSettlesTheThumb() {
        var drag = TextScaleDrag()
        drag.press(true)
        drag.press(false)
        #expect(drag.lift == .rest)
        #expect(drag.shown == nil)
    }

    /// Under a pointer the thumb lifts a little and the tick a click
    /// would land on is marked — not the one the thumb is on, and not while
    /// pressed.
    @Test func aPointerMarksTheStopAClickWouldLandOn() {
        var drag = TextScaleDrag()
        drag.hover(over: stop(234))
        #expect(drag.lift == .hover)
        #expect(drag.aimed(committed: 2) == 4)
        drag.hover(over: stop(122))
        #expect(drag.aimed(committed: 2) == nil, "the thumb's own stop was marked")
        drag.hover(over: stop(290))
        drag.press(true)
        #expect(drag.lift == .pressed)
        #expect(drag.aimed(committed: 2) == nil, "a stop was marked under a pressed thumb")
        drag.press(false)
        #expect(drag.aimed(committed: 2) == 5)
        drag.hover(over: nil)
        #expect(drag.lift == .rest)
        #expect(drag.aimed(committed: 2) == nil)
    }
}
