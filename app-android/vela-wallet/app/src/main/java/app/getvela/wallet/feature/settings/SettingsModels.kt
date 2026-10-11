package app.getvela.wallet.feature.settings

import androidx.compose.runtime.Immutable
import app.getvela.wallet.core.data.DebugMode
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

/**
 * Mobile gallery inventory — one id per mock in `design/settings/`, and
 * ST14B (spec 091, no mock): About with the debug-mode switch revealed.
 */
enum class SettingsScreenState {
    ST1, ST1B, ST2,

    /**
     * The account switcher with ONE account, through the live builder (PR 3
     * final note F15): "1 account · Total …" — the count is a plural family
     * and the core picks its form. It read "1 accounts · Total".
     */
    ST2B,
    ST3, ST3B, ST4, ST5, ST6, ST7, ST8,
    ST9, ST9B, ST10, ST10B, ST10C,
    /**
     * Add network, refused for the OTHER reason: no P-256 verifier. ST10C is
     * the chain whose contracts are missing (Chain Setup, opened on it); here
     * nothing can be deployed, so the line says Vela wallets cannot work on
     * this network — and there is no button.
     */
    ST10D,

    /**
     * The wizard's STOPS, each through the live builder from a view the real
     * `network_admin` machine wrote (`NetBoards`), so a board says the core's
     * own sentence (`NetWizardView.error_key`): the scan path — a chain added
     * by id, with no confirm step — refused for missing contracts, the check
     * kept beside the stop, so its reason and Chain Setup show (ST10E), for
     * no P-256 verifier, nothing to deploy (ST10F), and unable to verify
     * (ST10G); a network that is already added (ST10H), one nobody could find
     * (ST10I), and one the catalog lists no RPC for — the box to type one in
     * is there (ST10J).
     */
    ST10E, ST10F, ST10G, ST10H, ST10I, ST10J,
    ST11, ST12, ST13, ST13B, ST14, ST14B, ST15, ST16,
    /**
     * Spec 102, the web's boards: "Where you review and sign" for an account
     * on `getvela.app` reviewing on the official page (ST17) and for one on
     * its own domain, locked to its page (ST17B) — and, ST17C, that page
     * redeployed: its check asks to trust the new build, and "Trust this
     * version" answers on the row; Settings → Signing pages (ST18), and with
     * a page that will not open and an address refused (ST18B).
     */
    ST17, ST17B, ST17C, ST18, ST18B,
    SR1, SR2, SR2B, SR3,

    /**
     * The balance-by-network sheet through the LIVE builder, with the
     * display currency on its way (SR3B: a cold start with CNY stored — the
     * total and each network's worth keep their room, nothing drawn in the
     * placeholder's dollars) and the SAME frame once it commits (SR3C), so
     * the pair shows that nothing moved.
     */
    SR3B, SR3C,

    /**
     * The same sheet in three rounds the REAL balance machine wrote
     * (`BalanceBoards`), through the live builder — PR 3 final notes F21, F20
     * and F16. SR3D: Tempo's token list did not load — its row's short
     * status is the core's "Token list unavailable", never "RPC unavailable"
     * over an RPC that answers. SR3E: every network answered and one token
     * has no price — the sheet a person opens from "Some tokens couldn't be
     * priced." draws NO "Networks still updating / couldn't be reached" over
     * an empty list. SR3F: Ethereum's read failed inside the app — the home's
     * line is one line and cuts that sentence, so the sheet it opens says it
     * in full at its top.
     */
    SR3D, SR3E, SR3F,
    SR4, SR5,
    /** Spec 092: every network the wallet cannot reach, in one list. */
    SR6,

    /**
     * The same list when the one network on it is there for its TOKEN LIST
     * (Tempo: its RPC answers, the document that names its stablecoins did
     * not load) — the real balance machine's view through the live builder:
     * the title says the list, and the row offers no "Fix", because there is
     * no RPC to fix.
     */
    SR7,

