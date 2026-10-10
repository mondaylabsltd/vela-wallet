package app.getvela.wallet.feature.onboarding.core

import org.json.JSONArray
import org.json.JSONObject

/**
 * The core's view models, in Kotlin.
 *
 * The bridge speaks JSON in both directions (spec 019, contracts/shell-operations.md
 * §3), so this file is the one place a field name from `vela-core` is spelled out
 * on Android. Everything downstream reads a typed value; nothing else in the app
 * touches a `JSONObject` key.
 *
 * The parsers are TOLERANT of absent fields and INTOLERANT of wrong ones: a
 * missing optional reads as its default, but a `stage` the app does not know
 * throws, because rendering an unknown stage as "form" would silently show a
 * person the wrong screen. The generated TypeScript mirrors under
 * `app-web/.../onboarding/generated/` are the reference for every shape here.
 */

/** `CreateStage` — which screen of the create journey the core is in. */
enum class CreateStage(val wire: String) {
    Form("form"),
    AddKeys("add_keys"),
    SyncFailed("sync_failed"),
    Created("created"),
    ;

    companion object {
        fun of(wire: String): CreateStage = entries.firstOrNull { it.wire == wire }
            ?: error("unknown create stage from the core: $wire")
    }
}

/** `StatusKey` — the transient line the core reports. Semantic, never words. */
enum class StatusKey(val wire: String) {
    SettingUpIdentity("setting_up_identity"),
    VerifyingIdentity("verifying_identity"),
    ExtractingKey("extracting_key"),
    ComputingAddress("computing_address"),
    SyncingKey("syncing_key"),
    SetupCancelled("setup_cancelled"),
    VerifyCancelled("verify_cancelled"),
    ;

    companion object {
        fun of(wire: String): StatusKey = entries.firstOrNull { it.wire == wire }
            ?: error("unknown status key from the core: $wire")
    }
}

/** `SubmitLabel` — which word the create form's primary button carries. */
enum class SubmitLabel(val wire: String) {
    Create("create"),
    FinishVerify("finish_verify"),
    ;

    companion object {
        fun of(wire: String): SubmitLabel = entries.firstOrNull { it.wire == wire }
            ?: error("unknown submit label from the core: $wire")
    }
}

/**
 * `KeyMethod` — WHERE a key lives: this device, a phone or tablet reached by
 * scanning a code, or a USB security key. Three places, and no fourth.
 *
 * Spec 102: the Trusted Signer used to be drawn here as a fourth "method". It
 * never was a place a key lives — the same three places exist on the page too —
 * it is where a person reviews and signs (the account's signing VENUE, which is
 * the core's `signing_plan`, not this enum). A `trusted_signer` from the core
 * is now an unknown value and throws, as any other would.
 *
 * The CHOICE, not the report: `CreateKeyRow` separately carries what the
 * authenticator said about itself, and the two can legitimately disagree. The
 * ceremony follows the choice; the row's provider line shows the report.
 */
enum class KeyMethod(val wire: String) {
    Platform("platform"),
    Hybrid("hybrid"),
    SecurityKey("security_key"),
    ;

    companion object {
        fun of(wire: String): KeyMethod = entries.firstOrNull { it.wire == wire }
            ?: error("unknown key method from the core: $wire")
    }
}

/** `SessionRoute` — where the app is allowed to be. */
enum class SessionRoute(val wire: String) {
    Loading("loading"),
    Onboarding("onboarding"),
    Wallet("wallet"),
    ;

    companion object {
        fun of(wire: String): SessionRoute = entries.firstOrNull { it.wire == wire }
            ?: error("unknown session route from the core: $wire")
    }
}

/** `CreateKeyRow` — one row of the founding-key list. */
data class CreateKeyRow(
    val name: String,
    val authenticatorAttachment: String,
    val transports: String,
    val confirmed: Boolean,
    val synced: Boolean,
    val aaguid: String,
    /**
     * The vault holding this key, resolved by the core from [aaguid]. Empty
     * when the catalog does not know the model — the row then says what it
     * always said, from [method].
     */
    val providerName: String,
    val method: KeyMethod,
    /**
     * WHERE this key lives, from the authenticator's own report — the row's
     * icon and caption both read it. Defaults to [method] for a core that
     * predates the field.
     */
    val kind: KeyMethod = method,
)

