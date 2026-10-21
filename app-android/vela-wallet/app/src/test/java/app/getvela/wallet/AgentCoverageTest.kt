package app.getvela.wallet

import java.io.File
import org.json.JSONArray
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.intentCatalog

/**
 * Every capability the person has is an intent (spec 103) — on Android, kept
 * true by this ledger, as `agent::coverage` keeps it on the desktop,
 * `AgentCoverageTests` on the iPhone and `coverage.test.ts` on the web. It
 * names everything Android's screens do to the wallet's controllers: each
 * call a screen makes into a controller, the session, the preferences, the
 * container or the agent's neighbours. A new one fails here until it is
 * placed:
 *
 * - `Intent`  — it commits that catalogued action (an agent can do it too);
 * - `Step`    — one step of that action's own sequence;
 * - `View`    — what is shown or how, not what the wallet holds;
 * - `Machine` — a machine's input (a tick, a fee, a boot), not the person's;
 * - `None`    — not a capability, and why;
 * - `Later`   — a capability not yet offered to an agent, and why: the gaps,
 *   in one place, never silent.
 *
 * And what the person's own taps ask through the intents (`agent.person`) is
 * listed, so a commit point that stops (or starts) going through them is
 * seen. The screens are the navigation host (where the controllers are
 * wired), the activity, the screens it draws and the agent's own views; the
 * agent's `perform` is the intents' side and is not scanned.
 */
class AgentCoverageTest {

    private sealed class Place(val why: String) {
        class Intent(action: String) : Place(action)
        class Step(action: String) : Place(action)
        class View(why: String) : Place(why)
        class Machine(why: String) : Place(why)
        class None(why: String) : Place(why)
        class Later(why: String) : Place(why)

        val action: String? get() = if (this is Intent || this is Step) why else null
    }

