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
import app.getvela.wallet.feature.settings.core.NetChainMismatch
import app.getvela.wallet.feature.settings.core.NetProviderTestView
import app.getvela.wallet.feature.settings.CalloutTone
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.wallet.core.BalanceToken
import app.getvela.wallet.feature.wallet.core.BalanceView
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import app.getvela.wallet.core.data.PrefsView
import app.getvela.wallet.core.format.Formats
import app.getvela.wallet.core.format.NumberFormatKey
import app.getvela.wallet.core.format.TimeFormatKey
import java.util.Locale

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
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }
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

    /** The web's `servicePill`: every `NetServiceHealth` worded, quiet while checking. */
    @Test
    fun anEndpointSaysWhatItsHealthCheckFound() {
        fun endpoint(health: NetServiceHealth) = NetEndpointView(NetEndpointField.EthereumData, "", "https://d.example", health)
        val view = NetView(
            loaded = true,
            endpoints = listOf(
                endpoint(NetServiceHealth.Checking),
                endpoint(NetServiceHealth.Ok(42)),
                endpoint(NetServiceHealth.Ok(2_400)),
                endpoint(NetServiceHealth.NotHttps),
                endpoint(NetServiceHealth.Unreachable(http_status = 502)),
                endpoint(NetServiceHealth.InvalidResponse(latency_ms = 10)),
            ),
        )
        val pills = SettingsLive.withNetworks(base(), view, strings).endpoints.fields.map { it.badge }

        assertNull("checking claims nothing", pills[0])
        assertEquals(SettingsTone.Ok, pills[1]!!.tone)
        assertTrue(pills[1]!!.label.endsWith("42ms"))
        assertEquals(SettingsTone.Warn, pills[2]!!.tone)
        assertEquals(SettingsTone.Error to strings.t(I18nKeys.SettingsUi.HEALTH_HTTPS_REQUIRED), pills[3]!!.tone to pills[3]!!.label)
        assertEquals(SettingsTone.Error to strings.t(I18nKeys.SettingsUi.NETWORK_OFFLINE), pills[4]!!.tone to pills[4]!!.label)
        assertEquals(SettingsTone.Error to strings.t(I18nKeys.SettingsUi.HEALTH_INVALID), pills[5]!!.tone to pills[5]!!.label)
    }

    /** The web's `liveNetworkRows`: a custom network's row wears its tag, not a health pill. */
    @Test
    fun aCustomNetworkRowCarriesNoHealthPill() {
        val view = NetView(
            loaded = true,
            networks = listOf(row(1, "Ethereum", false, NetProbeHealth.Ok(42)), row(7777, "Seven", true, NetProbeHealth.Ok(42))),
        )
        val rows = SettingsLive.withNetworks(base(), view, strings).networks
        assertTrue(rows[0].badge != null)
        assertNull(rows[1].badge)
        assertEquals(strings.t(I18nKeys.SettingsUi.NETWORK_CUSTOM), rows[1].tag)
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

    /**
     * A provider card says what the core knows about THAT provider: whether a
     * key is set, and what its test found. The fixture drew Alchemy connected
     * with "supports 12" and Ankr unkeyed with "supports 8" on every device.
     */
    @Test
    fun aProviderCardShowsItsKeyAndItsOwnTestCount() {
        val view = NetView(
            loaded = true,
            providers = listOf(
                NetProviderView(NetProviderId.Alchemy, key = "", has_key = false),
                NetProviderView(
                    NetProviderId.Drpc,
                    key = "k",
                    has_key = true,
                    test = NetProviderTestView(done = true, ok_count = 3, total = 5),
                ),
                NetProviderView(
                    NetProviderId.Ankr,
                    key = "k",
                    has_key = true,
                    test = NetProviderTestView(done = false, ok_count = 1, total = 5),
                ),
            ),
        )
        val cards = SettingsLive.withNetworks(base(), view, strings).rpcProviders.providers

        val notSet = strings.t(I18nKeys.SettingsUi.PROVIDER_NOT_SET)
        assertEquals(SettingsTone.Neutral, cards[0].badge.tone)
        assertEquals(notSet, cards[0].badge.label)
        assertEquals(strings.t(I18nKeys.SettingsUi.PROVIDER_GET_KEY), cards[0].action)
        assertNull("no test ran, so no count", cards[0].support)
        // The link goes to the provider's own site — it used to open its label ("https://Get key →").
        assertEquals("https://dashboard.alchemy.com/", cards[0].linkUrl)

        assertEquals(SettingsTone.Ok, cards[1].badge.tone)
        assertEquals(strings.t(I18nKeys.SettingsUi.PROVIDER_CHECK_KEY), cards[1].action)
        assertNull("a keyed provider offers no get-key link", cards[1].link)
        assertEquals(
            strings.t(I18nKeys.SettingsUi.PROVIDER_SUPPORTS, mapOf("count" to "3", "total" to "5")),
            cards[1].support,
        )

        assertNull("a test still running has no count yet", cards[2].support)
    }

    /**
     * The network page shows what the probes found: an RPC that answers for
     * another chain is a danger note (fast and wrong), and the explorer has
     * its own pill. Before, the callout was always null and the header pill
     * was the fixture's latency.
     */
    @Test
    fun theNetworkPageNamesAWrongChainAndTheExplorersHealth() {
        val fallback = base().networkDetail
        val healthy = row(1, "Ethereum", custom = false).copy(
            explorer_health = NetProbeHealth.Ok(80),
        )
        val clean = SettingsLive.networkDetail(fallback, healthy, strings)
        assertNull(clean.callout)
        assertTrue(clean.explorer.badge!!.label.endsWith("80ms"))
        assertNull("the RPC was never probed", clean.rpc.badge)
        assertEquals("an unprobed header is quiet, not a latency", SettingsTone.Neutral, clean.badge.tone)

        val wrong = healthy.copy(
            rpc_health = NetProbeHealth.Ok(40),
            rpc_chain_mismatch = NetChainMismatch(expected_chain_id = 1, reported_chain_id = 56),
        )
        val detail = SettingsLive.networkDetail(fallback, wrong, strings)
        assertEquals(CalloutTone.Danger, detail.callout!!.tone)
        assertEquals(
            strings.t(I18nKeys.SettingsUi.RPC_CHAIN_MISMATCH, mapOf("reported" to "56", "expected" to "1")),
            detail.callout!!.text,
        )
        assertEquals(SettingsTone.Error, detail.rpc.tone)
        assertTrue(detail.badge.label.endsWith("40ms"))
    }

    /** The balance-by-network sheet, from the balance core's view. */
    @Test
    fun theBalanceDetailIsTheViewsChainsNotTheFixtures() {
        val view = BalanceView(
            tokens = listOf(
                BalanceToken(1, "ETH", "Ether", balance = "2", decimals = 18, price_usd = 1000.0),
                BalanceToken(10, "ETH", "Ether", balance = "1", decimals = 18, price_usd = 1000.0),
                BalanceToken(10, "OP", "Optimism", balance = "10", decimals = 18, token_address = "0xop", price_usd = 1.5),
                BalanceToken(137, "POL", "Polygon", balance = "5", decimals = 18, price_usd = 0.5),
            ),
            unpriced_tokens = listOf(BalanceToken(8453, "ODD", "Odd", balance = "1.23456789", decimals = 18, token_address = "0xodd")),
            failed_chain_ids = listOf(100, 137),
            rate_limited_chain_ids = listOf(137),
            banner_chain_ids = listOf(100),
            display_total_usd = 3017.5,
        )
        val names = mapOf(1 to "Ethereum", 10 to "Optimism", 100 to "Gnosis", 137 to "Polygon", 8453 to "Base")
        val usd = CurrencyView("USD", null, true)
        val detail = SettingsLive.balanceDetail(base().balanceDetail, view, usd, names, strings)

        // Rate-limited: grey, no button. Down: red, with the retry.
        assertEquals(listOf("137", "100"), detail.pending.map { it.id })
        assertNull("a rate limit heals itself; no retry", detail.pending[0].action)
        assertEquals(SettingsTone.Neutral, detail.pending[0].tone)
        assertEquals(SettingsTone.Error, detail.pending[1].tone)
        assertEquals(strings.t(I18nKeys.SettingsUi.BALANCE_DETAIL_RETRY), detail.pending[1].action)
        // Settled chains largest first; a still-pending chain is not also "done".
        assertEquals(listOf("Ethereum", "Optimism"), detail.done.map { it.name })
        assertEquals(WalletLive.Money.of(usd).fiat(2000.0), detail.done[0].amount)
        assertEquals(WalletLive.Money.of(usd).fiat(1015.0), detail.done[1].amount)
        assertEquals(
            strings.t(I18nKeys.SettingsUi.BALANCE_DETAIL_TOTAL, mapOf("amount" to WalletLive.Money.of(usd).fiat(3017.5))),
            detail.summary,
        )
        assertEquals(listOf("ODD"), detail.unpriced.map { it.name })
        assertTrue(detail.unpriced.single().status!!, detail.unpriced.single().status!!.startsWith("Base · "))
        assertTrue("no fixture chain survives", (detail.pending + detail.done).none { it.name == "BNB Chain" })

        val hidden = SettingsLive.balanceDetail(base().balanceDetail, view.copy(hidden = true), usd, names, strings)
        assertTrue(hidden.done.all { it.amount == "••••" })
        assertTrue(hidden.summary, hidden.summary.contains("••••"))
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
        assertNull("no custom RPC field before a verdict", add.customRpc)
    }

    /**
     * The custom-RPC box shows what the core holds, so typing echoes. The
     * live builder never set the field: the ST1 base has none, and the drawn
     * one was a fixed blank.
     */
    @Test
    fun theCustomRpcFieldIsTheWizardsOwnValue() {
        fun compat(compatible: Boolean) = NetCompatibility(chain_id = 42220, compatible = compatible)
        val checked = NetWizardView(
            phase = NetWizardPhase.Checked,
            chain_info = chainInfo(42220, "Celo Mainnet"),
            custom_rpc = "https://my.rpc",
            compat = compat(true),
            can_add = true,
        )
        val field = SettingsLive.withWizard(base(), wizardView(checked), strings).addNetwork.customRpc
        assertEquals("https://my.rpc", field!!.value)
        assertEquals("custom-rpc", field.id)

        // An incompatible chain is not rescued by a better RPC: no field (the web's rule).
        val incompatible = checked.copy(compat = compat(false), can_add = false)
        assertNull(SettingsLive.withWizard(base(), wizardView(incompatible), strings).addNetwork.customRpc)
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
                    // Fully compatible, multi-key included — which is what
                    // "nothing to explain" below means. The flag defaults to
                    // false, and a chain that cannot hold a multi-passkey
                    // wallet DOES get a callout saying so (spec 081 FR-009,
                    // the test two below this one).
                    multi_key_ready = true,
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
        // The core's contracts, then the signer precompile (the web's `checkSigner`).
        // The row is named for what it CHECKS — `p256_available`, the precompile.
        // It used to read "WebAuthn signer module", which is the name of a
        // different contract in the same list (main, 87502cd0).
        assertEquals(listOf("EntryPoint", "Safe", "P-256 precompile"), add.checks.map { it.label })
        assertTrue(add.checks.dropLast(1).all { it.ok })
        assertTrue(add.primary!!.isNotBlank())
        assertNull("nothing to explain on a compatible chain", add.callout)
    }

    /**
     * Spec 081 FR-009. A chain can be compatible AND unable to hold a wallet
     * made from several passkeys. The pill stays green — a one-key wallet does
     * work there — and the callout says the rest, because two crossed rows
     * under a green "Compatible" explain nothing on their own.
     */
    @Test
    fun aChainWithoutSafesPasskeyFactorySaysSoWhileStayingCompatible() {
        val checked = NetWizardView(
            phase = NetWizardPhase.Checked,
            chain_info = chainInfo(42220, "Celo Mainnet"),
            compat = NetCompatibility(
                chain_id = 42220,
                compatible = true,
                multi_key_ready = false,
                contracts = listOf(
                    NetContractStatus("EntryPoint", "0xaa", deployed = true),
                    NetContractStatus("Safe", "0xbb", deployed = true),
                    NetContractStatus(
                        "Safe Passkey Signer Factory",
                        "0xcc",
                        deployed = false,
                        multi_key_only = true,
                    ),
                ),
            ),
            can_add = true,
        )

        val add = SettingsLive.withWizard(base(), wizardView(checked), strings).addNetwork
        assertEquals(SettingsTone.Ok, add.candidate!!.badge!!.tone)
        assertEquals(CalloutTone.Warning, add.callout!!.tone)
        assertTrue(add.callout!!.text.isNotBlank())

        // And a chain that has everything says nothing extra.
        val whole = checked.copy(
            compat = checked.compat!!.copy(
                multi_key_ready = true,
                contracts = checked.compat!!.contracts.map { it.copy(deployed = true) },
            ),
        )
        assertNull(SettingsLive.withWizard(base(), wizardView(whole), strings).addNetwork.callout)
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
        assertEquals(false, add.checks.first { it.label == "EntryPoint" }.ok)
        assertNull("a chain whose contracts are missing cannot be added", add.primary)
        // Spec 072: it says why, offers the setup tool and a re-check.
        assertTrue(add.callout!!.text.isNotBlank())
        assertTrue(add.secondary!!.isNotBlank())
        assertTrue(add.recheck!!.isNotBlank())
    }

    /**
     * Spec 072: a chain that could not be CHECKED is not called incompatible
     * (the core's invariant ③): a warning, Retry and re-check — never the
     * setup tool, which is for chains really missing Vela's contracts.
     */
    @Test
    fun aChainThatCouldNotBeCheckedIsNeverCalledIncompatible() {
        val view = wizardView(
            NetWizardView(
                phase = NetWizardPhase.Checked,
                chain_info = chainInfo(1234, "Somewhere"),
                compat = null,
                can_add = false,
            ),
        )

        val add = SettingsLive.withWizard(base(), view, strings).addNetwork

        assertEquals(SettingsTone.Warn, add.candidate!!.badge!!.tone)
        assertEquals(strings.t("settingsModals.addNetwork.unableToVerify"), add.candidate!!.badge!!.label)
        assertEquals(strings.t("settingsModals.addNetwork.retry"), add.primary)
        assertNull(add.secondary)
        assertTrue(add.recheck!!.isNotBlank())
        assertTrue(add.checks.isEmpty())
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

    /** Spec 049: the sheets carry the wire keys and LIVE examples; the rows say the current rendering. */
    @Test
    fun `the format sheets are live examples keyed by preset`() {
        val saved = Formats.current
        Formats.current = Formats(number = NumberFormatKey.DotComma, time = TimeFormatKey.H12, locale = Locale.US)
        try {
            val prefs = PrefsView(numberFormat = NumberFormatKey.DotComma, timeFormat = TimeFormatKey.H12, loaded = true)
            val live = SettingsLive.withPreferences(base(), prefs, "en", strings, theme = "auto")

            assertEquals(listOf("auto", "comma_dot", "dot_comma", "space_comma", "indian"), live.numberSheet.rows.map { it.id })
            assertEquals("1.234.567,89", live.numberSheet.rows.single { it.selected }.label)
            assertEquals("dot_comma", live.numberSheet.rows.single { it.selected }.id)
            // 自动 shows what THIS locale resolves to, and keeps the drawn "自动 · 系统" note.
            assertEquals("1,234,567.89", live.numberSheet.rows[0].label)
            assertEquals(base().numberSheet.rows[0].note, live.numberSheet.rows[0].note)
            assertEquals("12,34,567.89", live.numberSheet.rows[4].label)
            assertEquals(listOf("auto", "h24", "h12"), live.timeSheet.rows.map { it.id })
            assertEquals("1:45 PM", live.timeSheet.rows.single { it.selected }.label)

            val rows = live.sections.flatMap { it.rows }
            assertEquals("1.234.567,89", rows.single { it.id == "number-format" }.value)
            assertEquals("1:45 PM", rows.single { it.id == "time-format" }.value)
            assertEquals("06/13/2026", rows.single { it.id == "date-format" }.value)
        } finally {
            Formats.current = saved
        }
    }

    /**
     * SR2 is THIS chain's fix: the fixture drew Polygon whichever chain the
     * home named. Save & Retry until the saved URL's probe says ok; only then
     * Done — the press that tells the balance core the chain is fixed.
     */
    @Test
    fun `the rpc fix names the failing chain and says restored only after its own save answers`() {
        val fallback = base().rpcFix
        val gnosis = row(100, "Gnosis", custom = false).copy(rpc_url = "https://rpc.gnosis.example", native_symbol = "XDAI")

        val failing = SettingsLive.rpcFix(fallback, gnosis, draft = null, saved = false, strings = strings)
        assertEquals("Gnosis", failing.name)
        assertTrue(failing.meta, failing.meta.contains("100") && failing.meta.endsWith("XDAI"))
        assertEquals("https://rpc.gnosis.example", failing.field.value)
        assertEquals(SettingsTone.Error, failing.badge.tone)
        assertEquals(false, failing.restored)

        // Typing shows the draft; a probe that was already ok before any save is not a repair.
        val healthy = gnosis.copy(rpc_health = NetProbeHealth.Ok(120))
        assertEquals("https://new.example", SettingsLive.rpcFix(fallback, healthy, "https://new.example", saved = false, strings = strings).field.value)
        assertEquals(false, SettingsLive.rpcFix(fallback, healthy, null, saved = false, strings = strings).restored)

        // Saved and still probing: neither offline nor restored.
        val checking = SettingsLive.rpcFix(fallback, gnosis.copy(rpc_health = NetProbeHealth.Checking), null, saved = true, strings = strings)
        assertEquals(SettingsTone.Neutral, checking.badge.tone)
        assertEquals(false, checking.restored)

        val restored = SettingsLive.rpcFix(fallback, healthy, null, saved = true, strings = strings)
        assertTrue(restored.restored)
        assertEquals(SettingsTone.Ok, restored.badge.tone)
        assertEquals(strings.t(app.getvela.wallet.core.i18n.I18nKeys.SettingsUi.COMMON_DONE), restored.primary)
        assertTrue(restored.providers.isEmpty())
        assertNull(restored.report)
    }

    /**
     * Spec 071/075, and the founder's 2026-09-26 ruling: Settings keeps the
     * Trusted Signer page and nothing else about signing — there is no default
     * "Sign with", because the account signs with the key it signed in with.
     */
    @Test
    fun `settings keeps the Trusted Signer page and no default sign-with`() {
        val view = app.getvela.wallet.feature.settings.core.SignPrefView()
        val model = SettingsLive.withSignPref(base(), view, strings)
        val rows = model.sections.flatMap { it.rows }

        assertFalse(
            "no row says \"${strings.t("settings.signing.title")}\"",
            rows.any { it.title == strings.t("settings.signing.title") || it.subtitle == strings.t("settings.signing.subtitle") },
        )

        fun rowValue(id: String) = rows.single { it.id == id }.value
        assertEquals(strings.t("settings.signing.pageOfficial"), rowValue(SettingsFixtures.SIGNER_PAGE_ROW))
        assertEquals(strings.t("settings.signing.pageTitle"), rows.single { it.id == SettingsFixtures.SIGNER_PAGE_ROW }.title)
        val chosen = SettingsLive.withSignPref(
            base(),
            view.copy(signer_url = "https://sign.example.test/", signer_url_is_default = false),
            strings,
        )
        assertEquals("sign.example.test", chosen.sections.flatMap { it.rows }.single { it.id == SettingsFixtures.SIGNER_PAGE_ROW }.value)
        // The pairing service is gone with the channel (owner, 2026-09-23):
        // no row, no sheet, and nothing in the advanced block that names one.
        val ids = model.sections.flatMap { it.rows }.map { it.id }
        assertFalse(ids.any { it.contains("tunnel", ignoreCase = true) })
    }
}
