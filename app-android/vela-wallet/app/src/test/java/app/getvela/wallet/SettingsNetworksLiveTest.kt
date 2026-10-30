package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.settings.CalloutTone
import app.getvela.wallet.feature.settings.SettingsFixtures
import app.getvela.wallet.feature.settings.SettingsLive
import app.getvela.wallet.feature.settings.SettingsOverlay
import app.getvela.wallet.feature.settings.SettingsPage
import app.getvela.wallet.feature.settings.SettingsScreenState
import app.getvela.wallet.feature.settings.SettingsTone
import app.getvela.wallet.feature.settings.core.NetBoards
import app.getvela.wallet.feature.settings.core.NetEndpointField
import app.getvela.wallet.feature.settings.core.NetNetworkRow
import app.getvela.wallet.feature.settings.core.NetView
import app.getvela.wallet.feature.settings.core.NetworkSpace
import app.getvela.wallet.feature.wallet.core.RpcPoolView
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Settings → Networks, a network's detail, Add network's Vela networks, the
 * endpoints page and the venue's page notes, through the live builders over
 * views the REAL `network_admin` machine wrote (`NetBoards.ledger`) — each
 * against the web's `liveNetworkRows` / `liveNetworkDetail` /
 * `liveEndpoints` / `addNetworkBody` / `venueModel`.
 */
class SettingsNetworksLiveTest {

    private val strings: VelaStrings by lazy {
        val root = System.getProperty("vela.repo.root")
            ?: error("vela.repo.root not set — run via Gradle (testOptions wires it)")
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
    }

    private fun base() = SettingsFixtures.buildState(SettingsScreenState.ST1, strings)

    private val nodes = SettingsFixtures.LEDGER_NODES

    private fun list(view: NetView = NetBoards.ledger(), space: NetworkSpace = NetworkSpace.Mainnet) =
        SettingsLive.withNetworks(base(), view, strings, space = space, nodes = nodes)

    // -- the list ---------------------------------------------------------------

    @Test
    fun `a Vela network shown on the person's word can be taken out, in its own words`() {
        val model = list()
        val stable = model.networks.single { it.id == "stable" }
        assertTrue(stable.removable)
        assertEquals(strings.t(I18nKeys.SettingsUi.NETWORK_HIDE_BODY), stable.removeBody)
        val ethereum = model.networks.single { it.id == "ethereum" }
        assertFalse("shown by default: no bin", ethereum.removable)
        assertNull(ethereum.removeBody)
    }

    @Test
    fun `the hidden Vela networks are not listed and are named under the mainnet heading`() {
        val view = NetBoards.ledger()
        val model = list(view)
        assertTrue(view.hidden_vela_chain_ids.isNotEmpty())
        assertTrue(model.networks.none { row -> view.networks.none { it.id == row.id } })
        val hint = model.networksSpace!!.hint
        assertTrue(hint, hint.startsWith(strings.t(I18nKeys.NetworkSpace.MAINNET_HINT)))
        assertTrue(hint, hint.contains("Also supported, not in your list:"))
        assertTrue(hint, hint.contains("MegaETH"))
        assertFalse("shown, so not named as hidden", hint.contains("Stable"))
        // The testnet space says nothing of them: they are mainnets.
        val testnet = list(view, NetworkSpace.Testnet).networksSpace!!.hint
        assertEquals(strings.t(I18nKeys.NetworkSpace.TESTNET_HINT), testnet)
        // None hidden, no line.
        assertNull(SettingsLive.moreNetworks(view.copy(hidden_vela_chain_ids = emptyList()), strings))
    }

    @Test
    fun `a chain the core does not name is said by its id`() {
        val line = SettingsLive.moreNetworks(NetView(loaded = true, hidden_vela_chain_ids = listOf(7_777L)), strings) { emptyMap() }!!
        assertTrue(line, line.contains(strings.t(I18nKeys.SettingsUi.CHAIN_ID, mapOf("chainId" to "7777"))))
    }

    @Test
    fun `each own node's row says what the pool says of it`() {
        val model = list()
        val gnosis = model.networks.single { it.id == "gnosis" }
        assertEquals(SettingsTone.Error, gnosis.notice!!.tone)
        assertEquals("Can't reach your node for Gnosis", gnosis.notice!!.text)
        val arbitrum = model.networks.single { it.id == "arbitrum" }
        assertEquals(SettingsTone.Warn, arbitrum.notice!!.tone)
        assertEquals(strings.t(I18nKeys.SettingsUi.NETWORK_FALLBACK), arbitrum.notice!!.text)
        assertNull("the shipped endpoint is not the person's node", model.networks.single { it.id == "ethereum" }.notice)
        assertTrue("no pool word, no notice", SettingsLive.withNetworks(base(), NetBoards.ledger(), strings).networks.all { it.notice == null })
    }

