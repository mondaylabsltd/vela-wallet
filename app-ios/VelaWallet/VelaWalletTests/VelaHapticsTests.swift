//
//  VelaHapticsTests.swift
//  VelaWalletTests
//
//  The haptics Android plays and iOS did not (074), pinned at the call: the
//  signing slider's threshold, a copy, and the send machine's two outcomes.
//  A simulator cannot vibrate; `VelaHaptic.recording` is what can be read.
//

import Testing
import UIKit
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

    @Test func aCopyIsOneSelectAndPutsTheValueOnTheClipboard() {
        let played = VelaHaptic.recording { velaCopy("0x14fB1f0000000000000000000000000000D1eA5c") }
        #expect(played == [.select])
        #expect(UIPasteboard.general.string == "0x14fB1f0000000000000000000000000000D1eA5c")
    }

    /// Nothing to copy is not a copy: no tick for an empty clipboard write.
    @Test func copyingNothingPlaysNothing() {
        let played = VelaHaptic.recording { velaCopy("") }
        #expect(played.isEmpty)
    }

    /// Spec 043's `haptic { kind }`: money left, or a refusal.
    @Test func theSendMachinesTwoKindsAreSuccessAndReject() {
        #expect(VelaHaptic(sendKind: "success") == .success)
        #expect(VelaHaptic(sendKind: "error") == .reject)
    }
}
