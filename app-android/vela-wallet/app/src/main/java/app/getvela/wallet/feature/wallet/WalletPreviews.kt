package app.getvela.wallet.feature.wallet

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.tooling.preview.Preview
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.core.i18n.VelaStrings

/**
 * Preview-only translation fake (never shipped): the tooling process cannot
 * load the native engine (same pattern as WelcomePreviews' PreviewStrings).
 * Sample copy mirrors locales/zh — the corpus stays the single source of truth
 * for the real app. The identicon falls back to a plain circle in previews
 * (IdenticonImage's tooling guard).
 */
private object WalletPreviewStrings : VelaStrings {
    private val sample = mapOf(
        I18nKeys.Wallet.NAV_WALLET to "钱包",
        I18nKeys.Wallet.NAV_CONTACTS to "通讯录",
        I18nKeys.Wallet.NAV_EXPLORE to "探索",
        I18nKeys.Wallet.NAV_SETTINGS to "设置",
        I18nKeys.Wallet.TOTAL_BALANCE to "总余额",
        I18nKeys.Wallet.LIVE_INDICATOR to "实时 · 监听收款中",
        I18nKeys.Wallet.BALANCE_STALE to "部分余额仍在更新。",
        I18nKeys.Wallet.BALANCE_UNPRICED to "部分代币无法获取价格。",
        I18nKeys.Wallet.NO_PRICE to "无价格",
        I18nKeys.Wallet.A11Y_HIDE_BALANCE to "隐藏余额",
        I18nKeys.Wallet.A11Y_SHOW_BALANCE to "显示余额",
        I18nKeys.Wallet.SECTION_ACTIVITY to "活动",
        I18nKeys.Wallet.EMPTY_NO_ACTIVITY to "暂无交易记录",
        I18nKeys.Wallet.EMPTY_ACTIVITY_SUBTITLE to "你发出的交易会显示在这里，收到的钱会计入余额。",
        I18nKeys.Wallet.SECTION_ASSETS to "资产",
        I18nKeys.Wallet.ASSETS_ADD to "添加",
        I18nKeys.Wallet.ASSETS_EMPTY_TITLE to "存入您的第一笔资产",
        I18nKeys.Wallet.ASSETS_EMPTY_SUBTEXT to "点击此处查看地址并接收代币",
        I18nKeys.Wallet.ACTION_RECEIVE to "收款",
        I18nKeys.Wallet.ACTION_SEND to "转账",
        I18nKeys.Wallet.ACTION_SCAN to "扫码",
        I18nKeys.Wallet.FILTER_ALL to "全部",
        I18nKeys.Wallet.LABEL_SENT to "已发送",
        I18nKeys.Wallet.LABEL_RECEIVED to "已收到",
        I18nKeys.Wallet.LABEL_DAPP_TX to "dApp 交易",
        I18nKeys.Wallet.TO_NAME to "至 {{name}}",
        I18nKeys.Wallet.FROM_NAME to "来自 {{name}}",
        I18nKeys.Wallet.DAY_TODAY to "今天",
        I18nKeys.Wallet.DAY_YESTERDAY to "昨天",
        I18nKeys.Wallet.PILL_ALL to "全部网络",
        I18nKeys.Wallet.SELECT_CHAIN to "选择链",
        I18nKeys.Wallet.ALL_NETWORKS to "所有网络",
        I18nKeys.Wallet.QR_CAPTION to "演示占位图案 · 不可扫描",
        I18nKeys.Wallet.COPY_ADDRESS to "复制地址",
    )

    override fun t(key: String): String = sample[key] ?: key.substringAfterLast('.')

    override fun t(key: String, vars: Map<String, String>): String =
        vars.entries.fold(t(key)) { acc, (name, value) -> acc.replace("{{$name}}", value) }

    /** Previews carry one sample per key, so a plural key is its own sample. */
    override fun t(key: String, count: Int): String = t(key, mapOf("count" to count.toString()))

    /** Previews have no engine and no clock: one sample. */
    override fun relativeTime(tsSeconds: Long, nowMs: Long, utcOffsetMinutes: Int, dateFormat: String): String = "2m"
}

@Composable
private fun WalletPreviewContent(darkTheme: Boolean) {
    VelaTheme(darkTheme = darkTheme) {
        CompositionLocalProvider(LocalVelaStrings provides WalletPreviewStrings) {
            WalletScreen(
                model = WalletFixtures.buildMobileState(WalletScreenState.H1, WalletPreviewStrings),
            )
        }
    }
}

@Preview(name = "Wallet home — dark (H1)")
@Composable
private fun WalletPreviewDark() {
    WalletPreviewContent(darkTheme = true)
}

@Preview(name = "Wallet home — light (H1, token-derived)")
@Composable
private fun WalletPreviewLight() {
    WalletPreviewContent(darkTheme = false)
}
