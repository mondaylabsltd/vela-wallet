package app.getvela.wallet.feature.settings

import androidx.compose.runtime.Immutable
import app.getvela.wallet.feature.wallet.TabsModel

/**
 * Settings view models (spec 023 — the Android port of the canonical shapes;
 * web's `src/lib/settings/model.ts` is the sibling).
 *
 * Display-ready only: pre-formatted counts, pre-composed meta lines, resolved
 * labels. Nothing here reads a preference store, probes an RPC or measures
 * storage — a later "real settings" feature swaps the fixture layer that builds
 * these and touches no component.
 *
 * The forty mocks in `design/settings/` are a small vocabulary re-dealt, which
 * is why this file is short: a row, a segmented control, a select row, a status
 * pill, a callout, a URL field and a confirm sheet cover almost all of them.
 */

/** Mobile gallery inventory — one id per mock in `design/settings/`. */
enum class SettingsScreenState {
    ST1, ST1B, ST2, ST3, ST3B, ST4, ST5, ST6, ST7, ST8,
    ST9, ST9B, ST10, ST10B, ST10C, ST11, ST12, ST13, ST13B, ST14, ST15, ST16,
    SR1, SR2, SR2B, SR3, SR4, SR5,
}

/** Which page the settings surface is showing (`Home` plus the pushed pages). */
enum class SettingsPage {
    Home, Networks, NetworkDetail, AddNetwork, RpcProviders, Endpoints, Storage, About,
}

/** Which sheet is over it. `None` is a real state, not an absence of one. */
enum class SettingsOverlay {
    None, Accounts, SignOut, Language, Currency, NumberFormat, DateFormat, TimeFormat,
    ClearCaches, EraseDevice, Feedback, RpcFix, BalanceDetail, Relayer,
}

/** Status-pill tone. `Neutral` is unset/idle, not failed. */
enum class SettingsTone { Ok, Warn, Error, Neutral }

@Immutable
data class StatusPillModel(
    val tone: SettingsTone,
    val label: String,
    val dot: Boolean = true,
)

/** Callout tone. `Success` swaps the triangle for a check. */
enum class CalloutTone { Warning, Danger, Info, Success }

@Immutable
data class CalloutModel(val tone: CalloutTone, val text: String)

/** Which glyph a settings row draws (models stay UI-type free). */
enum class SettingsIcon {
    Contacts, Feedback, Globe, Coins, Hash, Calendar, Clock,
    Network, Server, Plus, Zap, HardDrive, Info, Sun, Moon, Monitor,
}

/** Row emphasis. `Danger` is the red 退出登录 / 清理数据 family. */
enum class RowTone { Default, Accent, Danger }

/** What sits at the end of a settings row. */
enum class RowTrailing { Chevron, External, None }

@Immutable
data class SettingsRowModel(
    /** Action-sink id — routed by the screen, never by the component. */
    val id: String,
    val title: String,
    val icon: SettingsIcon? = null,
    val subtitle: String? = null,
    /** Right-aligned current value — "简体中文 · 系统", "12 个网络". */
    val value: String? = null,
    val trailing: RowTrailing = RowTrailing.Chevron,
    val tone: RowTone = RowTone.Default,
)

@Immutable
data class SettingsSectionModel(
    val rows: List<SettingsRowModel>,
    val label: String? = null,
    /** ST1b: 高级 is a disclosure, and it remembers being open. */
    val collapsible: Boolean = false,
    val collapsed: Boolean = false,
    /**
     * ST1: the appearance block ends in three CONTROLS rather than rows. The
     * flag says so in the data, instead of the screen counting indices.
     */
    val appearanceControls: Boolean = false,
)

/** ST1's identity block: avatar, name, address, and a trailing text action. */
@Immutable
data class AccountRowModel(
    val name: String,
    val addressDisplay: String,
    /** Full address — the identicon seed; never lowercased at a call site. */
    val addressFull: String,
    val action: String,
)

@Immutable
data class SegmentModel(val id: String, val label: String, val icon: SettingsIcon? = null)

@Immutable
data class SegmentedModel(
    val label: String,
    val segments: List<SegmentModel>,
    val selected: String,
)

/** The A ——●—— A slider. */
@Immutable
data class TextScaleModel(val label: String, val steps: Int, val index: Int)

/** One choice in a picker (语言/货币/数字/日期/时间). */
@Immutable
data class SelectRowModel(
    val id: String,
    val label: String,
    /** Right-aligned note — "系统 · 简体中文", "印度计数". */
    val note: String? = null,
    /** Leading circular badge — the currency sheet's ¥ / $ / €. */
    val glyph: String? = null,
    /** Secondary label after the primary one — the currency sheet's 美元. */
    val caption: String? = null,
    val selected: Boolean = false,
    /** Mono face — every number/date/time sample wants it. */
    val mono: Boolean = false,
)