    /**
     * The Keys block over the home, with its "Copy this wallet's record to
     * Ethereum" row in each state the core has a row for — through the LIVE
     * builder: not copied yet (SK1, the one a tap opens the sheet from),
     * copied (SK2), couldn't check (SK3, a tap asks again) and an older
     * wallet that can never be copied (SK4, a calm end). The block was live
     * only, so none of its states could be looked at without a wallet in
     * each.
     */
    SK1, SK2, SK3, SK4,
}

/** Which page the settings surface is showing (`Home` plus the pushed pages). */
enum class SettingsPage {
    Home, Networks, NetworkDetail, AddNetwork, RpcProviders, Endpoints, Storage, About,

    /** Spec 102: the signing pages this device keeps — official first, each with its integrity line. */
    SigningPages,

    /** Spec 102: this account's "Where you review and sign". */
    Venue,
    ;

    /**
     * Where Back goes from this page — the system Back and the page's own ‹
     * alike: a pushed page goes back to the Settings home. `null` on the home,
     * whose Back is not Settings' to answer (it leaves for the wallet).
     *
     * Device pass 2026-10-09: the system Back on 设置 → 高级 → 网络 left
     * Settings for 钱包, because nothing inside Settings answered it.
     */
    val back: SettingsPage? get() = if (this == Home) null else Home

    /**
     * Whether this page keeps its scroll across a visit to another. Only the
     * home does — Back lands where the person left it; a pushed page opens at
     * its top every time (device pass 2026-10-09: 网络 opened part-way down,
     * at the offset the home had been scrolled to, because every page shared
     * the home's one scroll).
     */
    val keepsItsPlace: Boolean get() = this == Home
}

/** Which sheet is over it. `None` is a real state, not an absence of one. */
enum class SettingsOverlay {
    None, Accounts, SignOut, Language, Currency, NumberFormat, DateFormat, TimeFormat,
    ClearCaches, EraseDevice, Feedback, RpcFix, BalanceDetail, Relayer,

    /** SR6 (spec 092): every network the wallet cannot reach, each with its RPC fix. */
    Unreachable,

    /**
     * One storage row's 清除, asked before it happens (spec 058, the founder's
     * ruling of 2026-09-15): 「联系人与分组 · 清除」 removed the whole address
     * book on a single tap, with nothing in between. The question is built from
     * what the row already says — its label, its group's warning, its own
     * action word — so no new sentence is invented for it.
     */
    ClearStorageItem,

    /** The default transaction speed (spec 069): three speeds, each with what it buys. */
    FeeSpeed,

    /** Spec 102: a saved signing page's new name. */
    RenameSigningPage,

    /**
     * Removing a saved signing page asks first, as iOS does: the page's own
     * name, "Remove" and Cancel. One tap on the row's "Remove" used to take
     * it — and its trusted version with it — with nothing in between.
     */
    RemoveSigningPage,

    /** Spec 072: removing a custom network asks first. */
    RemoveNetwork,

    /** Spec 072: resetting the service endpoints asks first. */
    ResetEndpoints,
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
    Network, Server, Plus, Zap, HardDrive, Info, Sun, Moon, Monitor, Upload, LogOut,
    /** Spec 102: where you review and sign (the web's lucide `eye`), and the signing pages. */
    Eye, FileText,
    /** Settings → Community: the brands' own monochrome marks. */
    BrandX, BrandTelegram, BrandDiscord,
}

/** Row emphasis. `Danger` is the red 退出登录 / 清理数据 family. */
enum class RowTone { Default, Accent, Danger }

/**
 * What sits at the end of a settings row. `Retry` is the one that does not
 * lead anywhere: it says "ask again", for a row whose answer did not arrive.
 */
enum class RowTrailing { Chevron, External, Retry, None }

