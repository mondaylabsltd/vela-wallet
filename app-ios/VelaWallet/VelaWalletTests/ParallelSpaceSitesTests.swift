//
//  ParallelSpaceSitesTests.swift
//  VelaWalletTests
//
//  A walk through the parallel space leaves the person's connected sites
//  exactly as they were (device walk 2026-10-11: the owner's Arc Orbs
//  connection was gone after one — and every earlier walk had done the same
//  to whatever was connected).
//
//  The REAL `DappBrowserCore` through the REAL `BrowserController` and
//  `DbrExecutor`, the REAL `ParallelSpaceBinding` with the build's fixture
//  keyset, and one defaults suite standing for one device: the account list,
//  the space's flag and every `vela.perm.*` / `vela.chain.*` key live in it
//  together, as they do on a phone. The page is the harness's fake one — the
//  messages WebKit would have reported.
//
//  The space is entered and left the way the app does it: at a LAUNCH, through
//  the `VELA_PARALLEL_SPACE` pin, before anything reads the account list. So a
//  launch here is a new `BrowserController` over the same storage, told what
//  the stores then hold. `space` is never passed by a test: the controller
//  decides it, through `ParallelSpaceHook` and the binding the app installs.
//
//  "Nothing was written" is counted, not inferred from the values: the
//  storage is a `UserDefaults` that logs every set and every removal with its
//  key, so a grant rewritten with the same bytes — or re-pinned and pinned
//  back — would be seen.
//

#if DEBUG

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

/// A defaults suite that remembers every write made through it.
private final class RecordingDefaults: UserDefaults {
    struct Write: Equatable {
        let key: String
        let removed: Bool
    }

    private(set) var writes: [Write] = []

    override func set(_ value: Any?, forKey defaultName: String) {
        writes.append(Write(key: defaultName, removed: value == nil))
        super.set(value, forKey: defaultName)
    }

    override func set(_ value: Int, forKey defaultName: String) {
        writes.append(Write(key: defaultName, removed: false))
        super.set(value, forKey: defaultName)
    }

    override func removeObject(forKey defaultName: String) {
        writes.append(Write(key: defaultName, removed: true))
        super.removeObject(forKey: defaultName)
    }

    /// The writes that touch a connected site: its grant or its network.
    var siteWrites: [Write] {
        writes.filter { $0.key.hasPrefix("vela.perm.") || $0.key.hasPrefix("vela.chain.") }
    }
}

private let orbs = "https://arcorbs.example"
private let swap = "https://swap.example"
private let fresh = "https://never-met.example"
private let t0: Double = 1_757_000_000_000

/// An address as every page is given it: the core's one spelling (EIP-55).
private func spelled(_ address: String) -> String {
    (try? checksumAddress(addressHex: address)) ?? address
}

@MainActor
@Suite(.hangLimit)
struct ParallelSpaceSitesTests {

    private static let ann: [String: Any] = [
        "id": "cred-1", "name": "Ann", "address": "0x1F9840a85d5aF5bf1D1762F925BDADdC4201F984",
        "public_key_hex": "04ab", "created_at_iso": "2026-08-25T10:00:00.000Z",
        "keys": [["credential_id": "cred-1", "public_key_hex": "04ab", "name": "Ann"]],
    ]
    private static let bo: [String: Any] = [
        "id": "cred-2", "name": "Bo", "address": "0x7687ab000000000000000000000000000000d141",
        "public_key_hex": "04cd", "created_at_iso": "2026-08-26T10:00:00.000Z",
        "keys": [["credential_id": "cred-2", "public_key_hex": "04cd", "name": "Bo"]],
    ]
    private static var annAddress: String { ann["address"] as! String }
    private static var boAddress: String { bo["address"] as! String }

    private static let enter = ["VELA_PARALLEL_SPACE": "1"]
    private static let leave = ["VELA_PARALLEL_SPACE": "0"]

    /// One device: its storage, and the two stores the app reads it through.
    @MainActor
    private struct Device {
        let suite: String
        let defaults: RecordingDefaults
        let store: VelaStore
        let accounts: AccountStore

