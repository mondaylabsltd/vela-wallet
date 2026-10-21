//
//  AgentCoverageTests.swift
//  VelaWalletTests
//
//  Every capability the person has is an intent (spec 103) — on the iPhone,
//  kept true by this ledger, as `agent::coverage` keeps it on the desktop and
//  `coverage.test.ts` on the web. It names everything the iPhone's screens do
//  to the wallet's stores: each call a screen makes into a store, the session,
//  the preferences or the agent's neighbours. A new one fails here until it
//  is placed:
//
//  - `intent`  — it commits that catalogued action (an agent can do it too);
//  - `step`    — one step of that action's own sequence;
//  - `view`    — what is shown or how, not what the wallet holds;
//  - `machine` — a machine's input (a tick, a fee, a boot), not the person's;
//  - `none`    — not a capability, and why;
//  - `later`   — a capability not yet offered to an agent, and why: the gaps,
//    in one place, never silent.
//
//  And what the person's own taps ask through the intents (`agent.person`)
//  is listed, so a commit point that stops (or starts) going through them is
//  seen. The screens are `RootView` (where the stores are wired) and the
//  views it draws; the agent's own `perform` is the intents' side and is not
//  scanned.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

private enum Place {
    case intent(String), step(String), view(String), machine(String), none(String), later(String)

    var action: String? {
        switch self {
        case .intent(let action), .step(let action): action
        default: nil
        }
    }

    var reason: String {
        switch self {
        case .intent(let why), .step(let why), .view(let why), .machine(let why), .none(let why), .later(let why): why
        }
    }
}

private let FILE = "a file the person picks or saves on this device"
private let DAPP = "a dApp's own request, answered by the person to that dApp"
private let ONBOARD = "signing in or creating a wallet: the passkey's own ceremony"
private let BOUNDARY = "where this account signs is the signing boundary itself (spec 102)"
private let SWEEP = "several tokens to one address (sweep)"
private let SIGN = "answering a dApp's signing request"
private let BROWSE = "browsing in the in-app dApp browser"
private let CAMERA = "the camera, which an agent has no use for"
private let FEE_TOKEN = "paying the fee in another token, per transfer"
private let SPEED = "choosing one transfer's own speed (the default is set_preference fee_speed)"
private let READ = "a read"
private let BOOT = "boot"
private let FEE = "the fee quote"
private let TRACKER = "the chain tracker"
private let ACCOUNT = "the account in front"
private let FLOW = "a screen opened or left"
private let PICKER = "a picker; the recipient is the step"
private let IMPORTER = "the list importer shown; the list is the step"
private let NOTICE = "a notice dismissed"
private let RETRY = "the review's own retry"