    private companion object {
        const val FILE = "a file the person picks or saves on this device"
        const val DAPP = "a dApp's own request, answered by the person to that dApp"
        const val ONBOARD = "signing in or creating a wallet: the passkey's own ceremony"
        const val BOUNDARY = "where this account signs is the signing boundary itself (spec 102)"
        const val SWEEP = "several tokens to one address (sweep)"
        const val SIGN = "answering a dApp's signing request"
        const val GUARD = "limiting a dApp's approval"
        const val BROWSE = "browsing in the in-app dApp browser"
        const val CAMERA = "the camera, which an agent has no use for"
        const val CODE = "a code handed to the send — the camera's or a pay link's"
        const val FEE_TOKEN = "paying the fee in another token, per transfer"
        const val DEBUG = "a developer's switch (spec 091), hidden in a release build"
        const val ONE_SITE = "disconnecting one connected site (clear_storage_item dapps disconnects them all)"
        const val READ = "a read"
        const val BOOT = "boot"
        const val FEE = "the fee quote"
        const val ACCOUNT = "the account in front"
        const val FLOW = "a screen opened or left"
        const val PICKER = "a picker; the recipient is the step"
        const val IMPORTER = "the list importer shown; the list is the step"
        const val NOTICE = "a notice dismissed"
        const val RETRY = "the review's own retry"
        const val OPENING = "the send opening on what it is for"
        const val REPORT = "a report to the wallet's makers"

        /** The calls the screens make, by `receiver.method`. */
        val CALLS: Map<String, Place> = mapOf(
            "browser.askChain" to Place.Later(BROWSE),
            "browser.askToOpen" to Place.Later(BROWSE),
            "browser.back" to Place.Later(BROWSE),
            "browser.clearRecent" to Place.Later(BROWSE),
            "browser.close" to Place.Later(BROWSE),
            "browser.closeAllTabs" to Place.Later(BROWSE),
            "browser.closeOtherTabs" to Place.Later(BROWSE),
            "browser.closeTab" to Place.Later(BROWSE),
            "browser.closeTabsToRight" to Place.Later(BROWSE),
            "browser.closedBy" to Place.Machine(READ),
            "browser.consentApproved" to Place.None(DAPP),
            "browser.consentRejected" to Place.None(DAPP),
            "browser.debugCrashRenderer" to Place.None(DEBUG),
            "browser.forward" to Place.Later(BROWSE),
            "browser.inspectorClosed" to Place.View("a site's connection shown"),
            "browser.landedHome" to Place.Later(BROWSE),
            "browser.networkCameBack" to Place.Machine("the network back"),
            "browser.newTab" to Place.Later(BROWSE),
            "browser.open" to Place.Later(BROWSE),
            "browser.putStatusAway" to Place.View("a page's status line seen"),
            "browser.reload" to Place.Later(BROWSE),
            "browser.revoke" to Place.Later(ONE_SITE),
            "browser.selectTab" to Place.Later(BROWSE),
            "browser.setSystemGroupHidden" to Place.Later(BROWSE),
            "browser.start" to Place.Machine(BOOT),
            "browser.stop" to Place.Later(BROWSE),
            "browser.toggleFavorite" to Place.Later(BROWSE),
            "browser.waitingTab" to Place.Machine(READ),
            "contacts.acknowledgeImport" to Place.None(FILE),
            "contacts.exportBook" to Place.None(FILE),
            "contacts.importBook" to Place.None(FILE),
            "contacts.inspect" to Place.Step("show"),
            "contacts.open" to Place.Machine(ACCOUNT),
            "contacts.toggleFavourite" to Place.Intent("set_contact_favorite"),
            "container.applyLanguage" to Place.Machine(BOOT),
            "container.clearStorageItem" to Place.Later(ONE_SITE),
            "container.deviceKeysOf" to Place.Machine(READ),
            "container.foundingKeyOf" to Place.Machine(READ),
            "container.openEthereumBackup" to Place.Later(
                "copying the wallet's keys to Ethereum's registry: a transaction the person signs on the signing sheet",
            ),
            "container.refreshSignerPages" to Place.Machine("the signing pages checked again"),
            "container.signInCredentialOf" to Place.Machine(READ),
            "controller.approve" to Place.Later(SIGN),
            "controller.feeTapped" to Place.Later(SIGN),
            "controller.guardCustomAmount" to Place.Later(GUARD),
            "controller.guardLegCustomAmount" to Place.Later(GUARD),
            "controller.guardLegPreset" to Place.Later(GUARD),
            "controller.pickFee" to Place.Later(SIGN),
            "controller.refreshFee" to Place.Later(SIGN),
            "controller.retry" to Place.Later(SIGN),
            "controller.swipeDismissed" to Place.Later(SIGN),
            "controller.toggleSpeed" to Place.Later(SIGN),
            "feedback.noticeShown" to Place.None(REPORT),
            "feedback.sheetClosed" to Place.None(REPORT),
            "feedback.sheetOpened" to Place.None(REPORT),
            "feedback.submit" to Place.None(REPORT),
            "flows.back" to Place.View(FLOW),
            "flows.close" to Place.View(FLOW),
            "flows.enter" to Place.View(FLOW),
            "flows.push" to Place.View(FLOW),
            "flows.sheetClosed" to Place.View(FLOW),
            "onboarding.attach" to Place.Machine(BOOT),
            "onboarding.beginSignIn" to Place.Later(ONBOARD),
            "onboarding.consumeFinished" to Place.Machine("the account the pages were given"),
            "onboarding.disposeCreate" to Place.Later(ONBOARD),
            "onboarding.signer" to Place.Machine("the passkey's ceremony"),
            "paymentHandOff.request" to Place.Machine("a pay link arriving"),
            "paymentHandOff.scan" to Place.None(CODE),
            "paymentHandOff.takeRequest" to Place.Machine(OPENING),
            "paymentHandOff.takeScan" to Place.None(CODE),
            "preferences.revealDebugMode" to Place.None(DEBUG),
            "preferences.setDebugMode" to Place.None(DEBUG),
            "relay.probeTreasury" to Place.Machine(READ),
            "send.appendSplit" to Place.Step("prepare_batch_transfer"),
            "send.back" to Place.Step("cancel_transfer"),
            "send.batchApply" to Place.Step("prepare_batch_transfer"),
            "send.batchFiatCode" to Place.Step("prepare_batch_transfer"),
            "send.batchPickFile" to Place.None(FILE),
            "send.batchRate" to Place.Step("prepare_batch_transfer"),
            "send.batchResetRate" to Place.Step("prepare_batch_transfer"),
            "send.batchTemplate" to Place.None(FILE),
            "send.batchText" to Place.Step("prepare_batch_transfer"),
            "send.batchUnit" to Place.Step("prepare_batch_transfer"),
            "send.cancelSigning" to Place.Step("cancel_transfer"),
            "send.cancelSweep" to Place.Later(SWEEP),
            "send.changeToken" to Place.Step("prepare_transfer"),
            "send.chooseFeeToken" to Place.Later(FEE_TOKEN),
            "send.closeBatch" to Place.View(IMPORTER),
            "send.closeContactPicker" to Place.View(PICKER),
            "send.closeScanner" to Place.None(CAMERA),
            "send.confirmSweep" to Place.Later(SWEEP),
            "send.continueTapped" to Place.Step("prepare_transfer"),
            "send.dismissRelayUnreachable" to Place.View(NOTICE),
            "send.dismissTreasurySheet" to Place.View(NOTICE),
            "send.displayChanged" to Place.Machine("the display currency"),
            "send.done" to Place.View("the receipt closed"),
            "send.enterSplit" to Place.Step("prepare_batch_transfer"),
            "send.feeTapped" to Place.Later(FEE_TOKEN),
            "send.handoffFor" to Place.Machine(READ),
            "send.left" to Place.Machine(OPENING),
            "send.open" to Place.Machine(OPENING),
            "send.openBatch" to Place.View(IMPORTER),
            "send.openContactPicker" to Place.View(PICKER),
            "send.openRowPicker" to Place.View(PICKER),
            "send.openRowScanner" to Place.None(CAMERA),
            "send.openScanner" to Place.None(CAMERA),
            "send.pickedAddress" to Place.Step("prepare_transfer"),
            "send.refreshFee" to Place.Machine(FEE),
            "send.retryAfterBootstrap" to Place.View(RETRY),
            "send.retryAfterError" to Place.View(RETRY),
            "send.retryRelayUnreachable" to Place.View(RETRY),
            "send.scanned" to Place.None(CODE),
            "send.seedSplit" to Place.Step("prepare_batch_transfer"),
            "send.selectAllValuable" to Place.Later(SWEEP),
            "send.selectToken" to Place.Step("prepare_transfer"),
            "send.setAmount" to Place.Step("prepare_transfer"),
            "send.setRecipient" to Place.Step("prepare_transfer"),
            "send.slideConfirm" to Place.Intent("submit_transfer"),
            "send.splitAdd" to Place.Step("prepare_batch_transfer"),
            "send.splitAddress" to Place.Step("prepare_batch_transfer"),
            "send.splitAmount" to Place.Step("prepare_batch_transfer"),
            "send.splitFillEmpty" to Place.Step("prepare_batch_transfer"),
            "send.splitRemove" to Place.Step("prepare_batch_transfer"),
            "send.startSweep" to Place.Later(SWEEP),
            "send.tapMax" to Place.Step("prepare_transfer"),
            "send.toggleFiatInput" to Place.View("typing the amount in fiat or in the token"),
            "send.toggleImportReplaces" to Place.Step("prepare_batch_transfer"),
            "send.toggleSpeed" to Place.View("the speed control unfolded"),
            "send.toggleSweep" to Place.Later(SWEEP),
            "session.chooseSigningVenue" to Place.None(BOUNDARY),
            "session.signOutConfirmed" to Place.Step("sign_out"),
            "session.signOutDismissed" to Place.Step("sign_out"),
            "settings.addSigningPage" to Place.None(BOUNDARY),
            "settings.commitEndpoint" to Place.Intent("set_service_endpoint"),
            "settings.commitOverride" to Place.Intent("set_network_endpoint"),
            "settings.commitProviderKey" to Place.Intent("set_provider_key"),
            "settings.confirmAddNetwork" to Place.Intent("add_network"),
            "settings.editCustomRpc" to Place.Step("add_network"),
            "settings.editEndpoint" to Place.Step("set_service_endpoint"),
            "settings.editOverride" to Place.Step("set_network_endpoint"),
            "settings.editProviderKey" to Place.Step("set_provider_key"),
            "settings.expandOverride" to Place.Step("set_network_endpoint"),
            "settings.openEndpoints" to Place.Step("set_service_endpoint"),
            "settings.openProviders" to Place.Step("set_provider_key"),
            "settings.refreshCurrency" to Place.Machine(BOOT),
            "settings.refreshFeeTier" to Place.Machine(BOOT),
            "settings.refreshSigningPages" to Place.Machine(BOOT),
            "settings.removeSigningPage" to Place.None(BOUNDARY),
            "settings.renameSigningPage" to Place.None(BOUNDARY),
            "settings.resetWizard" to Place.Step("add_network"),
            "settings.searchNetworks" to Place.Step("add_network"),
            "settings.selectChain" to Place.Step("add_network"),
            "settings.startNetworks" to Place.Machine(BOOT),
            "settings.testProvider" to Place.Later("testing a provider key; its answer is only drawn in Settings"),
            "themeRepository.setThemePreference" to Place.Intent("set_preference"),
            "trustedSigner.open" to Place.Step("submit_transfer"),
            "trustedSigner.personReturned" to Place.Machine("the person back from the signing page"),
            "wallet.addTokenInput" to Place.Step("add_token"),
            "wallet.addTokenSave" to Place.Intent("add_token"),
            "wallet.assetPicked" to Place.Step("show"),
            "wallet.filterChain" to Place.View("the history filtered by a chain"),
            "wallet.filterContact" to Place.View("a contact's page shown"),
            "wallet.fixChainResolved" to Place.Machine("a network's fix answered"),
            "wallet.includeNetwork" to Place.View("what the receive code says beside the address"),
            "wallet.open" to Place.Machine(ACCOUNT),
            "wallet.openAddToken" to Place.Machine(BOOT),
            "wallet.openReceive" to Place.Step("show"),
            "wallet.refresh" to Place.Intent("refresh_balances"),
            "wallet.storedRequest" to Place.Machine(READ),
            "wallet.switcherClosed" to Place.View("the account switcher shown"),
            "wallet.switcherOpened" to Place.View("the account switcher shown"),
            "wallet.unreachableListClosed" to Place.View("the networks out of reach listed"),
            "wallet.unreachableListOpened" to Place.View("the networks out of reach listed"),
            "wallet.validatePayLink" to Place.Machine("a pay link arriving"),
            "walletKeys.deviceOnly" to Place.Machine(READ),
            "walletKeys.read" to Place.Machine(READ),
        )

        /** What the person's taps ask through the intents (spec 103 D3): the web's and the desktop's list. */
        val PERSON = setOf(
            "add_network", "clear_caches", "clear_storage_item", "configure_agent", "delete_activity",
            "delete_contact", "delete_group", "erase_device", "refresh_balances", "remove_network",
            "remove_wallet", "reset_service_endpoints", "save_contact", "save_group", "set_balance_privacy",
            "set_contact_groups", "set_group_members", "set_preference", "sign_out", "submit_transfer",
            "switch_account",
        )

        /**
         * The controllers, the session and the neighbours a screen can reach, by
         * the names the screens give them. The agent itself is the intents' door,
         * read by the person test below.
         */
        val RECEIVERS = listOf(
            "browser", "contacts", "controller", "feedback", "flows", "onboarding", "paymentHandOff",
            "pool", "preferences", "prefsStore", "relay", "send", "session", "settings", "signing",
            "signingController", "themeRepository", "trustedSigner", "wallet", "walletKeys",
        )

        /** Reading a flow is not a call into it. */
        val READS = setOf("collectAsStateWithLifecycle", "collectAsState", "addressAt")

        val root = File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle"))
        val main = File(root, "app-android/vela-wallet/app/src/main/java/app/getvela/wallet")
    }