        /// Ann and Bo, Bo in front; Arc Orbs connected to Bo on Gnosis, a
        /// second site connected to Bo on Base — each with the network the
        /// site was last on.
        static func make() async -> Device {
            let suite = "parallel-sites-\(UUID().uuidString)"
            UserDefaults().removePersistentDomain(forName: suite)
            let defaults = RecordingDefaults(suiteName: suite)!
            let device = Device(
                suite: suite, defaults: defaults,
                store: VelaStore(defaults: defaults), accounts: AccountStore(defaults: defaults)
            )
            await device.accounts.saveAccount(ParallelSpaceSitesTests.ann)
            await device.accounts.saveAccount(ParallelSpaceSitesTests.bo)
            await device.accounts.saveActiveIndex(1)
            let bo = spelled(ParallelSpaceSitesTests.boAddress)
            device.store.writeString("vela.perm.\(orbs)", BrowserHarness.grant(orbs, bo, chain: 100))
            device.store.writeString("vela.chain.\(orbs)", "100")
            device.store.writeString("vela.perm.\(swap)", BrowserHarness.grant(swap, bo, chain: 8453))
            device.store.writeString("vela.chain.\(swap)", "8453")
            return device
        }

        func erase() { UserDefaults().removePersistentDomain(forName: suite) }

        /// Every connected site's stored text, by key — what "byte for byte"
        /// is compared on.
        var sites: [String: String] {
            var out: [String: String] = [:]
            for key in store.allKeys() where key.hasPrefix("vela.perm.") || key.hasPrefix("vela.chain.") {
                out[key] = store.rawValue(key)
            }
            return out
        }

        func addresses() async -> [String] {
            await accounts.loadAccounts().compactMap { $0["address"] as? String }
        }

        func inFront() async -> String? {
            let list = await accounts.loadAccounts()
            let index = await accounts.loadActiveIndex()
            return list.indices.contains(index) ? list[index]["address"] as? String : nil
        }

        /// A launch: a new browser over this device's storage, told the
        /// accounts the stores hold — what `RootView` tells it.
        func launch() async -> BrowserHarness {
            let harness = BrowserHarness(defaults: defaults)
            harness.browser.startConnections()
            harness.browser.networksChanged([1, 100, 8453])
            harness.browser.accountsChanged(addresses: await addresses(), active: await inFront())
            await harness.until { harness.browser.dbr.ready }
            await harness.settle()
            return harness
        }
    }

    private static func fixtureAddress() throws -> String {
        spelled(try fixtureMultiAddress())
    }

    /// What the page at `origin` is told its accounts are, asked now.
    private func accounts(_ h: BrowserHarness, tab: String, origin: String, id: String) async -> [String]? {
        await h.ask(tab, doc: "doc-\(tab)", origin: origin, id: id, method: "eth_accounts")
        await h.until { h.answer(tab, id: id) != nil }
        return h.answer(tab, id: id)?.result as? [String]
    }

    private func chain(_ h: BrowserHarness, tab: String, origin: String, id: String) async -> String? {
        await h.ask(tab, doc: "doc-\(tab)", origin: origin, id: id, method: "eth_chainId")
        await h.until { h.answer(tab, id: id) != nil }
        return h.answer(tab, id: id)?.result as? String
    }

    /// Connect from the page and say yes on the sheet.
    private func connect(_ h: BrowserHarness, tab: String, origin: String, id: String) async -> [String]? {
        await h.ask(tab, doc: "doc-\(tab)", origin: origin, id: id, method: "eth_requestAccounts")
        await h.until { h.browser.dbr.consent != nil || h.answer(tab, id: id) != nil }
        #expect(h.browser.dbr.consent?.origin == origin, "the site is asked for: nothing connects it in silence")
        h.browser.consentApproved()
        await h.until { h.answer(tab, id: id) != nil }
        await h.settle()
        return h.answer(tab, id: id)?.result as? [String]
    }

    // MARK: - The walk, as the app makes it: three launches