/// The calls the screens make, by `receiver.method`.
private let CALLS: [String: Place] = [
    "accounts.clearSignedInWallet": .step("erase_device"),
    "accounts.first": .machine(READ),
    "accounts.loadAccounts": .machine(READ),
    "accounts.loadServiceEndpoints": .machine(READ),
    "activity.chainFilter": .view("the history filtered by a chain"),
    "activity.contactFilter": .view("a contact's page shown"),
    "activity.focusTick": .step("refresh_balances"),
    "activity.open": .machine(ACCOUNT),
    "activity.privacyChanged": .step("set_balance_privacy"),
    "activity.reconciled": .machine(TRACKER),
    "activity.startLiveTicking": .machine("a tick"),
    "activity.stopLiveTicking": .machine("a tick"),
    "batch.apply": .step("prepare_batch_transfer"),
    "batch.editRate": .step("prepare_batch_transfer"),
    "batch.open": .view(IMPORTER),
    "batch.pickFile": .none(FILE),
    "batch.resetRate": .step("prepare_batch_transfer"),
    "batch.saveTemplate": .none(FILE),
    "batch.setText": .step("prepare_batch_transfer"),
    "batch.setUnit": .step("prepare_batch_transfer"),
    "browser.accountsChanged": .step("switch_account"),
    "browser.addNetworkAnswered": .none(DAPP),
    "browser.answerExternal": .later(BROWSE),
    "browser.appActive": .machine("the app in front or not"),
    "browser.askToOpen": .later(BROWSE),
    "browser.networksChanged": .machine("the networks"),
    "browser.open": .later(BROWSE),
    "browser.revoke": .later("disconnecting one connected site (clear_storage_item dapps disconnects them all)"),
    "browser.revokeAll": .step("clear_storage_item"),
    "browser.setDebugMode": .none("a developer's switch (spec 091), hidden in a release build"),
    "browser.signingAnswered": .none(DAPP),
    "browser.snapshot": .view("a tab's picture for the tab strip"),
    "browser.start": .machine(BOOT),
    "browser.startConnections": .machine(BOOT),
    "camera.flip": .none(CAMERA),
    "camera.start": .none(CAMERA),
    "camera.stop": .none(CAMERA),
    "camera.toggleTorch": .none(CAMERA),
    "contacts.contact": .machine(READ),
    "contacts.exportRequested": .none(FILE),
    "contacts.exportTaken": .none(FILE),
    "contacts.group": .machine(READ),
    "contacts.importAcknowledged": .none(FILE),
    "contacts.importFile": .none(FILE),
    "contacts.inspect": .step("show"),
    "contacts.open": .machine(ACCOUNT),
    "contacts.toggleFavorite": .intent("set_contact_favorite"),
    "deposits.open": .machine("the receive watcher"),
    "documents.pick": .none(FILE),
    "documents.share": .none(FILE),
    "fees.configureSpeed": .machine(FEE),
    "fees.pickSpeed": .later(SPEED),
    "fees.refresh": .machine(FEE),
    "fees.resetSpeed": .machine(FEE),
    "fees.selectAsset": .later(FEE_TOKEN),
    "fees.speedStage": .machine(FEE),
    "fees.toggleSpeed": .view("the speed control unfolded"),
    "fees.view": .machine(READ),
    "flows.back": .view(FLOW),
    "flows.close": .view(FLOW),
    "flows.enter": .view(FLOW),
    "flows.push": .view(FLOW),
    "flows.sheetClosed": .view(FLOW),
    "graph.launch": .machine(BOOT),
    "notifier.askOnceIfNeeded": .none("the notification permission, the platform's own"),
    "notifier.clearPendingReceipt": .view("a tapped notification's record shown"),
    "onboarding.answerInsertKey": .later(ONBOARD),
    "onboarding.consumeFinished": .machine("the account the pages were given"),
    "onboarding.dismissOnboardingSheet": .later(ONBOARD),
    "paymentRequest.includeNetwork": .view("what the receive code says beside the address"),
    "paymentRequest.linkOpened": .machine("a pay link arriving"),
    "paymentRequest.pickAsset": .step("show"),
    "paymentRequest.start": .step("show"),
    "pool.call": .machine(READ),
    "relayClient.clearCaches": .step("erase_device"),
    "send.advance": .step("prepare_transfer"),
    "send.appendSplitRecipients": .step("prepare_batch_transfer"),
    "send.back": .step("cancel_transfer"),
    "send.cancelSigning": .step("cancel_transfer"),
    "send.changeToken": .step("prepare_transfer"),
    "send.chooseFeeToken": .later(FEE_TOKEN),
    "send.closeBatchImport": .view(IMPORTER),
    "send.closeContactPicker": .view(PICKER),
    "send.closeScanner": .none(CAMERA),
    "send.confirmMultiSelection": .later(SWEEP),
    "send.dismissRelayUnreachable": .view(NOTICE),
    "send.dismissTreasurySheet": .view(NOTICE),
    "send.displayStands": .machine("the display currency"),
    "send.done": .view("the receipt closed"),
    "send.enterSplitMode": .step("prepare_batch_transfer"),
    "send.feeBusyChanged": .machine(FEE),
    "send.feeFailedChanged": .machine(FEE),
    "send.feeTokenChanged": .machine(FEE),
    "send.feeUpdated": .machine(FEE),
    "send.handIn": .step("prepare_transfer"),
    "send.holdingsUpdated": .machine("the balances"),
    "send.leave": .machine("the send opening on what it is for"),
    "send.open": .machine("the send opening on what it is for"),
    "send.openBatchImport": .view(IMPORTER),
    "send.openContactPicker": .view(PICKER),
    "send.openScanner": .none(CAMERA),
    "send.pickedAddress": .step("prepare_transfer"),
    "send.recipientsChanged": .step("prepare_batch_transfer"),
    "send.retryAfterBootstrap": .view(RETRY),
    "send.retryAfterError": .view(RETRY),
    "send.retryRelayUnreachable": .view(RETRY),
    "send.scanned": .none("a code handed to the send — the camera's or a pay link's"),
    "send.seedSplitRecipients": .step("prepare_batch_transfer"),
    "send.selectToken": .step("prepare_transfer"),
    "send.setAmount": .step("prepare_transfer"),
    "send.setRecipient": .step("prepare_transfer"),
    "send.slideConfirm": .intent("submit_transfer"),
    "send.sweepTap": .later(SWEEP),
    "send.tapMax": .step("prepare_transfer"),
    "send.toggleFiatInput": .view("typing the amount in fiat or in the token"),
    "session.chooseSigningVenue": .none(BOUNDARY),
    "session.signOut": .step("erase_device"),
    "session.signOutConfirmed": .step("sign_out"),
    "session.signOutDismissed": .step("sign_out"),
    "session.switchAccount": .intent("switch_account"),
    "settings.addSigningPage": .none(BOUNDARY),
    "settings.blurEndpoint": .intent("set_service_endpoint"),
    "settings.blurOverride": .intent("set_network_endpoint"),
    "settings.blurProviderKey": .intent("set_provider_key"),
    "settings.confirmAdd": .intent("add_network"),
    "settings.dappAddApproved": .none(DAPP),
    "settings.dappAddCancelled": .none(DAPP),
    "settings.dappAddDeclined": .none(DAPP),
    "settings.dappAddRequested": .none(DAPP),
    "settings.dappAddRetried": .none(DAPP),
    "settings.editCustomRpc": .step("add_network"),
    "settings.editEndpoint": .step("set_service_endpoint"),
    "settings.editOverride": .step("set_network_endpoint"),
    "settings.editProviderKey": .step("set_provider_key"),
    "settings.expandNetwork": .step("set_network_endpoint"),
    "settings.network": .machine(READ),
    "settings.open": .machine(BOOT),
    "settings.openCurrency": .machine(BOOT),
    "settings.openEndpoints": .step("set_service_endpoint"),
    "settings.openFeeTier": .machine(BOOT),
    "settings.openProviders": .step("set_provider_key"),
    "settings.openSigningPages": .machine(BOOT),
    "settings.recheck": .step("add_network"),
    "settings.removeSigningPage": .none(BOUNDARY),
    "settings.renameSigningPage": .none(BOUNDARY),
    "settings.resetWizard": .step("add_network"),
    "settings.savedSigningPageUrls": .machine(READ),
    "settings.search": .step("add_network"),
    "settings.selectChain": .step("add_network"),
    "settings.testProvider": .later("testing a provider key; its answer is only drawn in Settings"),
    "signing.approve": .later(SIGN),
    "signing.feeTapped": .later(SIGN),
    "signing.guardCustomAmount": .later("limiting a dApp's approval"),
    "signing.guardLegCustomAmount": .later("limiting a dApp's approval"),
    "signing.guardLegPreset": .later("limiting a dApp's approval"),
    "signing.guardPreset": .later("limiting a dApp's approval"),
    "signing.pickFee": .later(SIGN),
    "signing.refreshFee": .later(SIGN),
    "signing.retry": .later(SIGN),
    "signing.speed": .later(SIGN),
    "tokens.input": .step("add_token"),
    "tokens.open": .machine(BOOT),
    "tokens.save": .intent("add_token"),
    "tracker.backgrounded": .machine(TRACKER),
    "tracker.follow": .machine(TRACKER),
    "tracker.foregrounded": .machine(TRACKER),
    "tracker.submitted": .machine(TRACKER),
    "tracker.withdrawn": .machine(TRACKER),
    "trust.simJudged": .machine("the simulation"),
    "wallet.open": .machine(ACCOUNT),
    "wallet.pullRefresh": .intent("refresh_balances"),
    "wallet.refresh": .intent("refresh_balances"),
    "wallet.settled": .machine(READ),
    "wallet.switcherClosed": .view("the account switcher shown"),
    "wallet.switcherOpened": .view("the account switcher shown"),
    "wallet.unreachableListClosed": .view("the networks out of reach listed"),
    "wallet.unreachableListOpened": .view("the networks out of reach listed"),
]