@Immutable
data class SettingsRowModel(
    /** Action-sink id — routed by the screen, never by the component. */
    val id: String,
    val title: String,
    val icon: SettingsIcon? = null,
    val subtitle: String? = null,
    /**
     * The second line says something went WELL ("Copied to Ethereum" — the
     * core's `BackupTone::Positive`) and is drawn in the success colour.
     */
    val subtitlePositive: Boolean = false,
    /** Right-aligned current value — "简体中文 · 系统", "12 个网络". */
    val value: String? = null,
    val trailing: RowTrailing = RowTrailing.Chevron,
    val tone: RowTone = RowTone.Default,
    /**
     * Whether a tap does anything. `false` takes the ripple away with the
     * chevron: the house rule is that a control which cannot act is not
     * dressed as one, and a row that presses in under the finger and returns
     * silently is exactly that (dead-controls #8). Rows are actionable by
     * default — every one of them routes somewhere — so only a row with
     * states has to say otherwise.
     */
    val actionable: Boolean = true,
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
    /**
     * A line UNDER the label — the speed sheet's "Lowest fee, if you can
     * wait" (spec 068's ruling: the name is the speed, what it buys goes
     * under it, never squeezed beside it).
     */
    val detail: String? = null,
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

/**
 * Spec 102: Settings → Signing pages — the official page first, then the ones
 * this person added, each with whose keys it reaches and its integrity line.
 * Which page an ACCOUNT signs on is that account's own choice ([VenueModel]).
 */
@Immutable
data class SigningPagesModel(
    val title: String = "",
    val subtitle: String = "",
    val rows: List<app.getvela.wallet.feature.settings.components.SigningPageItemModel> = emptyList(),
    /** "Add a page" — the field's label. */
    val add: String = "",
    val addressPlaceholder: String = "https://",
    /** The rename sheet's field label. */
    val nameLabel: String = "",
    /** A saved row's two actions. */
    val rename: String = "",
    /** `pageInvalid` / `pageInsecure` / `pageDuplicate` for the last address typed; nothing was stored. */
    val addError: String? = null,
    /** What the add field starts with (a board's refused address); the person's typing owns it after. */
    val draft: String = "",
    val save: String = "",
    val remove: String = "",
    /** The remove question's way out (`common.cancel`). */
    val cancel: String = "",
    /** The askTrust line's answer (the version is the line's). */
    val trust: String = "",
    /** The list has been read; edits are offered only then. */
    val loaded: Boolean = false,
)

/**
 * Spec 102: "Where you review and sign" — one account's signing venue on this
 * device. [row] is the settings row (its value the venue in force); the sheet
 * lists every venue the core offers ([choices], R1/R2), the blocked ones
 * dimmed with their reason. Changing it never touches a key.
 */
@Immutable
data class VenueModel(
    val row: SettingsRowModel,
    val title: String,
    val subtitle: String,
    /** "Keys on getvela.app" — the account's signing domain, said on the sheet. */
    val domainLine: String,
    val choices: List<VenueChoiceModel>,
    /** "On a trusted page" and its line — the heading over the page choices. */
    val pageSection: String,
    val pageSectionBody: String,
    /** "Signing pages" — the way to the list where pages are added. */
    val manage: String,
    /** "Trust this version" — a page choice's answer when its line asks (`settings.signing.pageTrust`). */
    val trust: String = "",
)

@Immutable
data class VenueChoiceModel(
    /** The venue as the core wrote it — handed back on a pick, never rebuilt. */
    val venueJson: String,
    val title: String,
    /** The line under the title ("Vela's own signing sheet", "A zero-dependency page …"). */
    val body: String,
    /** A page choice's own row (address, domain, integrity line); `null` for Vela's sheet. */
    val page: app.getvela.wallet.feature.settings.components.SigningPageItemModel?,
    val selected: Boolean,
    /** R1: why this venue cannot reach the account's keys; the row is disabled. */
    val reason: String?,
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
    /**
     * The words for taking ONE wallet off this device (2026-09-23). Empty
     * leaves the affordance undrawn, which is what a fixture with nothing
     * wired behind it wants — the screen resolves no strings of its own.
     */
    val remove: String = "",
    val removeBody: String = "",
    val removeCancel: String = "",
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
    /** Spec 048: which chain the overrides are written for. */
    val chainId: Long = 0,
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
    /**
     * The callout is the reason the field under it is there ("No RPC
     * endpoint is listed for this network. Enter one, then re-check.", or
     * "Unable to verify — RPC request failed"): it is drawn ABOVE
     * [customRpc] — why, then where, then the re-check — instead of under
     * it, where it read as a verdict on what was typed.
     */
    val calloutAsksForRpc: Boolean = false,
    val primary: String? = null,
    /** "Open Chain Setup Tool" — drawn only with [secondaryUrl]. */
    val secondary: String? = null,
    /** Where [secondary] goes: the core's `setup_url`, Chain Setup opened on this chain. */
    val secondaryUrl: String? = null,
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
    /** Where [link] goes: the provider's own sign-up page (the label is not a URL). */
    val linkUrl: String? = null,
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
    val debugMode: DebugModeRowModel,
)

/**
 * Spec 091: About's hidden developer switch. Drawn only once revealed
 * ([DebugMode.revealed]) — seven taps on the version, the core's rule.
 */
@Immutable
data class DebugModeRowModel(
    val title: String,
    /** The one line under the title: what it does, and that it is for development only. */
    val body: String,
    /** The notice the revealing tap shows, once. */
    val revealedNotice: String,
    val mode: DebugMode = DebugMode.Hidden,
)

/**
 * Issue #466: what a report sheet opens with when something else wrote it —
 * a relay stop's "Report this" seeds the core's `what` and `steps` (snapshot
 * at the tap). The person reads them, may edit them, and sends; the area and
 * fingerprint ride beside, never in the boxes.
 */
@Immutable
data class FeedbackSeed(val what: String, val steps: String)

@Immutable
data class FeedbackModel(
    val title: String,
    val subtitle: String,
    val placeholder: String,
    val addSteps: String,
    val previewToggle: String,
    /** Exactly the payload's `environment`, line for line (live: [BugReport.environmentLines]). */
    val previewLines: List<String>,
    val consent: String,
    val send: String,
    val githubLink: String,
    /** Spec 078 round 3: the steps box behind 「+ 添加重现步骤」. */
    val stepsPlaceholder: String = "",
    /** The button's label while the endpoint is answering. */
    val sending: String = "",
    /** Filed — the bodies carry `{{number}}`. */
    val successTitle: String = "",
    val successBodyNew: String = "",
    val successBodyDeduped: String = "",
    val viewIssue: String = "",
    /** Not filed — the prefilled form is the road that still works. */
    val fallbackTitle: String = "",
    val fallbackBody: String = "",
    val openGithub: String = "",
    /** Closes the sheet from the filed state. */
    val done: String = "",
    /** The fallback's retry (`common.tryAgain`). */
    val tryAgain: String = "",
    /** Screenshots (the founder's ask, 2026-09-26) — public on the issue, at most five. */
    val screenshotsLabel: String = "",
    val addScreenshots: String = "",
    /** "Optional · up to 5", shown while none is attached. */
    val screenshotsHint: String = "",
    /** The warning that must be visible before 发送 once one is attached. */
    val screenshotsPublic: String = "",
    /** TalkBack's label for a tile's ✕; carries `{{index}}` (1-based). */
    val removeScreenshot: String = "",
    val screenshotsLimit: String = "",
    val screenshotUnsupported: String = "",
    /** Filed, but the images could not be stored. */
    val screenshotsDropped: String = "",
    /** Not filed: the form cannot carry the images. */
    val fallbackScreenshots: String = "",
    /** TalkBack's label for a tile, which opens the viewer (078 §C); carries `{{index}}` (1-based). */
    val viewScreenshot: String = "",
    /** The viewer's ✕, for TalkBack. */
    val closeViewer: String = "",
    /** The viewer's visible remove button. */
    val removeFromViewer: String = "",
    /** Where the last 发送 stands; the gallery draws [FeedbackStatus.Idle]. */
    val status: FeedbackStatus = FeedbackStatus.Idle,
    /** An answer that arrived after the sheet was closed: the settings page's notice. */
    val notice: FeedbackNoticeModel? = null,
)

/**
 * The page's notice for a report whose sheet was closed mid-send: filed →
 * 感谢反馈，已收到 with 在 GitHub 查看; not filed → 暂时无法在应用内发送 with
 * 打开 GitHub 表单. [url] is where the action goes.
 */
@Immutable
data class FeedbackNoticeModel(val message: String, val action: String, val url: String)

/**
 * Spec 078 round 3 (the web's `FeedbackResult` + `sending`): both endings are
 * outcomes. A report the endpoint could not file still has the prefilled form,
 * and the sheet offers it rather than apologising.
 */
@Immutable
sealed interface FeedbackStatus {
    data object Idle : FeedbackStatus

    /** The endpoint is answering: the button turns a spinner and stays at full emphasis. */
    data object Sending : FeedbackStatus

    /** Filed, as a new issue or a +1 on an open one — the number is the way back to it. */
    data class Filed(val number: Long, val url: String, val deduped: Boolean, val screenshotsDropped: Int = 0) : FeedbackStatus

    /** Not filed: [url] is the prefilled GitHub form, the person's words already in it. */
    data class Fallback(val url: String) : FeedbackStatus
}

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
    /** The saved RPC answered: the primary is Done, and pressing it clears the chain's failure. */
    val restored: Boolean = false,
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
    /** [amount] is a fiat figure withheld until the display currency is the person's: its room is kept. */
    val amountWithheld: Boolean = false,
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
    /** The hero's "couldn't be priced" line, answered by name (holdings in `status`). */
    val sectionUnpriced: String = "",
    val unpriced: List<BalanceDetailRowModel> = emptyList(),
    /** The total in [summary] is withheld until the display currency is the person's: empty, its line kept. */
    val summaryWithheld: Boolean = false,
    /**
     * The sentence the home's status line said when it opened this sheet, in
     * FULL (PR 3 final note F16): the line itself is one line and ends in an
     * ellipsis when the sentence is longer, so the whole of it stands here,
     * at the top. `null` when the line says nothing.
     */
    val lead: String? = null,
)

