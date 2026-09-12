package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.feature.wallet.AssetFiatModel
import app.getvela.wallet.feature.wallet.BalanceStateKind
import app.getvela.wallet.feature.wallet.BalanceStatusKind
import app.getvela.wallet.feature.wallet.NetworkPillModel
import app.getvela.wallet.feature.wallet.SectionMode
import app.getvela.wallet.feature.wallet.WalletFixtures
import app.getvela.wallet.feature.wallet.WalletScreenState
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Spec 015 FR-012: the fixture builder must reproduce the mock content
 * VERBATIM once merged with the zh corpus (the mocks are zh renderings).
 * Runs on the real engine via the host dylib, same as I18nEngineSmokeTest.
 */
class WalletFixturesTest {

    private val repoRoot = File(
        System.getProperty("vela.repo.root")
            ?: error("vela.repo.root not set — run via Gradle (testOptions wires it)"),
    )

    private fun zhStrings(): I18nRuntime = I18nRuntime { tag ->
        File(repoRoot, "assets/i18n/$tag.json").readBytes()
    }.apply { initialize("zh") }

    @Test
    fun h1MatchesTheMockVerbatim() {
        val model = WalletFixtures.buildMobileState(WalletScreenState.H1, zhStrings())

        assertEquals("大表哥", model.header.name)
        assertEquals("0x14fB1f…D1eA5c", model.header.addressDisplay)
        assertEquals("0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c", model.header.identiconSeed)
        assertEquals("全部网络", (model.pill as NetworkPillModel.All).label)

        assertEquals("总余额", model.balance.label)
        assertEquals("$1,383", model.balance.integer)
        assertEquals("28", model.balance.decimals)

        assertEquals("收款", model.actions.receive)
        assertEquals("转账", model.actions.send)
        assertEquals("扫码", model.actions.scan)

        assertEquals(listOf("今天", "昨天"), model.activityGroups.map { it.label })
        val sent = model.activityGroups[0].rows[0]
        assertEquals("已发送", sent.title)
        assertEquals("至 hold on", sent.subtitle)
        assertEquals("−2", sent.amount)
        assertEquals("POL", sent.unit)
        assertTrue(!sent.positive)
        val received = model.activityGroups[0].rows[1]
        assertEquals("已收到", received.title)
        assertEquals("来自 0x9F3c…21aE", received.subtitle)
        assertEquals("+120", received.amount)
        assertEquals("USDT", received.unit)
        assertTrue(received.positive)
        val dapp = model.activityGroups[0].rows[2]
        assertEquals("dApp 交易", dapp.title)
        assertEquals("PancakeSwap · BNB Chain", dapp.subtitle)

        assertEquals("活动", model.activitySection.title)
        assertEquals("资产", model.assetsSection.title)
        assertEquals("全部", model.assetsSection.action)
        assertEquals(6, model.assetRows.size)
        assertEquals("BNB", model.assetRows[0].ticker)
        assertEquals("0.8533", model.assetRows[0].balance)
        assertEquals(AssetFiatModel.Value("$496.46"), model.assetRows[0].fiat)

        assertEquals("钱包", model.tabs.wallet)
        assertEquals("通讯录", model.tabs.contacts)
    }

    @Test
    fun h4CarriesTheUnpricedWarningAndCakeRow() {
        val model = WalletFixtures.buildMobileState(WalletScreenState.H4, zhStrings())
        assertEquals("46", model.balance.decimals)
        assertEquals(BalanceStatusKind.Warning, model.balance.status?.kind)
        assertEquals("部分代币无法获取价格。", model.balance.status?.text)
        val cake = model.assetRows.last()
        assertEquals("CAKE", cake.ticker)
        assertEquals("18.20", cake.balance)
        assertEquals(AssetFiatModel.NoPrice("无价格"), cake.fiat)
        // H4 shows only the first two activity rows (mock).
        assertEquals(1, model.activityGroups.size)
        assertEquals(2, model.activityGroups[0].rows.size)
    }