@Immutable
data class SelectSheetModel(
    val title: String,
    val rows: List<SelectRowModel>,
    val subtitle: String? = null,
    val searchPlaceholder: String? = null,
    val footerNote: String? = null,
    val footerLink: String? = null,
)

@Immutable
data class AccountsSheetRowModel(
    val name: String,
    val addressDisplay: String,
    val addressFull: String,
    val amount: String,
    val selected: Boolean,
)

@Immutable
data class AccountsSheetModel(
    val title: String,
    /** "3 个账户 · 总计 $3,262.40". */
    val summary: String,
    val rows: List<AccountsSheetRowModel>,
    val primary: String,
    val secondary: String,
)

/** ST3 / ST13b / ST16 share this; only the tone and the callout differ. */
@Immutable
data class ConfirmSheetModel(
    val title: String,
    val body: String,
    val confirm: String,
    val cancel: String,
    val danger: Boolean,
    /** Second, quieter paragraph — the sign-out sheet's "keeps" line. */
    val note: String? = null,
    val callout: CalloutModel? = null,
)

/** A chain's circular avatar: a letter over a fixture-supplied brand colour. */
@Immutable
data class ChainMarkModel(val letter: String, val colorArgb: Long, /** Spec 047: the chain's logo from the chain-data endpoint; the letter is the fallback. */ val logoUrl: String? = null)

@Immutable
data class NetworkRowModel(
    val id: String,
    val mark: ChainMarkModel,
    val name: String,
    /** "链 1" — the chain-id line under the name. */
    val meta: String,
    val badge: StatusPillModel? = null,
    /** ST9: custom networks carry a 自定义 tag and a bin. */
    val tag: String? = null,
    val removable: Boolean = false,
)

@Immutable
data class UrlFieldModel(
    val id: String,
    val label: String,
    val value: String,
    val placeholder: String? = null,
    val hint: String? = null,
    val badge: StatusPillModel? = null,
    val tone: SettingsTone? = null,
)

@Immutable
data class NetworkDetailModel(
    val title: String,
    /** "链 1 · ETH". */
    val subtitle: String,
    val mark: ChainMarkModel,
    val name: String,
    val note: String,
    val badge: StatusPillModel,
    val rpc: UrlFieldModel,
    val explorer: UrlFieldModel,
    val callout: CalloutModel? = null,
)

@Immutable
data class CheckItemModel(val label: String, val ok: Boolean)

@Immutable
data class AddNetworkModel(
    val title: String,
    val subtitle: String,
    val searchPlaceholder: String,
    /** What is typed in the search box — the core's, so a keystroke round-trips. */
    val query: String = "",
    val results: List<NetworkRowModel> = emptyList(),
    val candidate: NetworkRowModel? = null,
    val checksTitle: String? = null,
    val checks: List<CheckItemModel> = emptyList(),
    val customRpc: UrlFieldModel? = null,
    val callout: CalloutModel? = null,
    val primary: String? = null,
    val secondary: String? = null,
    val recheck: String? = null,
)

@Immutable
data class ProviderCardModel(
    val id: String,
    val name: String,
    val badge: StatusPillModel,
    val field: UrlFieldModel,
    /** The blue action inside the field — 检查密钥 / 获取密钥. */
    val action: String,
    val support: String? = null,
    val link: String? = null,
)

@Immutable
data class RpcProvidersModel(
    val title: String,
    val subtitle: String,
    val description: String,
    val providers: List<ProviderCardModel>,
)

@Immutable
data class EndpointsModel(
    val title: String,
    val description: String,
    val fields: List<UrlFieldModel>,
    val reset: String,
)

@Immutable
data class StorageSegmentModel(val id: String, val label: String, val fraction: Float, val colorArgb: Long)

@Immutable
data class StorageItemModel(
    val id: String,
    val label: String,
    /** "200 条 · 1.0 MB" — already joined by the fixture layer. */
    val meta: String,
    val action: String,
    val destructive: Boolean = false,
)

@Immutable
data class StorageGroupModel(
    val label: String,
    val items: List<StorageItemModel>,
    /** The 清除全部缓存 link under the cache group. */
    val action: String? = null,
)

@Immutable
data class StorageModel(
    val title: String,
    val subtitle: String,
    /** "2.4" and "MB", split so the number can carry the display type. */
    val amount: String,
    val unit: String,
    val summary: String,
    val segments: List<StorageSegmentModel>,
    val groups: List<StorageGroupModel>,
)

@Immutable
data class KeyValueRowModel(
    val label: String,
    val value: String,
    val mono: Boolean = false,
    val external: Boolean = false,
)

@Immutable
data class AboutModel(
    val title: String,
    val tagline: String,
    val version: String,
    val sectionTechnical: String,
    val rows: List<KeyValueRowModel>,
    val links: List<KeyValueRowModel>,
    val footer: String,
)