/// What the person's taps ask through the intents (spec 103 D3): the same way
/// in as an agent's, by the same action. The web's and the desktop's list.
private let PERSON: Set<String> = [
    "add_network", "clear_caches", "clear_storage_item", "configure_agent", "delete_activity",
    "delete_contact", "delete_group", "erase_device", "refresh_balances", "remove_network",
    "remove_wallet", "reset_service_endpoints", "save_contact", "save_group", "set_balance_privacy",
    "set_contact_groups", "set_group_members", "set_preference", "sign_out", "submit_transfer",
    "switch_account",
]

/// The stores, the session and the neighbours a screen can reach, by the
/// names `RootView` gives them. The agent itself is the intents' door, read
/// by the person test below.
private let RECEIVERS = [
    "accounts", "activity", "batch", "browser", "camera", "contacts", "deposits", "documents",
    "fees", "flows", "graph", "notifier", "onboarding", "paymentRequest", "pool", "preferences",
    "relayClient", "router", "send", "session", "settings", "signing", "tokens", "tracker", "trust",
    "wallet", "welcome", "identity", "netWatch", "spine", "trustedSigner",
]

@MainActor
struct AgentCoverageTests {

    /// The app's sources, from this test's own path.
    private static let app = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()      // VelaWalletTests
        .deletingLastPathComponent()      // VelaWallet (project dir)
        .appendingPathComponent("VelaWallet")