    @Test
    fun `the pool's view is read as the two lists`() {
        val own = SettingsLive.OwnNodes.of(RpcPoolView(fallback_chains = listOf(42161), own_node_unreached_chains = listOf(100)))
        assertEquals(listOf(42_161L), own.fallback)
        assertEquals(listOf(100L), own.unreached)
    }

    @Test
    fun `the remove question names the network, and a Vela network's says it is kept`() {
        val model = list()
        val stable = SettingsLive.removeNetworkQuestion(model, "stable")
        assertEquals("Stable", stable.title)
        assertEquals(strings.t(I18nKeys.SettingsUi.NETWORK_HIDE_BODY), stable.body)
        assertEquals(model.removeNetworkSheet.confirm, stable.confirm)
        val custom = model.copy(networks = model.networks + model.networks.first().copy(id = "custom-7", name = "Seven", removable = true))
        assertEquals(model.removeNetworkSheet.body, SettingsLive.removeNetworkQuestion(custom, "custom-7").body)
        assertEquals("Seven", SettingsLive.removeNetworkQuestion(custom, "custom-7").title)
        assertEquals(model.removeNetworkSheet, SettingsLive.removeNetworkQuestion(model, "nowhere"))
        assertEquals(model.removeNetworkSheet, SettingsLive.removeNetworkQuestion(model, null))
    }

    // -- a network's detail ------------------------------------------------------

    private fun detail(chainId: Long) = NetBoards.ledger(expand = chainId).let { view ->
        SettingsLive.networkDetail(base().networkDetail, view.networks.single { it.chain_id == chainId }, strings, nodes)
    }

    @Test
    fun `only use my node is offered where the RPC is the person's own, with its line`() {
        val on = detail(NetBoards.OWN_ONLY_CHAIN)
        assertEquals(strings.t(I18nKeys.SettingsUi.NETWORK_ONLY_MINE), on.onlyMine!!.title)
        assertEquals(strings.t(I18nKeys.SettingsUi.NETWORK_ONLY_MINE_HINT), on.onlyMine!!.subtitle)
        assertTrue(on.onlyMine!!.on)
        assertEquals(NetBoards.OWN_ONLY_CHAIN, on.chainId)
        assertFalse(detail(NetBoards.OWN_CHAIN).onlyMine!!.on)
        assertNull("the shipped endpoint offers no switch", detail(1L).onlyMine)
    }

    @Test
    fun `the detail says the node cannot be reached, or that public nodes serve for now`() {
        val down = detail(NetBoards.OWN_ONLY_CHAIN).nodeCallout!!
        assertEquals(CalloutTone.Danger, down.tone)
        assertEquals("Can't reach your node for Gnosis", down.text)
        val fallback = detail(NetBoards.OWN_CHAIN).nodeCallout!!
        assertEquals(CalloutTone.Warning, fallback.tone)
        assertEquals(strings.t(I18nKeys.SettingsUi.NETWORK_FALLBACK), fallback.text)
        assertNull(detail(1L).nodeCallout)
    }

    @Test
    fun `a refused address is never drawn beside a latency pill`() {
        val row = NetNetworkRow(id = "x", chain_id = 1, display_name = "Ethereum", native_symbol = "ETH", rpc_url = "http://evil.example", rpc_url_refused = true, explorer_url_refused = true)
        val refused = SettingsLive.networkDetail(base().networkDetail, row, strings)
        val pill = strings.t(I18nKeys.SettingsUi.HEALTH_HTTPS_REQUIRED)
        assertEquals(pill, refused.rpc.badge!!.label)
        assertEquals(SettingsTone.Error, refused.rpc.tone)
        assertEquals(pill, refused.explorer.badge!!.label)
        assertEquals(pill, refused.badge.label)
        assertEquals(CalloutTone.Danger, refused.callout!!.tone)
        assertEquals(strings.t(I18nKeys.SettingsUi.HEALTH_URL_REFUSED), refused.callout!!.text)
    }

    // -- Add network: a Vela network shown again ---------------------------------

    @Test
    fun `the search finds a hidden Vela network and says it is Vela's`() {
        val model = SettingsLive.withWizard(base(), NetBoards.velaWizard("Mega"), strings, spaceChoice = true)
        val row = model.addNetwork.results.single { it.name == "MegaETH" }
        assertEquals(strings.t(I18nKeys.SettingsUi.ADD_VELA_NETWORK), row.tag)
    }

    @Test
    fun `a Vela network picked is offered back as itself, nothing to check`() {
        val add = SettingsLive.withWizard(base(), NetBoards.velaWizard("Mega", pick = 4_326L), strings, spaceChoice = true).addNetwork
        assertEquals("MegaETH", add.candidate!!.name)
        assertEquals(strings.t(I18nKeys.SettingsUi.ADD_VELA_NETWORK), add.candidate!!.badge!!.label)
        assertEquals(SettingsTone.Ok, add.candidate!!.badge!!.tone)
        assertEquals(CalloutTone.Info, add.callout!!.tone)
        assertEquals(strings.t(I18nKeys.SettingsUi.ADD_VELA_NETWORK_HINT), add.callout!!.text)
        assertEquals(strings.t(I18nKeys.SettingsUi.ADD_BUTTON), add.primary)
        assertTrue(add.checks.isEmpty())
        assertNull(add.checksTitle)
        assertNull(add.customRpc)
        assertNull(add.recheck)
        assertNull("no space to choose: it is the mainnet it ships as", add.space)
    }