    /// A grant stored for a real account → enter → connect inside, the same
    /// site and one never met, and change a site's network → leave. The
    /// person's sites are byte for byte what they were after every step, and
    /// no site key was written at any point.
    @Test func aWalkThroughTheSpaceLeavesEverySiteKeyUntouched() async throws {
        let device = await Device.make()
        defer { device.erase() }
        let bo = spelled(Self.boAddress)
        let fixture = try Self.fixtureAddress()

        // Outside, before: the person's own wallet, their site connected.
        let before = await device.launch()
        await before.hello("t1", doc: "doc-t1", origin: orbs)
        #expect(await accounts(before, tab: "t1", origin: orbs, id: "a") == [bo])
        #expect(await chain(before, tab: "t1", origin: orbs, id: "c") == "0x64")
        let stored = device.sites
        #expect(stored.count == 4)
        #expect(device.defaults.siteWrites.count == 4, "the four keys this test seeded, and nothing since")
        let seeded = device.defaults.siteWrites

        // Into the space — the app's own door, at a launch.
        let binding = ParallelSpaceBinding()
        await binding.apply(environment: Self.enter, store: device.store, accounts: device.accounts)
        #expect(binding.isActive)
        #expect(await device.inFront()?.caseInsensitiveCompare(fixture) == .orderedSame)
        #expect(await device.addresses().count == 3, "the space's record is added beside the person's")

        let inside = await device.launch()
        #expect(device.sites == stored, "entering wrote no site")
        #expect(device.defaults.siteWrites == seeded)
        #expect(inside.browser.dbr.sites.isEmpty, "the space starts with no connected site")

        // The same site, inside: not connected there until it is connected THERE.
        await inside.hello("t1", doc: "doc-t1", origin: orbs)
        #expect(await accounts(inside, tab: "t1", origin: orbs, id: "a") == [])
        #expect(await chain(inside, tab: "t1", origin: orbs, id: "c") == "0x64", "a site keeps the network it was on")
        #expect(await connect(inside, tab: "t1", origin: orbs, id: "r") == [fixture])
        #expect(device.sites == stored, "connecting the person's site inside the space wrote nothing over it")
        #expect(device.defaults.siteWrites == seeded)

        // A network change inside stays inside.
        await inside.ask("t1", doc: "doc-t1", origin: orbs, id: "s", method: "wallet_switchEthereumChain",
                         params: [["chainId": "0x2105"]])
        await inside.until { inside.answer("t1", id: "s") != nil }
        await inside.settle()
        #expect(inside.answer("t1", id: "s")?.errorCode == nil)
        #expect(await chain(inside, tab: "t1", origin: orbs, id: "c2") == "0x2105")
        #expect(device.sites == stored)
        #expect(device.defaults.siteWrites == seeded)

        // A site the person never met, connected inside.
        await inside.hello("t2", doc: "doc-t2", origin: fresh)
        #expect(await connect(inside, tab: "t2", origin: fresh, id: "r") == [fixture])
        #expect(inside.browser.dbr.sites.map(\.origin).sorted() == [orbs, fresh].sorted(),
                "the space's own sites are listed while it is in front")
        #expect(device.sites == stored, "a site connected inside the space is not stored")
        #expect(device.defaults.siteWrites == seeded)
        #expect(inside.records.count == 2, "the test wallet's own activity rows are still written")
        #expect(inside.records.allSatisfy { ($0["from"] as? String)?.caseInsensitiveCompare(fixture) == .orderedSame })

        // Out again — the app's own door, at the next launch.
        await binding.apply(environment: Self.leave, store: device.store, accounts: device.accounts)
        #expect(await device.addresses() == [Self.annAddress, Self.boAddress], "only the space's record went")
        #expect(await device.inFront() == Self.boAddress)