    /**
     * The screens: the navigation host, the activity, the views it draws (a
     * screen, a sheet, a route, a page's body) and the agent's own views. Not
     * the controllers, the executors, the live builders — the machines' side —
     * nor the galleries' fixtures.
     */
    private fun screens(): List<Pair<String, String>> {
        val files = mutableListOf(File(main, "navigation/VelaNavHost.kt"), File(main, "MainActivity.kt"))
        File(main, "feature").walkTopDown()
            .filter { it.isFile && it.extension == "kt" && "Gallery" !in it.name }
            .filter { file ->
                listOf("Screen.kt", "Sheet.kt", "Route.kt", "Body.kt", "Page.kt").any { file.name.endsWith(it) } ||
                    file.name == "AgentViews.kt"
            }
            .forEach(files::add)
        return files.sortedBy { it.path }.map { it.name to it.readText() }
    }

    /** `receiver.method(`, `application.container.receiver.method(` and `container.method(`. */
    private fun calls(): Map<String, String> {
        val receiver = Regex(
            """(?<![\w.])(?:application\.container\.|container\.)?(${RECEIVERS.joinToString("|")})\??\.([a-z]\w*)\(""",
        )
        val container = Regex("""(?<![\w.])(?:application\.)?container\.([a-z]\w*)\(""")
        val found = sortedMapOf<String, String>()
        for ((file, text) in screens()) {
            receiver.findAll(text).forEach { match ->
                val (name, method) = match.destructured
                if (method !in READS) found.putIfAbsent("$name.$method", file)
            }
            container.findAll(text).forEach { match -> found.putIfAbsent("container.${match.groupValues[1]}", file) }
        }
        return found
    }