/** `CreateView`. */
data class CreateView(
    val stage: CreateStage,
    val name: String,
    val nameEditable: Boolean,
    val nameTooLong: Boolean,
    val acks: List<Boolean>,
    val canSubmit: Boolean,
    val submitLabel: SubmitLabel,
    val showStartOver: Boolean,
    val busy: Boolean,
    val status: StatusKey?,
    val keys: List<CreateKeyRow>,
    val canAddKey: Boolean,
    val canFinish: Boolean,
    val needsSecondKey: Boolean,
    val canGoBack: Boolean,
    val address: String?,
    val syncErrorDetail: String?,
    /** The places a key may be minted in — always the three (spec 102). */
    val addMethods: List<KeyMethod> = KeyMethod.entries,
    /**
     * Spec 102: the domain this wallet's keys are minted for — `getvela.app`,
     * or the domain of the page chosen with "Use a trusted signing page". Shown,
     * so a person sees which site their keys will belong to.
     */
    val signingDomain: String = "getvela.app",
    /** Spec 102: that page, normalised, when one was chosen; `null` in the app. */
    val signingPage: String? = null,
    /** Spec 102: may a page still be chosen? Only before the first key. */
    val canChoosePage: Boolean = false,
    /**
     * Issue #475: the corpus key of the heading over the three places —
     * "Add a passkey" with no key yet, "Add another" with room for one more,
     * "Limit of 7 reached" at the cap. The core picks it; the screen says it.
     */
    val addHeadingKey: String = ADD_HEADING_ANOTHER,
    /**
     * Issue #475: the three places are drawn open, with no fold to tap — no
     * key yet and one may be added. The core's word; only whether the fold
     * is open is the screen's.
     */
    val methodsPinned: Boolean = false,
    /**
     * Is the "Added n / 7" counter drawn? The core's word — from the first
     * key on. With no key it read "0 / 7" over an empty list: a count of
     * nothing, beside a subtitle that already says "up to 7".
     */
    val keyCountShown: Boolean = false,
) {
    companion object {
        fun from(json: JSONObject): CreateView = CreateView(
            stage = CreateStage.of(json.getString("stage")),
            name = json.optString("name"),
            nameEditable = json.optBoolean("name_editable"),
            nameTooLong = json.optBoolean("name_too_long"),
            acks = json.optJSONArray("acks").booleans(),
            canSubmit = json.optBoolean("can_submit"),
            submitLabel = SubmitLabel.of(json.getString("submit_label")),
            showStartOver = json.optBoolean("show_start_over"),
            busy = json.optBoolean("busy"),
            status = json.nullableString("status")?.let(StatusKey::of),
            keys = json.optJSONArray("keys").objects().map { key ->
                CreateKeyRow(
                    name = key.optString("name"),
                    authenticatorAttachment = key.optString("authenticator_attachment"),
                    transports = key.optString("transports"),
                    confirmed = key.optBoolean("confirmed"),
                    synced = key.optBoolean("synced"),
                    aaguid = key.optString("aaguid"),
                    providerName = key.optString("provider_name"),
                    method = KeyMethod.of(key.getString("method")),
                    kind = key.optString("kind").ifEmpty { key.getString("method") }
                        .let(KeyMethod::of),
                )
            },
            canAddKey = json.optBoolean("can_add_key"),
            canFinish = json.optBoolean("can_finish"),
            needsSecondKey = json.optBoolean("needs_second_key"),
            canGoBack = json.optBoolean("can_go_back"),
            address = json.nullableString("address"),
            syncErrorDetail = json.nullableString("sync_error_detail"),
            addMethods = json.optJSONArray("add_methods")
                ?.let { arr -> (0 until arr.length()).map { KeyMethod.of(arr.getString(it)) } }
                ?: KeyMethod.entries,
            signingDomain = json.optString("signing_domain").ifEmpty { "getvela.app" },
            signingPage = json.nullableString("signing_page"),
            canChoosePage = json.optBoolean("can_choose_page"),
            addHeadingKey = json.optString("add_heading_key").ifEmpty { ADD_HEADING_ANOTHER },
            methodsPinned = json.optBoolean("methods_pinned"),
            keyCountShown = json.optBoolean("key_count_shown"),
        )

        /** What a core from before issue #475 leaves the heading as: the label the list always had. */
        const val ADD_HEADING_ANOTHER = "onboarding.create.addMethodLabel"
    }
}

/** `LoginView` — two booleans, and it stays that way (data-model §4). */
data class LoginView(val busy: Boolean, val endpointUnreachable: Boolean) {
    companion object {
        fun from(json: JSONObject): LoginView = LoginView(
            busy = json.optBoolean("busy"),
            endpointUnreachable = json.optBoolean("endpoint_unreachable"),
        )
    }
}