/**
 * SR6 (spec 092): every network the wallet cannot reach, in the core's order
 * (last seen holding something first), each with what was last read there.
 */
@Immutable
data class UnreachableModel(
    /** The home's own line, live — or "every network is back" once none is. */
    val title: String = "",
    /** Absent once the list is empty. */
    val summary: String? = null,
    val rows: List<UnreachableRowModel> = emptyList(),
    /**
     * The home line's sentence in full when it is NOT [title] (F16): the line
     * said the fault's own sentence, or "Offline", over networks that are also
     * out of reach. `null` when the title already is the line.
     */
    val lead: String? = null,
)

@Immutable
data class UnreachableRowModel(
    val chainId: Int,
    val mark: ChainMarkModel,
    val name: String,
    /** "Last seen $1,234.50", "Not read yet", … */
    val line: String,
    /**
     * "Fix" — the row's way to its network's RPC editor. `null` where the
     * core says there is no RPC to fix (`UnreachableNetwork.rpc_fixable`: the
     * network answers, its token list is what could not be loaded): no
     * action is drawn.
     */
    val action: String?,
    /** [line] carries a worth withheld until the display currency is the person's: empty, its line kept. */
    val lineWithheld: Boolean = false,
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
/**
 * The keys that control this wallet (spec 062) — drawn under the account and
 * above everything else, with the Ethereum backup as the block's last row.
 * [rows] is empty while the first answer is in flight; [note] is set only when
 * the registry did not answer and the rows are what this device remembers.
 */
data class WalletKeysModel(
    val title: String,
    val subtitle: String,
    /** "3"; empty while loading — never a guessed count. */
    val count: String,
    val loading: Boolean,
    val note: String?,
    val rows: List<WalletKeyRowModel>,
    val backup: SettingsRowModel?,
    /** Spec 102: "Keys on {{domain}}" — only for a wallet on its own signing domain. */
    val domain: String? = null,
    /**
     * Under the copy's row, the core's words: what becomes public (the
     * address and name, each key's name, public key, credential ID and
     * authenticator model), that it costs a network fee, and that a copy
     * moves no money and brings back no lost passkey. Which state it is said
     * under is the core's too (`BackupRow.explain_key`): `null` — under a
     * wallet that can never be copied — draws no paragraph and keeps no room
     * for one.
     */
    val backupExplain: String?,
    val copyLabel: String,
    val copiedLabel: String,
)

/** [SignsHere] is the one filled pill — the key this device signs with; the rest are outlined. */
enum class KeyPillTone { SignsHere, Verified, Synced, Local }

data class KeyPillModel(val text: String, val tone: KeyPillTone)

data class KeyDetailModel(val label: String, val value: String, val mono: Boolean, val copy: Boolean)

data class WalletKeyRowModel(
    val name: String,
    /** The vault's name when the catalog knows it; else the method's generic line. */
    val holder: String,
    /** `197d…647b` — what tells two unnamed keys apart. */
    val fingerprint: String,
    /** "Verify to use", "Cloud-synced" / "Device-bound" — the registry explorer's pills. */
    val pills: List<KeyPillModel>,
    /** What the row opens onto: the explorer's facts. Empty = nothing to open. */
    val details: List<KeyDetailModel>,
    val key: app.getvela.wallet.feature.onboarding.core.CreateKeyRow,
)

data class SettingsScreenModel(
    val state: SettingsScreenState,
    val title: String,
    val page: SettingsPage,
    val overlay: SettingsOverlay,
    /** Which tab the bottom bar highlights — 钱包 for the SR rescue states. */
    val selectedTab: String,
    val tabs: TabsModel,
    val account: AccountRowModel,
    /** Live only (spec 062): the keys that control the wallet, and their Ethereum backup. */
    val keys: WalletKeysModel? = null,
    val sections: List<SettingsSectionModel>,
    val theme: SegmentedModel,
    val textScale: TextScaleModel,
    val signOutLabel: String,
    val eraseTitle: String,
    val eraseSubtitle: String,
    val networksTitle: String,
    val networksSubtitle: String,
    val networks: List<NetworkRowModel>,
    val addNetworkLabel: String,
    /** Spec 072: the trash icon's name, and the question it asks. */
    val removeNetworkLabel: String = "",
    val removeNetworkSheet: ConfirmSheetModel = ConfirmSheetModel(title = "", body = "", confirm = "", cancel = "", danger = true),
    /** Spec 072 (FR-010): what the endpoints' Reset asks before every field goes back. */
    val resetEndpointsSheet: ConfirmSheetModel = ConfirmSheetModel(title = "", body = "", confirm = "", cancel = "", danger = true),
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
    /** Spec 069: the default transaction speed's sheet. */
    val feeSpeedSheet: SelectSheetModel = SelectSheetModel(title = "", rows = emptyList()),
    /** Spec 102: Settings → Signing pages. */
    val signingPages: SigningPagesModel = SigningPagesModel(),
    /** Spec 102: the account's "Where you review and sign"; `null` until an account is known. */
    val venue: VenueModel? = null,
    val numberSheet: SelectSheetModel,
    val dateSheet: SelectSheetModel,
    val timeSheet: SelectSheetModel,
    val clearCachesSheet: ConfirmSheetModel,
    val eraseSheet: ConfirmSheetModel,
    val feedback: FeedbackModel,
    val rpcBanner: RpcBannerModel?,
    val rpcFix: RpcFixModel,
    val balanceDetail: BalanceDetailModel,
    /** SR6 (spec 092), built live from the balance core's view. */
    val unreachable: UnreachableModel = UnreachableModel(),
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
