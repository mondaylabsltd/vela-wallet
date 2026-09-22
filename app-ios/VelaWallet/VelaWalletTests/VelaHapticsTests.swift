//
//  VelaHapticsTests.swift
//  VelaWalletTests
//
//  The haptics Android plays and iOS did not (074), pinned at the call: the
//  signing slider's threshold, a copy, and the send machine's two outcomes.
//  A simulator cannot vibrate; `VelaHaptic.recording` is what can be read.
//

import Testing
@testable import VelaWallet

@MainActor
struct VelaHapticsTests {

    /// Released past 88% the slide is a signature, and the finger feels it —
    /// once. Short of it the knob goes back without a sound.
    @Test func theSigningSlideIsADetentOnlyPastItsThreshold() {
        var confirmed = false
        let short = VelaHaptic.recording {
            confirmed = SlideToConfirmView.released(at: ExploreGeometry.slideCommit - 0.01)
        }
        #expect(!confirmed)
        #expect(short.isEmpty)

        let past = VelaHaptic.recording {
            confirmed = SlideToConfirmView.released(at: ExploreGeometry.slideCommit)
        }
        #expect(confirmed)
        #expect(past == [.detent])
    }
}
