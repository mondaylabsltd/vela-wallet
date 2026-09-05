package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.settings.SettingsFixtures
import app.getvela.wallet.feature.settings.SettingsLive
import app.getvela.wallet.feature.settings.SettingsScreenState
import app.getvela.wallet.feature.settings.core.CurrencyView
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
        // FR-013. The ST9 fixture rows carry a green `42ms`; a live row carries
        // nothing until spec 041 can probe. Passing a health through would be
        // the easy version of this bug, so the assertion covers both.
        val view = NetView(
            loaded = true,
            networks = listOf(
                row(1, "Ethereum", false),
                row(10, "Optimism", false, NetProbeHealth.Ok(42)),
            ),
        )
        val model = SettingsLive.withNetworks(base(), view, strings)
        assertTrue(
            "no live network row may show a status pill in spec 040",
            model.networks.all { it.badge == null },
        )
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
}