/**
 * One row of the account switcher. `index` is the position in the ORIGINAL list.
 *
 * Spec 102: [signingDomain] is the RP ID the account's keys live under, and
 * [signingVenueJson] where its transactions and messages are reviewed and
 * signed (`{"type":"in_vela"}` / `{"type":"page","url":…}`) — both the core's,
 * read off the record as the session holds it, and handed back to the core
 * (`signingVenueChoices`) rather than interpreted here.
 */
data class SessionAccountRow(
    val index: Int,
    val name: String,
    val address: String,
    val signingDomain: String = "getvela.app",
    val signingVenueJson: String = """{"type":"in_vela"}""",
)

/** `SessionSignOutView` — present iff the confirmation dialog is open. */
data class SessionSignOutView(
    val pendingUploadWarning: Boolean,
    /** How many wallets this device is signed into — the sheet says so when
     *  it is more than one, because "nothing is deleted, it all comes back"
     *  says nothing about signing in six times (2026-09-23). */
    val accountCount: Int = 1,
)

/** `SessionView` — the route guard and the account list. */
data class SessionView(
    val loading: Boolean,
    val hasWallet: Boolean,
    val address: String,
    val activeIndex: Int,
    val accounts: List<SessionAccountRow>,
    val allowedRoute: SessionRoute,
    val signOut: SessionSignOutView?,
) {
    /**
     * The active account's display NAME, `""` when there is none.
     *
     * The address rides in the view pre-derived; the name does not, so every
     * screen that wants it would otherwise re-index the account list — and an
     * out-of-range [activeIndex] from a torn view would throw rather than
     * render an empty header.
     */
    val activeName: String get() = accounts.getOrNull(activeIndex)?.name.orEmpty()

    /** The active account's row, `null` when there is none (or a torn view). */
    val activeRow: SessionAccountRow? get() = accounts.getOrNull(activeIndex)

    companion object {
        fun from(json: JSONObject): SessionView = SessionView(
            loading = json.optBoolean("loading"),
            hasWallet = json.optBoolean("has_wallet"),
            address = json.optString("address"),
            activeIndex = json.optInt("active_index"),
            accounts = json.optJSONArray("accounts").objects().map { row ->
                val account = row.optJSONObject("account") ?: JSONObject()
                SessionAccountRow(
                    index = row.optInt("index"),
                    name = account.optString("name"),
                    address = account.optString("address"),
                    signingDomain = account.optString("signing_domain").ifEmpty { "getvela.app" },
                    signingVenueJson = account.optJSONObject("signing_venue")?.toString()
                        ?: """{"type":"in_vela"}""",
                )
            },
            allowedRoute = SessionRoute.of(json.getString("allowed_route")),
            signOut = json.optJSONObject("sign_out")?.let { sheet ->
                SessionSignOutView(
                    pendingUploadWarning = sheet.optBoolean("pending_upload_warning"),
                    // One wallet and six cannot read the same (2026-09-23).
                    accountCount = sheet.optInt("account_count", 1),
                )
            },
        )
    }
}

/**
 * `PromptKind` — a question or a notice.
 *
 * `detail` is the platform's own words on the two variants that carry them, and
 * it is forwarded verbatim: it goes into the bug report, and prettifying it here
 * would lose the only part worth filing.
 */
data class PromptKind(
    val type: String,
    val detail: String?,
    /** The core's verdict that the link to the other device failed, not the authenticator (issue #446). */
    val phoneLink: Boolean = false,
    /** The core's verdict that a not-supported ceremony was a security key's (issue #450). */
    val securityKey: Boolean = false,
    /**
     * `registry_unreachable` only: every failed lookup never left this device,
     * so the sheet says "check your connection" rather than blame the registry.
     */
    val local: Boolean = false,
) {
    companion object {
        fun from(json: JSONObject): PromptKind =
            PromptKind(
                json.getString("type"),
                json.nullableString("detail"),
                json.optBoolean("phone_link", false),
                json.optBoolean("security_key", false),
                json.optBoolean("local", false),
            )
    }
}

// ---------------------------------------------------------------------------
// org.json helpers
// ---------------------------------------------------------------------------
//
// `optString` returns the four-character string "null" for a JSON null, which is
// how a nullable field silently becomes a non-empty value that renders. Every
// nullable field in this file goes through `nullableString` instead.

internal fun JSONObject.nullableString(key: String): String? =
    if (isNull(key)) null else optString(key).takeIf { it.isNotEmpty() }

private fun JSONArray?.booleans(): List<Boolean> =
    if (this == null) emptyList() else (0 until length()).map { optBoolean(it) }

internal fun JSONArray?.objects(): List<JSONObject> =
    if (this == null) emptyList() else (0 until length()).mapNotNull { optJSONObject(it) }