@Immutable
data class FeedbackModel(
    val title: String,
    val subtitle: String,
    val placeholder: String,
    val addSteps: String,
    val previewToggle: String,
    val previewLines: List<String>,
    val consent: String,
    val send: String,
    val githubLink: String,
)

/** SR1: the amber "these networks are down" banner and its per-chain fixes. */
@Immutable
data class RpcBannerChipModel(val id: String, val mark: ChainMarkModel, val name: String, val action: String)

@Immutable
data class RpcBannerModel(val text: String, val chips: List<RpcBannerChipModel>)

@Immutable
data class RpcFixModel(
    val title: String,
    val mark: ChainMarkModel,
    val name: String,
    /** "链 137 · POL". */
    val meta: String,
    val badge: StatusPillModel,
    val callout: CalloutModel,
    val field: UrlFieldModel,
    val primary: String,
    val providersLabel: String? = null,
    val providers: List<String> = emptyList(),
    val report: String? = null,
)

/** SR3: the quiet rate-limited balance breakdown. */
@Immutable
data class BalanceDetailRowModel(
    val id: String,
    val mark: ChainMarkModel,
    val name: String,
    val status: String? = null,
    val tone: SettingsTone = SettingsTone.Neutral,
    val action: String? = null,
    val amount: String? = null,
)

@Immutable
data class BalanceDetailModel(
    val title: String,
    val summary: String,
    val sectionPending: String,
    val pendingNote: String,
    val pending: List<BalanceDetailRowModel>,
    val sectionDone: String,
    val done: List<BalanceDetailRowModel>,
)

/** SR4: fund this chain's bundler treasury. */
@Immutable
data class RelayerModel(
    val title: String,
    val lead: String,
    val mark: ChainMarkModel,
    val name: String,
    val amountHint: String,
    val qrCaption: String,
    val addressDisplay: String,
    val copyLabel: String,
    val callout: CalloutModel,
    val primary: String,
)

/** SR5: the passkey index is unreachable, and onboarding needs it. */
@Immutable
data class IndexDownModel(
    val title: String,
    val subtitle: String,
    val callout: CalloutModel,
    val field: UrlFieldModel,
    val primary: String,
    val secondary: String,
    val footer: String,
)

/** Everything one settings state needs. */
@Immutable
data class SettingsScreenModel(
    val state: SettingsScreenState,
    val title: String,
    val page: SettingsPage,
    val overlay: SettingsOverlay,
    /** Which tab the bottom bar highlights — 钱包 for the SR rescue states. */
    val selectedTab: String,
    val tabs: TabsModel,
    val account: AccountRowModel,
    val sections: List<SettingsSectionModel>,
    val theme: SegmentedModel,
    val avatar: SegmentedModel,
    val textScale: TextScaleModel,
    val signOutLabel: String,
    val eraseTitle: String,
    val eraseSubtitle: String,
    val networksTitle: String,
    val networksSubtitle: String,
    val networks: List<NetworkRowModel>,
    val addNetworkLabel: String,
    val networkDetail: NetworkDetailModel,
    val addNetwork: AddNetworkModel,
    val rpcProviders: RpcProvidersModel,
    val endpoints: EndpointsModel,
    val storage: StorageModel,
    val about: AboutModel,
    val accountsSheet: AccountsSheetModel,
    val signOutSheet: ConfirmSheetModel,
    val languageSheet: SelectSheetModel,
    val currencySheet: SelectSheetModel,
    val numberSheet: SelectSheetModel,
    val dateSheet: SelectSheetModel,
    val timeSheet: SelectSheetModel,
    val clearCachesSheet: ConfirmSheetModel,
    val eraseSheet: ConfirmSheetModel,
    val feedback: FeedbackModel,
    val rpcBanner: RpcBannerModel?,
    val rpcFix: RpcFixModel,
    val balanceDetail: BalanceDetailModel,
    val relayer: RelayerModel,
    val indexDown: IndexDownModel,
    /** Scrim title behind a rescue sheet — "钱包", "转账", "设备存储". */
    val backdropTitle: String,
    val closeLabel: String,
) {
    /** The signed-in identity, swapped over the fixture account (spec 019). */
    fun withIdentity(name: String, address: String, display: String): SettingsScreenModel = copy(
        account = account.copy(name = name, addressFull = address, addressDisplay = display),
        accountsSheet = accountsSheet.copy(
            rows = accountsSheet.rows.mapIndexed { index, row ->
                // Only the ACTIVE row: the other two are fixtures, and there is
                // no honest way to make them real without an account list the
                // core does not expose yet.
                if (index == 0) {
                    row.copy(name = name, addressFull = address, addressDisplay = display)
                } else {
                    row
                }
            },
        ),
    )
}
