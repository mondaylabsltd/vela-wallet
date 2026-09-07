package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.wallet.AssetFiatModel
import app.getvela.wallet.feature.wallet.BalanceStateKind
import app.getvela.wallet.feature.wallet.BalanceStatusKind
import app.getvela.wallet.feature.wallet.SectionMode
import app.getvela.wallet.feature.wallet.WalletFixtures
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.wallet.WalletScreenState
import app.getvela.wallet.feature.wallet.core.BalanceToken
import app.getvela.wallet.feature.wallet.core.BalanceView
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The live wallet-home builders.
 *
 * Every case here is a way a money screen can lie, and each one was written
 * because the screen told that lie at least once. The most expensive was the
 * last: a real phone, two real holdings, and a confident "$0.00" above them.
 */
class WalletLiveTest {

    private val strings: VelaStrings by lazy {
        val root = System.getProperty("vela.repo.root")
            ?: error("vela.repo.root not set — run via Gradle (testOptions wires it)")
        I18nRuntime { tag -> File(root, "public/i18n/$tag.json").readBytes() }
            .apply { initialize("en") }
    }

    private fun base() = WalletFixtures.buildMobileState(WalletScreenState.H1, strings)

    /** The chain names a device would have, from its network list. */
    private val chains = mapOf(137 to "Polygon", 42161 to "Arbitrum")

    private fun home(view: BalanceView) = WalletLive.home(base(), view, strings, chains)

    private fun token(
        symbol: String,
        balance: String,
        price: Double? = null,
        chainId: Int = 137,
        name: String = symbol,
    ) = BalanceToken(
        chain_id = chainId,
        symbol = symbol,
        // The TOKEN's name — "Ether", "USDT" — not the chain's. They were the
        // same string for as long as the shell was writing chain names into
        // this field, which is what hid the bug below.
        name = name,
        balance = balance,
        decimals = 18,
        price_usd = price,
    )

    /**
     * A row says which chain a holding is on.
     *
     * Reading that off `BalanceToken.name` looked correct for a whole phase,
     * because the shell was putting chain names in it. The moment real token
     * names arrived from the chain documents, every row read "USDT / USDT" and
     * "ETH / Ether" — the one thing the second line is for, gone.
     */
    @Test
    fun `an asset row names the chain, not the token again`() {
        val view = BalanceView(
            display_total_usd = 5.0,
            tokens = listOf(
                token("ETH", "0.002", price = 2500.0, chainId = 42161, name = "Ether"),
                token("USDT", "0.01", price = 1.0, chainId = 137, name = "USDT"),
            ),
        )

        val rows = home(view).assetRows

        assertEquals("Arbitrum", rows[0].chain)
        assertEquals("Polygon", rows[1].chain)
    }

    /** A chain this device has no row for still says something, never a blank. */
    @Test
    fun `an unknown chain falls back to the token's own name`() {
        val view = BalanceView(
            display_total_usd = 1.0,
            tokens = listOf(token("MON", "1", price = 1.0, chainId = 143, name = "Monad")),
        )

        assertEquals("Monad", home(view).assetRows.single().chain)
    }

    /**
     * The device bug, pinned.
     *
     * The core folds an unpriced holding in at zero, so a wallet whose every
     * coin is unpriced totals exactly 0.0 — indistinguishable, at the type
     * level, from a wallet that is genuinely empty. Rendering that as "$0.00"
     * told a person holding POL and ETH that they had nothing.
     */
    @Test
    fun `every holding unpriced shows no total at all, never a zero`() {
        val view = BalanceView(
            display_total_usd = 0.0,
            tokens = listOf(token("POL", "0.152784"), token("ETH", "0.002")),
        )

        val model = home(view)

        assertEquals(BalanceStateKind.Loading, model.balance.state)
        assertNull(model.balance.integer)
        assertNull(model.balance.decimals)
    }

    /**
     * And it says why.
     *
     * The H1 fixture carries no status line, so borrowing one with `?.copy`
     * produced a bare skeleton: an empty hero, two holdings under it, and no
     * explanation. The warning has to be built, not inherited.
     */
    @Test
    fun `the empty hero explains itself`() {
        val view = BalanceView(display_total_usd = 0.0, tokens = listOf(token("POL", "0.152784")))

        val status = home(view).balance.status

        assertNotNull("an empty hero with no reason given is the bug", status)
        assertEquals(BalanceStatusKind.Warning, status!!.kind)
        assertTrue(status.text.isNotBlank())
    }

    /** A zero that IS zero keeps its figure — the states must stay distinct. */
    @Test
    fun `a genuinely empty wallet shows zero`() {
        val model = home(BalanceView(display_total_usd = 0.0))

        assertEquals(BalanceStateKind.ZeroLive, model.balance.state)
        assertEquals("$0", model.balance.integer)
    }

    /** One priced holding is enough for a total; the rest show "—" in their rows. */
    @Test
    fun `a partly priced wallet totals what it could price`() {
        val view = BalanceView(
            display_total_usd = 4.5,
            tokens = listOf(token("POL", "10", price = 0.45), token("ETH", "0.002")),
        )

        val model = home(view)

        assertEquals(BalanceStateKind.Normal, model.balance.state)
        assertEquals("$4", model.balance.integer)
        assertEquals("50", model.balance.decimals)
        assertEquals(AssetFiatModel.Value("$4.50"), model.assetRows[0].fiat)
        assertEquals(AssetFiatModel.NoPrice("—"), model.assetRows[1].fiat)
    }

    /**
     * Nothing held is not the same as nothing read yet.
     *
     * Both leave the asset list empty; only one of them should say "no assets".
     */
    @Test
    fun `an empty list while loading is not an empty wallet`() {
        val loading = home(BalanceView(holdings_loading = true))
        val settled = home(BalanceView())

        assertEquals(SectionMode.Loading, loading.assetsSection.mode)
        assertEquals(SectionMode.Empty, settled.assetsSection.mode)
    }

    /** Amounts are truncated, never rounded: a rounded-up balance is money nobody has. */
    @Test
    fun `a long balance is cut, not rounded up`() {
        val view = BalanceView(
            display_total_usd = 1.0,
            tokens = listOf(token("POL", "0.1234569999", price = 1.0)),
        )

        assertEquals("0.123456 POL", home(view).assetRows[0].balance)
    }

    /** Hidden hides the figure and nothing else. */
    @Test
    fun `hidden keeps the holdings, drops the number`() {
        val view = BalanceView(
            display_total_usd = 12.0,
            hidden = true,
            tokens = listOf(token("POL", "10", price = 1.2)),
        )

        val model = home(view)

        assertEquals(BalanceStateKind.Hidden, model.balance.state)
        assertEquals(1, model.assetRows.size)
    }
}
