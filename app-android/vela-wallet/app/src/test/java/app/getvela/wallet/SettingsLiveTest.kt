package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.settings.SettingsFixtures
import app.getvela.wallet.feature.settings.SettingsLive
import app.getvela.wallet.feature.settings.SettingsScreenState
import app.getvela.wallet.feature.settings.SettingsTone
import app.getvela.wallet.feature.settings.core.CurrencyView
import app.getvela.wallet.feature.settings.core.NetChainIndexEntry
import app.getvela.wallet.feature.settings.core.NetChainInfo
import app.getvela.wallet.feature.settings.core.NetCompatibility
import app.getvela.wallet.feature.settings.core.NetContractStatus
import app.getvela.wallet.feature.settings.core.NetWizardPhase
import app.getvela.wallet.feature.settings.core.NetWizardView
import app.getvela.wallet.feature.settings.core.NetEndpointField
import app.getvela.wallet.feature.settings.core.NetEndpointView
import app.getvela.wallet.feature.settings.core.NetNetworkRow
import app.getvela.wallet.feature.settings.core.NetProbeHealth
import app.getvela.wallet.feature.settings.core.NetProviderId
import app.getvela.wallet.feature.settings.core.NetProviderView
import app.getvela.wallet.feature.settings.core.NetServiceHealth
import app.getvela.wallet.feature.settings.core.NetView
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The live settings builders: a core view in, the drawn display models out.
 *
 * The assertions worth having here are the ones about what must NOT appear —
 * a fixture person's chain in a live list, a latency pill nobody measured, a
 * field whose id the host cannot map back to a machine event.
 */
class SettingsLiveTest {

    /** The real engine over the real catalogs, as the fixture tests do it. */
    private val strings: VelaStrings by lazy {
        val root = System.getProperty("vela.repo.root")
            ?: error("vela.repo.root not set — run via Gradle (testOptions wires it)")
        I18nRuntime { tag -> File(root, "public/i18n/$tag.json").readBytes() }
            .apply { initialize("en") }
    }

    private fun base() = SettingsFixtures.buildState(SettingsScreenState.ST1, strings)

    private fun row(chainId: Long, name: String, custom: Boolean, health: NetProbeHealth? = null) =
        NetNetworkRow(
            id = if (custom) "custom-$chainId" else name.lowercase(),
            chain_id = chainId,
            display_name = name,
            native_symbol = "SYM",
            is_custom = custom,
            rpc_health = health,
        )

    // -- networks ------------------------------------------------------------

    @Test
    fun anUnloadedViewLeavesTheScreenAlone() {
        // Before storage answers there is nothing truer to show than what is
        // already on screen. Blanking the list for a frame is a flicker a
        // person reads as a bug.
        val model = base()
        assertEquals(model.networks, SettingsLive.withNetworks(model, NetView(), strings).networks)
    }

    @Test
    fun theListIsTheCoresNotTheFixtures() {
        val view = NetView(
            loaded = true,
            networks = listOf(row(1, "Ethereum", false), row(7777, "Seven", true)),
        )
        val model = SettingsLive.withNetworks(base(), view, strings)

        assertEquals(2, model.networks.size)
        assertEquals("Ethereum", model.networks[0].name)
        // The fixture list has eleven chains including Zora and Zircuit; none
        // of them may survive into a live render.
        assertTrue(model.networks.none { it.name == "Zora" })
    }

    @Test
    fun onlyCustomNetworksCanBeRemoved() {
        val view = NetView(
            loaded = true,
            networks = listOf(row(1, "Ethereum", false), row(7777, "Seven", true)),
        )
        val model = SettingsLive.withNetworks(base(), view, strings)
        assertEquals(false, model.networks[0].removable)
        assertEquals(true, model.networks[1].removable)
        assertNull("a built-in network carries no custom tag", model.networks[0].tag)
    }

    @Test
    fun noRowClaimsALatencyNobodyMeasured() {
        // FR-013, and the rule that outlived the phase that wrote it.
        //
        // In spec 040 nothing could probe, so NO row could carry a pill. Spec
        // 041 gives the shell a way to measure — and the invariant is unchanged
        // in the only way that matters: a row shows a latency when somebody
        // measured it, and shows nothing when nobody has. "Checking" is nothing
        // too; a list of twelve spinners reads as a broken screen.
        val view = NetView(
            loaded = true,
            networks = listOf(
                row(1, "Ethereum", false),
                row(10, "Optimism", false, NetProbeHealth.Checking),
            ),
        )
        val model = SettingsLive.withNetworks(base(), view, strings)
        assertTrue(
            "an unprobed row must not claim a latency",
            model.networks.all { it.badge == null },
        )
    }

