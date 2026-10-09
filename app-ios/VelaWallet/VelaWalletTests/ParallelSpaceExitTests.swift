//
//  ParallelSpaceExitTests.swift
//  VelaWalletTests
//
//  Leaving the parallel space puts the person back on the wallet they were on
//  (device pass 2026-10-09: it always landed on the first account — Android's
//  `ParallelSpaceExitTest` is the same finding). The real binding, the real
//  fixture keyset and the real stores, on a defaults suite of the test's own:
//  the space is entered and left the way a launch does it, through the
//  `VELA_PARALLEL_SPACE` pin.
//

#if DEBUG

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct ParallelSpaceExitTests {

    private let ann: [String: Any] = [
        "id": "cred-1", "name": "Ann", "address": "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
        "public_key_hex": "04ab", "created_at_iso": "2026-08-25T10:00:00.000Z",
        "keys": [["credential_id": "cred-1", "public_key_hex": "04ab", "name": "Ann"]],
    ]
    private let bo: [String: Any] = [
        "id": "cred-2", "name": "Bo", "address": "0x1F9840a85d5aF5bf1D1762F925BDADdC4201F984",
        "public_key_hex": "04cd", "created_at_iso": "2026-08-26T10:00:00.000Z",
        "keys": [["credential_id": "cred-2", "public_key_hex": "04cd", "name": "Bo"]],
    ]
    private var annAddress: String { ann["address"] as! String }
    private var boAddress: String { bo["address"] as! String }

    private static let enter = ["VELA_PARALLEL_SPACE": "1"]
    private static let leave = ["VELA_PARALLEL_SPACE": "0"]
    /// A relaunch with nothing pinned: the persisted flag decides.
    private static let relaunch: [String: String] = [:]

    /// A device holding Ann and Bo, Bo in front, on a defaults suite nobody
    /// else reads.
    private func device() async -> (store: VelaStore, accounts: AccountStore, suite: String) {
        let suite = "parallel-exit-\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suite)!
        let store = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        await accounts.saveAccount(ann)
        await accounts.saveAccount(bo)
        await accounts.saveActiveIndex(1)
        return (store, accounts, suite)
    }

    private func inFront(_ accounts: AccountStore) async -> String? {
        let list = await accounts.loadAccounts()
        let index = await accounts.loadActiveIndex()
        return list.indices.contains(index) ? list[index]["address"] as? String : nil
    }

    private func addresses(_ accounts: AccountStore) async -> [String] {
        await accounts.loadAccounts().compactMap { $0["address"] as? String }
    }

    /// Enters, and checks the space's own wallet is the one in front.
    private func enterTheSpace(_ store: VelaStore, _ accounts: AccountStore) async throws -> ParallelSpaceBinding {
        let binding = ParallelSpaceBinding()
        await binding.apply(environment: Self.enter, store: store, accounts: accounts)
        #expect(binding.isActive)
        let fixture = try fixtureMultiAddress()
        #expect(await inFront(accounts)?.caseInsensitiveCompare(fixture) == .orderedSame, "the space's wallet is in front")
        #expect(await accounts.loadAccounts().count == 3)
        return binding
    }

    @Test func leavingGoesBackToTheWalletThatWasInFrontBeforeTheSpace() async throws {
        let (store, accounts, suite) = await device()
        defer { UserDefaults().removePersistentDomain(forName: suite) }
        let binding = try await enterTheSpace(store, accounts)

        await binding.apply(environment: Self.leave, store: store, accounts: accounts)

        #expect(!binding.isActive)
        #expect(await addresses(accounts) == [annAddress, boAddress], "only the space's record went")
        #expect(await accounts.loadActiveIndex() == 1, "Bo was in front before the space")
        #expect(await inFront(accounts) == boAddress)
        #expect(store.readString(ParallelSpaceBinding.returnToKey) == nil, "nothing is left remembered")
        #expect(store.readString(ParallelSpaceBinding.flagKey) == nil)
    }

    @Test func aWalletSwitchedToInsideTheSpaceStaysInFront() async throws {
        let (store, accounts, suite) = await device()
        defer { UserDefaults().removePersistentDomain(forName: suite) }
        let binding = try await enterTheSpace(store, accounts)
        await accounts.saveActiveIndex(0) // the person moved to Ann inside the space

        await binding.apply(environment: Self.leave, store: store, accounts: accounts)

        #expect(await accounts.loadActiveIndex() == 0)
        #expect(await inFront(accounts) == annAddress)
    }

    /// A relaunch inside the space enters again — the space's record goes to
    /// the front — and still knows where the way out leads.
    @Test func aRelaunchInsideTheSpaceKeepsTheWayBack() async throws {
        let (store, accounts, suite) = await device()
        defer { UserDefaults().removePersistentDomain(forName: suite) }
        _ = try await enterTheSpace(store, accounts)

        let relaunched = ParallelSpaceBinding()
        await relaunched.apply(environment: Self.relaunch, store: store, accounts: accounts)
        #expect(relaunched.isActive)
        await relaunched.apply(environment: Self.leave, store: store, accounts: accounts)

        #expect(await inFront(accounts) == boAddress)
    }

    /// Relaunched after switching to Ann inside the space: the relaunch puts
    /// the space's wallet in front again, and Ann — where the person asked to
    /// be — is where leaving goes.
    @Test func aWalletChosenInsideTheSpaceSurvivesARelaunch() async throws {
        let (store, accounts, suite) = await device()
        defer { UserDefaults().removePersistentDomain(forName: suite) }
        _ = try await enterTheSpace(store, accounts)
        await accounts.saveActiveIndex(0)

        let relaunched = ParallelSpaceBinding()
        await relaunched.apply(environment: Self.relaunch, store: store, accounts: accounts)
        await relaunched.apply(environment: Self.leave, store: store, accounts: accounts)

        #expect(await inFront(accounts) == annAddress)
    }

    /// Entered by a build that remembered nothing: the first wallet, as before.
    @Test func withNothingToGoBackToTheFirstWalletIsInFront() async throws {
        let (store, accounts, suite) = await device()
        defer { UserDefaults().removePersistentDomain(forName: suite) }
        let binding = try await enterTheSpace(store, accounts)
        store.writeString(ParallelSpaceBinding.returnToKey, nil)

        await binding.apply(environment: Self.leave, store: store, accounts: accounts)

        #expect(await accounts.loadActiveIndex() == 0)
        #expect(await inFront(accounts) == annAddress)
    }

    /// `VELA_PARALLEL_SPACE=0` on a launch that was never inside: nothing to
    /// remove, and the wallet in front stays — it used to be reset to the first.
    @Test func leavingASpaceNeverEnteredMovesNobody() async throws {
        let (store, accounts, suite) = await device()
        defer { UserDefaults().removePersistentDomain(forName: suite) }

        await ParallelSpaceBinding().apply(environment: Self.leave, store: store, accounts: accounts)

        #expect(await addresses(accounts) == [annAddress, boAddress])
        #expect(await inFront(accounts) == boAddress)
    }

    @Test func theIndexIsFoundByAddressWhateverItsCaseAndAbsentReadsAsZero() {
        let list = [ann, bo]
        #expect(ParallelSpaceBinding.index(of: boAddress.lowercased(), in: list) == 1)
        #expect(ParallelSpaceBinding.index(of: annAddress, in: list) == 0)
        #expect(ParallelSpaceBinding.index(of: "0x6B175474E89094C44Da98b954EedeAC495271d0F", in: list) == 0)
        #expect(ParallelSpaceBinding.index(of: nil, in: list) == 0)
        #expect(ParallelSpaceBinding.index(of: "", in: list) == 0)
        #expect(ParallelSpaceBinding.realAccount(in: list, at: 1, fixtureId: "cred-fixture") == boAddress)
        #expect(ParallelSpaceBinding.realAccount(in: list, at: 1, fixtureId: "cred-2") == nil, "the space's own record")
        #expect(ParallelSpaceBinding.realAccount(in: list, at: 5, fixtureId: "cred-fixture") == nil)
    }
}

#endif