        let after = await device.launch()
        #expect(device.sites == stored, "the person's sites are byte for byte what they were")
        #expect(device.defaults.siteWrites == seeded, "and no site key was written during the whole walk")
        #expect(after.browser.dbr.sites.map(\.origin).sorted() == [orbs, swap].sorted())
        await after.hello("t1", doc: "doc-t1", origin: orbs)
        #expect(await accounts(after, tab: "t1", origin: orbs, id: "a") == [bo], "Arc Orbs is still connected to Bo")
        #expect(await chain(after, tab: "t1", origin: orbs, id: "c") == "0x64", "on the network it was on")
        await after.hello("t2", doc: "doc-t2", origin: fresh)
        #expect(await accounts(after, tab: "t2", origin: fresh, id: "a") == [],
                "what was connected in the space ended with it")

        // The log is live: the account list and the space's flag WERE written.
        #expect(device.defaults.writes.contains { $0.key == VelaStore.Key.accounts })
        #expect(device.defaults.writes.contains { $0.key == ParallelSpaceBinding.flagKey })
    }

    // MARK: - The same walk inside one run, whichever event arrives first

    /// The browser machine hears two events when the accounts change: the
    /// list, and the account in front. The loss came from their order on the
    /// way out — the shorter list arrived while every site was pinned to the
    /// test wallet. The space's wallet arriving and going inside ONE run (the
    /// account switcher inside the space does exactly this) must leave the
    /// store alone in both orders.
    @Test(arguments: [false, true])
    func theTestWalletComingAndGoingWritesNothingInEitherOrder(switchFirst: Bool) async throws {
        let device = await Device.make()
        defer { device.erase() }
        let bo = spelled(Self.boAddress)
        let fixture = try Self.fixtureAddress()
        let h = await device.launch()
        let stored = device.sites
        let seeded = device.defaults.siteWrites
        await h.hello("t1", doc: "doc-t1", origin: orbs)

        /// What `accountsChanged` says, in this test's order. `space` is the
        /// controller's own answer for the account, not this test's.
        func tell(addresses: [String], active: String) async {
            let events = BrowserController.accountEvents(
                addresses: addresses, active: active, nowMs: t0,
                isSpaceAccount: { ParallelSpaceHook.isSpaceAccount($0) }
            )
            h.browser.eventsForTesting(switchFirst ? events.reversed() : events)
            await h.settle()
        }

        // The test wallet joins the list and comes to the front.
        await tell(addresses: [Self.annAddress, Self.boAddress, fixture], active: fixture)
        #expect(h.events("t1").last?.event == "accountsChanged")
        #expect(h.events("t1").last?.message["data"] as? [String] == [], "the open page is no longer Bo's")
        #expect(device.sites == stored)
        #expect(device.defaults.siteWrites == seeded)

        #expect(await connect(h, tab: "t1", origin: orbs, id: "r") == [fixture])
        await h.hello("t2", doc: "doc-t2", origin: fresh)
        #expect(await connect(h, tab: "t2", origin: fresh, id: "r") == [fixture])
        #expect(device.sites == stored)
        #expect(device.defaults.siteWrites == seeded)

        // Its record goes and Bo is in front again.
        await tell(addresses: [Self.annAddress, Self.boAddress], active: Self.boAddress)
        #expect(device.sites == stored, "the person's sites are byte for byte what they were")
        #expect(device.defaults.siteWrites == seeded, "and no site key was written on the way in, inside, or on the way out")
        #expect(h.events("t1").last?.event == "accountsChanged")
        #expect(h.events("t1").last?.message["data"] as? [String] == [bo], "Arc Orbs' page is told it is Bo's again")
        #expect(h.events("t2").contains { $0.event == "accountsChanged" && $0.message["data"] as? [String] == [] },
                "the page of the site met in the space is told it is nobody's")
        #expect(await accounts(h, tab: "t1", origin: orbs, id: "z1") == [bo])
        #expect(await accounts(h, tab: "t2", origin: fresh, id: "z2") == [])
        #expect(h.browser.dbr.sites.map(\.origin).sorted() == [orbs, swap].sorted())
        #expect(device.sites == stored)
        #expect(device.defaults.siteWrites == seeded)
    }

    // MARK: - Whose account it is, not which launch it is

    /// Inside the space the person can switch to one of their own wallets and
    /// back. Their sites are theirs while their wallet is in front, and set
    /// aside again when the test wallet returns — the space's flag does not
    /// move at any of these.
    @Test func insideTheSpaceThePersonsOwnWalletStillHasItsSites() async throws {
        let device = await Device.make()
        defer { device.erase() }
        let bo = spelled(Self.boAddress)
        let fixture = try Self.fixtureAddress()
        let binding = ParallelSpaceBinding()
        await binding.apply(environment: Self.enter, store: device.store, accounts: device.accounts)
        let h = await device.launch()
        let stored = device.sites
        let seeded = device.defaults.siteWrites
        let all = await device.addresses()
        await h.hello("t1", doc: "doc-t1", origin: orbs)
        #expect(await accounts(h, tab: "t1", origin: orbs, id: "a1") == [])

        // The switcher, to Bo — the account Arc Orbs was connected to.
        h.browser.accountsChanged(addresses: all, active: Self.boAddress)
        await h.settle()
        #expect(binding.isActive, "still inside the space")
        #expect(await accounts(h, tab: "t1", origin: orbs, id: "a2") == [bo])
        #expect(device.sites == stored)
        #expect(device.defaults.siteWrites == seeded, "back on the account the sites were pinned to: nothing to write")

        // And back to the test wallet.
        h.browser.accountsChanged(addresses: all, active: fixture)
        await h.settle()
        #expect(await accounts(h, tab: "t1", origin: orbs, id: "a3") == [])
        #expect(device.sites == stored)
        #expect(device.defaults.siteWrites == seeded)
    }

    /// `space` is true for the test wallet's address — however it is cased,
    /// and whether or not a space was entered at this launch — and for no
    /// account of the person's, inside the space or out of it.
    @Test func spaceIsSaidOfTheTestWalletAndOfNoOtherAccount() async throws {
        let fixture = try fixtureMultiAddress()

        func said(_ active: String?, by isSpaceAccount: @MainActor (String) -> Bool) -> Bool? {
            BrowserController.accountEvents(
                addresses: [Self.annAddress], active: active, nowMs: t0, isSpaceAccount: isSpaceAccount
            ).first { $0["type"] as? String == "account_switched" }?["space"] as? Bool
        }

        // The app's own hook, as the controller asks it.
        let hook: @MainActor (String) -> Bool = { ParallelSpaceHook.isSpaceAccount($0) }
        #expect(said(fixture, by: hook) == true)
        #expect(said(fixture.lowercased(), by: hook) == true)
        #expect(said("0x" + fixture.dropFirst(2).uppercased(), by: hook) == true)
        #expect(said(Self.annAddress, by: hook) == false)
        #expect(said(Self.boAddress, by: hook) == false)
        #expect(said(nil, by: hook) == nil, "with nobody in front there is no switch to tell")
        #expect(!ParallelSpaceHook.isSpaceAccount(nil))
        #expect(!ParallelSpaceHook.isSpaceAccount(""))

        // A binding that never entered, and one that did: the same answers.
        let outside = ParallelSpaceBinding()
        #expect(!outside.isActive)
        #expect(outside.isSpaceAccount(fixture))
        #expect(!outside.isSpaceAccount(Self.boAddress))

        let device = await Device.make()
        defer { device.erase() }
        let inside = ParallelSpaceBinding()
        await inside.apply(environment: Self.enter, store: device.store, accounts: device.accounts)
        #expect(inside.isActive)
        #expect(inside.isSpaceAccount(fixture))
        #expect(!inside.isSpaceAccount(Self.boAddress), "a wallet of the person's is theirs inside the space too")
        #expect(!inside.isSpaceAccount(""))

        // The list is always said first, and it is the whole list.
        let events = BrowserController.accountEvents(
            addresses: [Self.annAddress, fixture], active: fixture, nowMs: t0, isSpaceAccount: hook
        )
        #expect(events.map { $0["type"] as? String } == ["accounts_updated", "account_switched"])
        #expect(events.first?["addresses"] as? [String] == [Self.annAddress, fixture])
        #expect(events.last?["address"] as? String == fixture)
    }
}

#endif