    @Test
    fun aMeasuredEndpointShowsWhatWasMeasured() {
        val view = NetView(
            loaded = true,
            networks = listOf(
                row(1, "Ethereum", false, NetProbeHealth.Ok(42)),
                row(10, "Optimism", false, NetProbeHealth.Ok(2_400)),
                row(56, "BNB", false, NetProbeHealth.Error),
            ),
        )

        val rows = SettingsLive.withNetworks(base(), view, strings).networks

        // The number is the one that came back, in the drawn design's units.
        assertTrue("fast reads in milliseconds", rows[0].badge!!.label.endsWith("42ms"))
        assertEquals(SettingsTone.Ok, rows[0].badge!!.tone)
        // Past a second it reads in seconds and stops being green — the same
        // threshold the fixture has always drawn.
        assertTrue("slow reads in seconds", rows[1].badge!!.label.endsWith("2.4s"))
        assertEquals(SettingsTone.Warn, rows[1].badge!!.tone)
        // A failed probe is a verdict, and it carries no number at all: there
        // is no latency for a request that never answered.
        assertEquals(SettingsTone.Error, rows[2].badge!!.tone)
        assertTrue("a failure has no latency to report", !rows[2].badge!!.label.contains("ms"))
    }

    @Test
    fun aNetworkKeepsItsMarkBetweenLaunches() {
        val view = NetView(loaded = true, networks = listOf(row(8453, "Base", false)))
        val first = SettingsLive.withNetworks(base(), view, strings).networks[0].mark
        val second = SettingsLive.withNetworks(base(), view, strings).networks[0].mark
        assertEquals(first, second)
        assertEquals("B", first.letter)
    }

    // -- endpoints and providers --------------------------------------------

    @Test
    fun anEndpointFieldCarriesTheIdTheHostMapsBack() {
        // If this id is not the core's own enum name, typing in the box reaches
        // nothing: `endpointField(fieldId)` returns null and the keystroke is
        // dropped in silence. That is the failure this test exists for.
        val view = NetView(
            loaded = true,
            endpoints = listOf(
                NetEndpointView(
                    field = NetEndpointField.BundlerService,
                    value = "https://b.example",
                    default_value = "https://default.example",
                    health = NetServiceHealth.Checking,
                ),
            ),
        )
        val model = SettingsLive.withNetworks(base(), view, strings)
        val field = model.endpoints.fields.single()

        assertEquals(NetEndpointField.BundlerService.name, field.id)
        assertEquals("https://b.example", field.value)
        // The default is the placeholder: an unset endpoint shows what it WOULD
        // use, greyed, rather than an empty box.
        assertEquals("https://default.example", field.placeholder)
    }

    @Test
    fun aProviderKeyFieldCarriesItsProviderId() {
        val view = NetView(
            loaded = true,
            providers = listOf(
                NetProviderView(
                    provider = NetProviderId.Alchemy,
                    key = "secret",
                    has_key = true,
                ),
            ),
        )
        val model = SettingsLive.withNetworks(base(), view, strings)
        val card = model.rpcProviders.providers.single()

        assertEquals(NetProviderId.Alchemy.name, card.field.id)
        assertEquals("secret", card.field.value)
        assertEquals("Alchemy", card.name)
    }

    // -- currency ------------------------------------------------------------

    @Test
    fun theCurrencySheetTicksTheCoresChoice() {
        val model = SettingsLive.withCurrency(base(), CurrencyView("JPY", null, true))
        val selected = model.currencySheet.rows.filter { it.selected }
        assertEquals(1, selected.size)
        assertEquals("JPY", selected.single().id)
        // Eight offered, in the catalogue's order — the same eight the web
        // client offers.
        assertEquals(8, model.currencySheet.rows.size)
        assertEquals("USD", model.currencySheet.rows.first().id)
    }

    @Test
    fun theCurrencyRowShowsNoAmountItCannotConvert() {
        // A sample like "¥1,234.56" implies a rate, and there is none until
        // spec 041. The row shows the code and its glyph, nothing more.
        val model = SettingsLive.withCurrency(base(), CurrencyView("JPY", null, true))
        val row = model.sections.flatMap { it.rows }.single { it.id == "currency" }
        assertEquals("JPY · ¥", row.value)
    }

    // -- the add-network wizard (spec 041 phase 8) ---------------------------

    private fun indexEntry(chainId: Long, name: String, symbol: String) = NetChainIndexEntry(
        chain_id = chainId,
        name = name,
        short_name = name.lowercase(),
        native_currency_symbol = symbol,
        has_logo = false,
    )

    private fun chainInfo(chainId: Long, name: String, testnet: Boolean = false) = NetChainInfo(
        chain_id = chainId,
        name = name,
        short_name = name.lowercase(),
        native_name = name,
        native_symbol = "CELO",
        native_decimals = 18,
        rpc_url = "https://rpc.example",
        rpc_urls = listOf("https://rpc.example"),
        explorer_url = "https://explorer.example",
        logo_url = "",
        is_testnet = testnet,
    )