    /// The screens: `RootView`, the views it draws (a screen, a sheet, a
    /// page's body), and the agent's own views. Not the stores, the
    /// executors, the live builders — the machines' side.
    private static func screens() throws -> [(String, String)] {
        let fm = FileManager.default
        var files = [app.appendingPathComponent("App/RootView.swift")]
        let features = app.appendingPathComponent("Features")
        let walker = fm.enumerator(at: features, includingPropertiesForKeys: nil)
        while let url = walker?.nextObject() as? URL {
            let name = url.lastPathComponent
            guard name.hasSuffix(".swift") else { continue }
            let screen = ["Screen.swift", "Sheet.swift", "Body.swift", "Host.swift", "Page.swift"]
                .contains { name.hasSuffix($0) } || name == "AgentViews.swift"
            if screen { files.append(url) }
        }
        return try files.sorted { $0.path < $1.path }.map { url in
            (url.lastPathComponent, try String(contentsOf: url, encoding: .utf8))
        }
    }

    /// `receiver.method(` — and `receiver?.method(` — each with a file it is in.
    private static func calls() throws -> [String: String] {
        let pattern = try NSRegularExpression(
            pattern: #"(?<![\w.])("# + RECEIVERS.joined(separator: "|") + #")\??\.([a-z]\w*)\("#
        )
        var found: [String: String] = [:]
        for (file, text) in try screens() {
            let range = NSRange(text.startIndex..., in: text)
            for match in pattern.matches(in: text, range: range) {
                guard let receiver = Range(match.range(at: 1), in: text),
                      let method = Range(match.range(at: 2), in: text)
                else { continue }
                let key = "\(text[receiver]).\(text[method])"
                if found[key] == nil { found[key] = file }
            }
        }
        return found
    }

    /// The catalogued actions the screens name as an intent's `type`.
    private static func asked() throws -> Set<String> {
        let catalog = try Self.catalog()
        let pattern = try NSRegularExpression(pattern: #""type":\s*"(\w+)""#)
        var found: Set<String> = []
        for (_, text) in try screens() {
            let range = NSRange(text.startIndex..., in: text)
            for match in pattern.matches(in: text, range: range) {
                guard let kind = Range(match.range(at: 1), in: text) else { continue }
                if catalog.contains(String(text[kind])) { found.insert(String(text[kind])) }
            }
        }
        return found
    }

    private static func catalog() throws -> Set<String> {
        let entries = try #require(try JSONSerialization.jsonObject(with: Data(intentCatalog().utf8)) as? [[String: Any]])
        return Set(entries.compactMap { $0["action"] as? String })
    }

    @Test func placesEveryCallAScreenMakes() throws {
        let found = try Self.calls()
        let unplaced = found.keys.filter { CALLS[$0] == nil }.sorted().map { "\($0) (\(found[$0]!))" }
        #expect(unplaced.isEmpty, "place each in this ledger: \(unplaced)")
        let stale = CALLS.keys.filter { found[$0] == nil }.sorted()
        #expect(stale.isEmpty, "no longer called: drop them: \(stale)")
    }

    @Test func listsWhatThePersonsTapsAskThroughTheIntents() throws {
        #expect(try Self.asked() == PERSON)
    }

    @Test func namesOnlyActionsTheCatalogHas() throws {
        let catalog = try Self.catalog()
        let named = CALLS.values.compactMap(\.action) + Array(PERSON)
        #expect(named.filter { !catalog.contains($0) }.isEmpty)
        // A reason is never empty.
        for (name, place) in CALLS { #expect(!place.reason.isEmpty, "\(name)") }
    }
}