    // -- the endpoints page --------------------------------------------------------

    @Test
    fun `every endpoint the core lists, the multisig service with them, each probed`() {
        val endpoints = list(NetBoards.ledger(endpoints = true)).endpoints
        assertEquals(NetEndpointField.entries.map { it.name }, endpoints.fields.map { it.id })
        val byId = endpoints.fields.associateBy { it.id }
        val genesis = byId.getValue(NetEndpointField.GenesisService.name)
        assertEquals("Multisig service", genesis.label)
        assertEquals(strings.t(I18nKeys.SettingsUi.ENDPOINT_GENESIS_HINT), genesis.hint)
        assertEquals("https://safe-genesis.getvela.app", genesis.placeholder)
        assertEquals(SettingsTone.Ok, genesis.badge!!.tone)
        assertEquals(SettingsTone.Ok, byId.getValue(NetEndpointField.EthereumData.name).badge!!.tone)
        assertEquals(strings.t(I18nKeys.SettingsUi.HEALTH_INVALID), byId.getValue(NetEndpointField.PasskeyIndex.name).badge!!.label)
        assertEquals(SettingsTone.Warn, byId.getValue(NetEndpointField.FiatRates.name).badge!!.tone)
        val relay = byId.getValue(NetEndpointField.BundlerService.name)
        assertEquals(strings.t(I18nKeys.SettingsUi.HEALTH_URL_REFUSED), relay.hint)
        assertEquals(SettingsTone.Error, relay.tone)
        assertEquals(strings.t(I18nKeys.SettingsUi.ENDPOINTS_GUIDE), endpoints.guide)
        assertEquals(strings.t(I18nKeys.SettingsUi.ENDPOINT_CHAIN_DATA), byId.getValue(NetEndpointField.EthereumData.name).label)
    }

    // -- where you review and sign ------------------------------------------------

    @Test
    fun `the venue says what a signing page protects against and what it does not`() {
        val account = app.getvela.wallet.feature.onboarding.core.SessionAccountRow(
            index = 0, name = "Savings", address = "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
            signingDomain = "getvela.app", signingVenueJson = """{"type":"in_vela"}""",
        )
        val line = { _: String -> SettingsFixtures.pageLine(SettingsFixtures.OFFICIAL_PAGE) }
        val venue = SettingsLive.withVenue(base(), account, emptyList(), line, strings).venue!!
        assertEquals(2, venue.pageNotes.size)
        assertEquals(strings.t(I18nKeys.SettingsUi.VENUE_PAGE_PROTECTS), venue.pageNotes[0])
        assertTrue(venue.pageNotes[1], venue.pageNotes[1].contains("getvela.app"))
        assertTrue("the official page is a choice", venue.choices.any { it.page?.official == true })
    }

    // -- the gallery boards ------------------------------------------------------

    @Test
    fun `the ledger boards open where they say, drawn by the live builders`() {
        assertEquals(SettingsPage.Networks, SettingsFixtures.buildState(SettingsScreenState.ST9C, strings).page)
        val down = SettingsFixtures.buildState(SettingsScreenState.ST9D, strings)
        assertEquals(SettingsPage.NetworkDetail, down.page)
        assertTrue(down.networkDetail.onlyMine!!.on)
        assertNotNull(down.networkDetail.nodeCallout)
        assertFalse(SettingsFixtures.buildState(SettingsScreenState.ST9E, strings).networkDetail.onlyMine!!.on)
        val hide = SettingsFixtures.buildState(SettingsScreenState.ST9F, strings)
        assertEquals(SettingsOverlay.RemoveNetwork, hide.overlay)
        assertEquals("Stable", hide.removeNetworkSheet.title)
        assertEquals(strings.t(I18nKeys.SettingsUi.NETWORK_HIDE_BODY), hide.removeNetworkSheet.body)
        assertEquals(strings.t(I18nKeys.SettingsUi.ADD_VELA_NETWORK), SettingsFixtures.buildState(SettingsScreenState.ST10K, strings).addNetwork.results.first().tag)
        assertEquals("MegaETH", SettingsFixtures.buildState(SettingsScreenState.ST10L, strings).addNetwork.candidate!!.name)
        val endpoints = SettingsFixtures.buildState(SettingsScreenState.ST12B, strings)
        assertEquals(SettingsPage.Endpoints, endpoints.page)
        assertEquals(5, endpoints.endpoints.fields.size)
    }
}