    private fun wizardView(wizard: NetWizardView) = NetView(loaded = true, wizard = wizard)

    /**
     * **A search nobody performed has no results.**
     *
     * The fixture drew three under an empty box — which reads as "these are
     * your options" — and two of them were chains that do not exist.
     */
    @Test
    fun anEmptySearchOffersNothing() {
        val view = wizardView(
            NetWizardView(query = "", suggestions = listOf(indexEntry(42220, "Celo", "CELO"))),
        )

        val add = SettingsLive.withWizard(base(), view, strings).addNetwork

        assertEquals(emptyList<Any>(), add.results)
    }

    @Test
    fun aSearchShowsWhatTheIndexAnswered() {
        val view = wizardView(
            NetWizardView(
                query = "celo",
                suggestions = listOf(
                    indexEntry(42220, "Celo Mainnet", "CELO"),
                    indexEntry(44787, "Celo Alfajores Testnet", "CELO"),
                ),
            ),
        )

        val add = SettingsLive.withWizard(base(), view, strings).addNetwork

        assertEquals(listOf("Celo Mainnet", "Celo Alfajores Testnet"), add.results.map { it.name })
        // The chain id IS the identity: two chains can share a name, and the
        // tap has to reach the right one.
        assertEquals(listOf("42220", "44787"), add.results.map { it.id })
        assertTrue(add.results[0].meta.contains("42220"))
        assertTrue(add.results[0].meta.contains("CELO"))
    }

    /**
     * **No verdict until one was reached.**
     *
     * The same rule as the latency pill: a chain drawn as compatible before
     * anything was checked is a claim nobody made. And the add button — which
     * writes a network somebody's money will be read from — must not be
     * reachable while the checks are still running.
     */
    @Test
    fun aCandidateBeingCheckedCarriesNoVerdictAndNoButton() {
        val view = wizardView(
            NetWizardView(
                phase = NetWizardPhase.Checking,
                chain_info = chainInfo(42220, "Celo Mainnet"),
                compat = null,
                can_add = false,
            ),
        )

        val add = SettingsLive.withWizard(base(), view, strings).addNetwork

        assertEquals("Celo Mainnet", add.candidate!!.name)
        assertNull("no pill before a verdict", add.candidate!!.badge)
        assertEquals(emptyList<Any>(), add.checks)
        assertNull("the add button must not be reachable mid-check", add.primary)
    }

    @Test
    fun acheckedCandidateShowsEveryContractAndOffersTheButton() {
        val view = wizardView(
            NetWizardView(
                phase = NetWizardPhase.Checked,
                chain_info = chainInfo(42220, "Celo Mainnet"),
                compat = NetCompatibility(
                    chain_id = 42220,
                    compatible = true,
                    contracts = listOf(
                        NetContractStatus("EntryPoint", "0xaa", deployed = true),
                        NetContractStatus("Safe", "0xbb", deployed = true),
                    ),
                ),
                can_add = true,
            ),
        )

        val add = SettingsLive.withWizard(base(), view, strings).addNetwork

        assertEquals(SettingsTone.Ok, add.candidate!!.badge!!.tone)
        assertEquals(listOf("EntryPoint", "Safe"), add.checks.map { it.label })
        assertTrue(add.checks.all { it.ok })
        assertTrue(add.primary!!.isNotBlank())
    }

    /** An incompatible chain is named as such, and cannot be added. */
    @Test
    fun anIncompatibleChainIsNotOfferedForAdding() {
        val view = wizardView(
            NetWizardView(
                phase = NetWizardPhase.Checked,
                chain_info = chainInfo(1234, "Nowhere"),
                compat = NetCompatibility(
                    chain_id = 1234,
                    compatible = false,
                    contracts = listOf(NetContractStatus("EntryPoint", "0xaa", deployed = false)),
                ),
                can_add = false,
            ),
        )

        val add = SettingsLive.withWizard(base(), view, strings).addNetwork

        assertEquals(SettingsTone.Error, add.candidate!!.badge!!.tone)
        assertEquals(false, add.checks.single().ok)
        assertNull("a chain whose contracts are missing cannot be added", add.primary)
    }

    /** A testnet says so, because sending real money to one loses it. */
    @Test
    fun aTestnetCandidateIsTagged() {
        val view = wizardView(
            NetWizardView(chain_info = chainInfo(44787, "Celo Alfajores Testnet", testnet = true)),
        )

        val add = SettingsLive.withWizard(base(), view, strings).addNetwork

        assertTrue(add.candidate!!.tag!!.isNotBlank())
    }
}