    @Test
    fun h5MasksAmountsButKeepsUnits() {
        val model = WalletFixtures.buildMobileState(WalletScreenState.H5, zhStrings())
        assertEquals(BalanceStateKind.Hidden, model.balance.state)
        assertEquals("••••••", model.balance.integer)
        assertEquals(null, model.balance.decimals)
        val rows = model.activityGroups.flatMap { it.rows }
        assertTrue(rows.all { it.amount == "••••" })
        assertTrue(rows.all { it.unit.isNotEmpty() })
        // Received rows keep the success color while masked (data-model).
        assertTrue(rows.first { it.unit == "USDT" }.positive)
        assertTrue(model.assetRows.all { it.fiat == AssetFiatModel.Masked && it.balance == "••••" })
    }

    @Test
    fun h6IsRefreshingOnCachedTotals() {
        val model = WalletFixtures.buildMobileState(WalletScreenState.H6, zhStrings())
        assertEquals(BalanceStatusKind.Refreshing, model.balance.status?.kind)
        assertEquals("部分余额仍在更新。", model.balance.status?.text)
        assertEquals("$1,383", model.balance.integer)
    }

    @Test
    fun extremeStatesUseTheLongFixtures() {
        val model = WalletFixtures.buildMobileState(WalletScreenState.H7, zhStrings())
        assertEquals("这是一个非常长", model.header.name)
        assertEquals("BNB Chain", (model.pill as NetworkPillModel.Single).label)
        assertEquals("$1,234,567", model.balance.integer)
        assertEquals("89", model.balance.decimals)
        val rows = model.activityGroups.flatMap { it.rows }
        assertEquals("−1234.5678", rows[0].amount)
        assertEquals("至 Alexandra", rows[0].subtitle)
        assertEquals("−0.0000001", rows[1].amount)
        assertEquals("app.uniswap.org · BNB", rows[1].subtitle)
        assertEquals("1,234,567.8901", model.assetRows[1].balance)
        assertEquals("以太坊主网 Ethereum", model.assetRows[0].chain)
        assertEquals(1f, model.textScale, 0f)

        val scaled = WalletFixtures.buildMobileState(WalletScreenState.H7X, zhStrings())
        assertEquals(1.35f, scaled.textScale, 0f)
    }

    @Test
    fun h2AndH3AreEmptyAndLoading() {
        val empty = WalletFixtures.buildMobileState(WalletScreenState.H2, zhStrings())
        assertEquals(BalanceStateKind.ZeroLive, empty.balance.state)
        assertEquals("$0", empty.balance.integer)
        assertEquals("实时 · 监听收款中", empty.balance.liveText)
        assertEquals(SectionMode.Empty, empty.activitySection.mode)
        assertEquals("暂无交易记录", empty.activitySection.empty?.title)
        assertEquals("存入您的第一笔资产", empty.assetsSection.empty?.title)
        assertTrue(empty.assetRows.isEmpty())

        val loading = WalletFixtures.buildMobileState(WalletScreenState.H3, zhStrings())
        assertEquals(BalanceStateKind.Loading, loading.balance.state)
        assertEquals(null, loading.balance.integer)
        assertEquals(SectionMode.Loading, loading.activitySection.mode)
    }

    @Test
    fun h8SheetListsAllNetworksFirstWithCounts() {
        val model = WalletFixtures.buildMobileState(WalletScreenState.H8, zhStrings())
        val sheet = model.sheet ?: error("H8 must carry the chain sheet")
        assertEquals("选择链", sheet.title)
        assertEquals(7, sheet.rows.size)
        assertEquals("所有网络", sheet.rows[0].name)
        assertEquals(8, sheet.rows[0].count)
        assertTrue(sheet.rows[0].selected)
        assertEquals(
            listOf("BNB Chain" to 1, "Ethereum" to 3, "Arbitrum" to 1, "Gnosis" to 1, "Base" to 1, "Polygon" to 1),
            sheet.rows.drop(1).map { it.name to it.count },
        )
    }
}