    /** The catalogued actions the screens name as an intent's `type`. */
    private fun asked(): Set<String> {
        val catalog = catalog()
        val pattern = Regex("""intent\(\s*"(\w+)"|put\("type",\s*"(\w+)"\)""")
        return screens().flatMap { (_, text) ->
            pattern.findAll(text).map { it.groupValues[1].ifEmpty { it.groupValues[2] } }.toList()
        }.filter { it in catalog }.toSet()
    }

    private fun catalog(): Set<String> {
        val entries = JSONArray(intentCatalog())
        return (0 until entries.length()).map { entries.getJSONObject(it).getString("action") }.toSet()
    }

    @Test
    fun placesEveryCallAScreenMakes() {
        val found = calls()
        assertEquals("place each in this ledger", emptyList<String>(), found.keys.filter { it !in CALLS }.map { "$it (${found[it]})" })
        assertEquals("no longer called: drop them", emptyList<String>(), CALLS.keys.filter { it !in found }.sorted())
    }

    @Test
    fun listsWhatThePersonsTapsAskThroughTheIntents() {
        assertEquals(PERSON.sorted(), asked().sorted())
    }

    @Test
    fun namesOnlyActionsTheCatalogHas() {
        val catalog = catalog()
        val named = CALLS.values.mapNotNull { it.action } + PERSON
        assertEquals(emptyList<String>(), named.filter { it !in catalog })
        // A reason is never empty.
        CALLS.forEach { (name, place) -> assertTrue(name, place.why.isNotEmpty()) }
    }
}
