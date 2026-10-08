//
//  VelaHapticsTests.swift
//  VelaWalletTests
//
//  The haptics Android plays and iOS did not (074), pinned at the call: the
//  shared button's press (the signing confirm), a copy, and the send
//  machine's two outcomes.
//  A simulator cannot vibrate; `VelaHaptic.recording` is what can be read.
//

import Testing
import UIKit
@testable import VelaWallet

@MainActor
struct VelaHapticsTests {

    /// The signing sheet confirms with a tap (issue #461): the shared
    /// button's press, felt once, and the approve once — the 88% slide and its
    /// detent are gone.
    @Test func theSigningConfirmIsOnePressAndOneApprove() {
        var approved = 0
        let played = VelaHaptic.recording {
            VelaButton.pressed { approved += 1 }
        }
        #expect(approved == 1)
        #expect(played == [.press])
    }

    /// Read through `VelaClipboard.recording`, not `UIPasteboard.general`: the
    /// real pasteboard is an XPC call to a simulator daemon, and one that
    /// stopped answering stalled the whole suite for ten minutes (2026-09-28).
    @Test func aCopyIsOneSelectAndPutsTheValueOnTheClipboard() {
        var copied: [String] = []
        let played = VelaHaptic.recording {
            copied = VelaClipboard.recording { velaCopy("0x14fB1f0000000000000000000000000000D1eA5c") }
        }
        #expect(played == [.select])
        #expect(copied == ["0x14fB1f0000000000000000000000000000D1eA5c"])
    }

    /// Nothing to copy is not a copy: no tick for an empty clipboard write.
    @Test func copyingNothingPlaysNothing() {
        var copied: [String] = []
        let played = VelaHaptic.recording { copied = VelaClipboard.recording { velaCopy("") } }
        #expect(played.isEmpty)
        #expect(copied.isEmpty)
    }

    /// Spec 043's `haptic { kind }`: money left, or a refusal.
    @Test func theSendMachinesTwoKindsAreSuccessAndReject() {
        #expect(VelaHaptic(sendKind: "success") == .success)
        #expect(VelaHaptic(sendKind: "error") == .reject)
    }
}
