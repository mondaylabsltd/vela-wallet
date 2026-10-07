//
//  DocumentPortsTests.swift
//  VelaWalletTests
//
//  Issue #449: Export contacts did nothing. The share sheet was presented from
//  the menu while the menu was leaving; UIKit refused it with a console line
//  and nothing else, and the `await` waited for an answer that never came.
//  A presentation UIKit will not take must answer at once.
//

import Testing
import UIKit
import UniformTypeIdentifiers
@testable import VelaWallet

@MainActor
struct DocumentPortsTests {

    /// A controller in no window — one UIKit refuses to present from.
    private func refusing() -> UIKitDocumentPorts {
        let detached = UIViewController()
        return UIKitDocumentPorts(presenter: { detached })
    }

    @Test func aRefusedShareAnswersInsteadOfWaiting() async {
        let shared = await refusing().share(name: "contacts.json", type: .json, bytes: Data("{}".utf8))
        #expect(shared == false)
    }

    @Test func aRefusedSaveAnswersInsteadOfWaiting() async {
        let saved = await refusing().create(name: "contacts.json", type: .json, bytes: Data("{}".utf8))
        #expect(saved == false)
    }

    @Test func aRefusedPickAnswersInsteadOfWaiting() async {
        let picked = await refusing().pick(types: DocumentTypes.addressBook)
        #expect(picked == nil)
    }
}

/// Found on the device the moment Export worked (#449): swiped away, the share
/// sheet calls its completion handler twice, and a checked continuation
/// resumed twice traps. The answer is given once.
@MainActor
struct ShareAnswerTests {
    @Test func aSecondAnswerIsIgnored() async {
        let completed = await withCheckedContinuation { continuation in
            let answer = ShareAnswer(continuation)
            answer.resume(false)
            answer.resume(true)
        }
        #expect(completed == false)
    }
}
