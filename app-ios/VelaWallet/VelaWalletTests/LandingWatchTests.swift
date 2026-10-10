//
//  LandingWatchTests.swift
//  VelaWalletTests
//
//  Issue #409, on the REAL session machine, the real executor and a real store,
//  with the registry answered from memory.
//
//  A one-key wallet is entered at the registry's 202 and its pending record is
//  left behind with the task id. The session confirms the landing — a read of
//  that task, never a passkey — and only then removes the record. A landing
//  that fails leaves the record, and with it the sign-out warning.
//

import Foundation
import Testing
@testable import VelaWallet

/// A registry whose task endpoint answers `body`, and that remembers what it
/// was asked.
private final class TaskScript: @unchecked Sendable {
    private let lock = NSLock()
    private var paths: [String] = []
    let body: String

    init(body: String) { self.body = body }

    var asked: [String] { lock.lock(); defer { lock.unlock() }; return paths }

    var transport: RegistryClient.Transport {
        { [self] request in
            guard let url = request.url,
                  let response = HTTPURLResponse(url: url, statusCode: 200, httpVersion: nil, headerFields: nil)
            else { throw URLError(.badURL) }
            lock.lock(); paths.append(url.path); lock.unlock()
            return (Data(body.utf8), response)
        }
    }
}

/// `timeLimit`: one wait here is for the registry to be asked, which no machine's idleness
/// bounds (`Waits.swift`). With no limit, one that never came would have
/// taken the job with it; this reports it.
@MainActor
@Suite(.hangLimit)
struct LandingWatchTests {

    private func seeded(task: String) -> (UserDefaults, AccountStore) {
        let defaults = UserDefaults(suiteName: "vela.tests.landing.\(UUID().uuidString)")!
        let shelf = VelaStore(defaults: defaults)
        shelf.writeList(VelaStore.Key.accounts, [[
            "id": "cred-1", "name": "Ann",
            "address": "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
            "public_key_hex": "04ab", "created_at_iso": "2026-10-04T10:00:00.000Z",
        ]])
        shelf.writeString(VelaStore.Key.activeIndex, "0")
        shelf.writeList(VelaStore.Key.pendingUploads, [
            // Something an older build or a test left behind: it must cost
            // only itself.
            ["unconfirmed": true],
            [
                "id": "cred-1", "name": "Ann", "public_key_hex": "04ab",
                "attestation_object_hex": "a0", "created_at_iso": "2026-10-04T10:00:00.000Z",
                "members": [[String: Any]](), "task_id": task,
            ],
        ])
        return (defaults, AccountStore(defaults: defaults))
    }

    private func outboxIds(_ store: AccountStore) async -> [String] {
        await store.loadPendingUploads().compactMap { $0["id"] as? String }
    }

    @Test func aLandingConfirmedAtLaunchRemovesTheRecordWithNothingButARead() async {
        let (_, store) = seeded(task: "t1")
        let registry = TaskScript(body: #"{"id":"t1","status":"done","txHash":"0xabc"}"#)
        let session = SessionController(store: store, registryTransport: registry.transport)
        session.boot()
        await Wait.until({ !session.view.loading }, orIdle: { session.isIdle })
        #expect(session.view.hasWallet)

        await Wait.until({ await outboxIds(store).isEmpty }, orIdle: { session.isIdle })
        #expect(await outboxIds(store).isEmpty, "confirmed, so the record is gone")
        #expect(await store.loadPendingUploads().count == 1, "the unreadable record was left alone")
        #expect(registry.asked == ["/api/task/t1"])
    }

    @Test func aFailedLandingKeepsTheRecordAndTheSignOutWarning() async {
        let (_, store) = seeded(task: "t2")
        let registry = TaskScript(body: #"{"id":"t2","status":"failed","error":"execution reverted"}"#)
        let session = SessionController(store: store, registryTransport: registry.transport)
        session.boot()
        await Wait.until({ !session.view.loading }, orIdle: { session.isIdle })
        await Wait.until({ !registry.asked.isEmpty && session.isIdle })
        #expect(await outboxIds(store) == ["cred-1"], "an unconfirmed landing removes nothing")

        session.signOut()
        await Wait.until({ session.view.signOut != nil }, orIdle: { session.isIdle })
        #expect(session.view.signOut?.pendingUploadWarning == true)
    }
}
