package app.getvela.wallet.feature.settings

import app.getvela.wallet.core.diagnostics.BugReport
import app.getvela.wallet.core.format.tokenAmountText
import app.getvela.wallet.feature.send.core.FeeTier
import app.getvela.wallet.feature.settings.core.FeeTierPrefView
import app.getvela.wallet.feature.settings.core.SigningPage
import app.getvela.wallet.feature.settings.core.SigningPagesView
import app.getvela.wallet.feature.settings.components.SigningPageItemModel
import app.getvela.wallet.feature.settings.components.integrityModel
import app.getvela.wallet.feature.signing.trustedsigner.SignerPageChecks
import app.getvela.wallet.feature.signing.trustedsigner.VenueWords
import app.getvela.wallet.core.crux.Wire
import kotlinx.serialization.builtins.ListSerializer
import uniffi.vela_core_uniffi.SignerIntegrityLine
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.wallet.components.tokenGlyph
import app.getvela.wallet.feature.settings.core.NetNetworkRow
import app.getvela.wallet.feature.browser.ExploreLive
import app.getvela.wallet.feature.send.core.SendTreasuryStatus
import app.getvela.wallet.feature.send.core.SendTreasuryAsset
import app.getvela.wallet.feature.send.SendLive
import app.getvela.wallet.feature.settings.core.DeviceStorage
import app.getvela.wallet.core.format.DateFormatKey
import app.getvela.wallet.core.format.NumberFormatKey
import app.getvela.wallet.core.format.TextScaleLevel
import app.getvela.wallet.core.format.TimeFormatKey
import app.getvela.wallet.core.marks.Marks
import app.getvela.wallet.core.format.Formats
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.settings.core.CurrencyCatalog
import app.getvela.wallet.feature.settings.core.CurrencyView
import app.getvela.wallet.feature.settings.core.NetProbeHealth
import app.getvela.wallet.feature.settings.core.NetServiceHealth
import app.getvela.wallet.feature.settings.core.NetEndpointField
import app.getvela.wallet.feature.settings.core.NetProviderId
import app.getvela.wallet.feature.settings.core.NetRpcField
import app.getvela.wallet.feature.settings.core.NetView
import app.getvela.wallet.feature.settings.core.NetWizardErrorKind
import app.getvela.wallet.feature.settings.core.NetWizardPhase
import app.getvela.wallet.feature.wallet.core.BalanceView

/**
 * The live settings builders: what the core has ruled → the display models the
 * drawn components already accept.
 *
 * The sibling of [SettingsFixtures], and the division between them is the
 * point: fixtures are the canon the gallery renders, so they keep every state
 * including the ones no wallet is currently in. This file renders **one**
 * state — the person's.
 *
 * It grows a machine at a time. Spec 040 phase 3 brings the display currency;
 * phase 5 brings the networks, endpoints and providers. Anything a builder here
 * has not claimed yet is still fixture-fed and says so at its call site.
 */
object SettingsLive {

    /**
     * The display currency, as the settings surface shows it in two places:
     * the localisation row's value, and which row the currency sheet ticks.
     *
     * **What this does not do is convert anything.** `view.rate` is `null`
     * until spec 041 gives the shell a way to price a currency, and the row
     * shows the code alone rather than a converted sample when it is. Showing
     * "¥1,234.56" against a rate nobody could fetch would be inventing an
     * exchange rate on the settings screen.
     */
    /**
     * The default transaction speed (spec 069): the row's value and which
     * speed the sheet ticks, both from the `fee_tier_pref` core — so the
     * Settings row and the send screen's folded control say the same thing.
     */
    fun withFeeTier(model: SettingsScreenModel, view: FeeTierPrefView, s: VelaStrings): SettingsScreenModel {
        val id = when (view.tier) {
            FeeTier.Standard -> "standard"
            FeeTier.Slow -> "slow"
            else -> "fast"
        }
        val sheet = SettingsFixtures.feeSpeedSheet(s, id)
        return model.copy(
            sections = model.sections.map { section ->
                section.copy(
                    rows = section.rows.map { row ->
                        if (row.id == SettingsFixtures.FEE_SPEED_ROW) {
                            row.copy(value = sheet.rows.firstOrNull { it.selected }?.label ?: row.value)
                        } else {
                            row
                        }
                    },
                )
            },
            feeSpeedSheet = sheet,
        )
    }

    /**
     * How a signing page is NAMED, everywhere it is named (D6) — the lists,
     * and the hand-off card: Vela's official signing page; the person's own
     * label; else "Self-hosted · <domain>". Never by its host alone: a host
     * says where a page is, not whose it is.
     */
    fun pageName(name: String, official: Boolean, domain: String, address: String, s: VelaStrings): String {
        val label = if (official) "" else name.trim()
        return when {
            official -> s.t("settings.signing.pageOfficial")
            label.isNotEmpty() -> label
            else -> s.t("settings.signing.pageSelfHosted", mapOf("domain" to domain.ifBlank { address.substringBefore('/') }))
        }
    }

    /**
     * One signing page as every list draws it (spec 102): its name — Vela's
     * official signing page, the person's label, or "Self-hosted · <domain>"
     * (D6: only a page the person deployed is "their own"); the address;
     * whose keys it reaches; and the integrity line the phone's check gives
     * it now. [trustVersion]: the version its check asks the person to trust
     * (`SignerPageAdmission.versionToTrust`), when it does.
     */
    fun pageItem(
        url: String,
        name: String,
        domain: String,
        official: Boolean,
        line: SignerIntegrityLine,
        s: VelaStrings,
        trustVersion: String? = null,
    ): SigningPageItemModel {
        val address = url.substringAfter("://").trimEnd('/')
        val integrity = integrityModel(line, s)
        val label = if (official) "" else name.trim()
        val host = address.substringBefore('/')
        val title = pageName(name = name, official = official, domain = domain, address = address, s = s)
        return SigningPageItemModel(
            url = url,
            title = title,
            address = address,
            // A page is named once: "Self-hosted · sign.example.com" already
            // says the address, so the row does not say it again (a page at a
            // path, or one named otherwise, still shows where it is).
            showAddress = !title.contains(address),
            // "Keys on …" only where it tells the person something: Vela's
            // official page at sign.getvela.app keeps its keys on getvela.app;
            // a self-hosted page whose keys are its own host's says nothing more.
            domain = if (domain.isBlank() || domain.equals(host.substringBefore(':'), ignoreCase = true)) {
                ""
            } else {
                s.t("settings.signing.keysOn", mapOf("domain" to domain))
            },
            integrity = integrity,
            official = official,
            trustVersion = trustVersion?.takeIf { integrity.asksToTrust && it.isNotBlank() },
            label = label,
        )
    }

    /**
     * Settings → Signing pages (spec 102): the row in the advanced section and
     * the page behind it, from the `signing_pages` core and the integrity
     * checks. Nothing here decides what may be saved — a refused address comes
     * back as the core's `add_error`. [toTrust] is the version a page's check
     * asks the person to trust, when it does.
     */
    fun withSigningPages(
        model: SettingsScreenModel,
        view: SigningPagesView,
        line: (String) -> SignerIntegrityLine,
        s: VelaStrings,
        toTrust: (String) -> String? = { null },
    ): SettingsScreenModel {
        val rows = view.pages.map { row ->
            pageItem(
                url = row.url, name = row.name, domain = row.domain, official = row.official,
                line = line(row.url), s = s, trustVersion = toTrust(row.url),
            )
        }
        val pages = SigningPagesModel(
            title = s.t("settings.signing.title"),
            subtitle = s.t("settings.signing.subtitle"),
            rows = rows,
            add = s.t("settings.signing.pageAdd"),
            nameLabel = s.t("settings.signing.pageName"),
            rename = s.t("settings.signing.pageRename"),
            addError = when (view.add_error) {
                "invalid" -> s.t("settings.signing.pageInvalid")
                "insecure" -> s.t("settings.signing.pageInsecure")
                "duplicate" -> s.t("settings.signing.pageDuplicate")
                else -> null
            },
            save = s.t("settings.signing.pageSave"),
            remove = s.t("settings.signing.pageRemove"),
            cancel = s.t("common.cancel"),
            trust = s.t("settings.signing.pageTrust"),
            loaded = view.loaded,
        )
        return model.copy(
            sections = model.sections.map { section ->
                section.copy(
                    rows = section.rows.map { row ->
                        if (row.id == SettingsFixtures.SIGNING_PAGES_ROW) {
                            row.copy(value = view.pages.size.toString())
                        } else {
                            row
                        }
                    },
                )
            },
            signingPages = pages,
        )
    }

    /**
     * "Where you review and sign" (spec 102, P2-09) for the active account:
     * the core's choices (`signingVenueChoices`, R1/R2) in its order — Vela's
     * sheet, the official page, the saved pages, then the account's page when
     * it is not saved — each blocked one with its reason. A custom-domain
     * account's Vela row comes back blocked: it is locked to its page. A page
     * whose check asks to be trusted carries the version to trust
     * ([toTrust], `versionToTrust`), answered by "Trust this version" there.
     */
    fun withVenue(
        model: SettingsScreenModel,
        account: app.getvela.wallet.feature.onboarding.core.SessionAccountRow?,
        saved: List<SigningPage>,
        line: (String) -> SignerIntegrityLine,
        s: VelaStrings,
        toTrust: (String) -> String? = { null },
    ): SettingsScreenModel {
        account ?: return model.copy(venue = null)
        val savedJson = Wire.json.encodeToString(ListSerializer(SigningPage.serializer()), saved)
        val choicesJson = runCatching {
            uniffi.vela_core_uniffi.signingVenueChoices(account.signingDomain, account.signingVenueJson, savedJson)
        }.getOrNull() ?: return model.copy(venue = null)
        val array = runCatching { org.json.JSONArray(choicesJson) }.getOrNull() ?: return model.copy(venue = null)
        val inVela = uniffi.vela_core_uniffi.venueWords("in_vela")
        val onPage = uniffi.vela_core_uniffi.venueWords("page")
        fun lineOf(words: uniffi.vela_core_uniffi.KeyMethodWords?): String =
            words?.lineName ?: words?.lineKey?.let(s::t).orEmpty()
        val choices = (0 until array.length()).mapNotNull { index ->
            val choice = array.optJSONObject(index) ?: return@mapNotNull null
            val venue = choice.optJSONObject("venue") ?: return@mapNotNull null
            val reason = choice.optJSONObject("blocked")?.let { block -> VenueWords.block(block, s::t) }
            val url = venue.optString("url")
            if (venue.optString("type") == "page" && url.isNotEmpty()) {
                VenueChoiceModel(
                    venueJson = venue.toString(),
                    title = "",
                    body = "",
                    page = pageItem(
                        url = url, name = choice.optString("name"), domain = choice.optString("domain"),
                        official = choice.optBoolean("official"), line = line(url), s = s,
                        // A choice that cannot reach the keys is not asked about.
                        trustVersion = toTrust(url).takeIf { reason == null },
                    ),
                    selected = choice.optBoolean("active"),
                    reason = reason,
                )
            } else {
                VenueChoiceModel(
                    venueJson = venue.toString(),
                    title = inVela?.titleKey?.let(s::t).orEmpty(),
                    body = lineOf(inVela),
                    page = null,
                    selected = choice.optBoolean("active"),
                    reason = reason,
                )
            }
        }
        val active = choices.firstOrNull { it.selected }
        val title = s.t("settings.venue.title")
        // The row says where it signs now — "Review and sign in Vela", or the
        // page by its name (D6: 「Vela 官方签名页」, the person's label,
        // 「自己部署的签名页 · domain」), which already says it is a page — or,
        // when nothing here can reach the keys, why. (iOS says the same.)
        val blocked = choices.isNotEmpty() && choices.all { it.reason != null }
        return model.copy(
            venue = VenueModel(
                row = SettingsRowModel(
                    id = VENUE_ROW,
                    title = title,
                    icon = SettingsIcon.Eye,
                    subtitle = if (blocked) choices.first().reason else null,
                    value = when {
                        active == null -> null
                        active.page != null -> active.page.title
                        else -> active.title
                    },
                ),
                title = title,
                subtitle = s.t("settings.venue.subtitle"),
                domainLine = s.t("settings.signing.keysOn", mapOf("domain" to account.signingDomain)),
                choices = choices,
                pageSection = onPage?.titleKey?.let(s::t).orEmpty(),
                pageSectionBody = lineOf(onPage),
                manage = s.t("settings.signing.title"),
                trust = s.t("settings.signing.pageTrust"),
            ),
        )
    }

    const val VENUE_ROW = "signing-venue"

    fun withCurrency(model: SettingsScreenModel, view: CurrencyView): SettingsScreenModel {
        // The person's own choice — on its way (`pending`) or settled — never
        // the USD placeholder standing in for it.
        val code = view.pending?.takeIf { !view.committed } ?: view.code
        return model.copy(
            sections = model.sections.map { section ->
                section.copy(
                    rows = section.rows.map { row ->
                        if (row.id == "currency") row.copy(value = currencyRowValue(code)) else row
                    },
                )
            },
            currencySheet = model.currencySheet.copy(
                rows = CurrencyCatalog.entries.map { entry ->
                    SelectRowModel(
                        id = entry.code,
                        label = entry.code,
                        glyph = entry.glyph,
                        caption = entry.caption,
                        selected = entry.code == code,
                    )
                },
            ),
        )
    }

    /**
     * The networks a person has, the endpoints they point at, and the provider
     * keys they hold — all from the core's store rather than from the eleven
     * chains the ST9 mock happens to draw.
     *
     * **Health is measured or it is absent.** `rpc_health` was `null` for the
     * whole of spec 040 and the rows carried no pill, because an unmeasured
     * endpoint drawn as fast is the one lie this screen is most able to tell by
     * accident. Spec 041 gives the shell a way to probe; a row still carries no
     * pill until its own probe has answered. (FR-013.)
     */
    /**
     * An endpoint's health, as a pill — or nothing.
     *
     * `null` means nobody has asked yet, and it stays blank. "Checking" is also
     * blank rather than a spinner in a list of twelve rows: a screenful of
     * spinners reads as a broken screen, and the answer arrives in about a
     * second.
     *
     * The wording and the one-second threshold are the drawn design's, taken
     * from `SettingsFixtures.latency` rather than reinvented — the same pill a
     * person has been looking at, now with a number somebody measured.
     */
    private fun healthPill(health: NetProbeHealth?, strings: VelaStrings): StatusPillModel? =
        when (health) {
            null, NetProbeHealth.Checking -> null
            is NetProbeHealth.Ok -> {
                val slow = health.latency_ms >= SLOW_MS
                val value = if (slow) {
                    "%.1fs".format(health.latency_ms / 1000.0)
                } else {
                    "${health.latency_ms}ms"
                }
                val word = strings.t(
                    if (slow) I18nKeys.SettingsUi.NETWORK_SLOW else I18nKeys.SettingsUi.NETWORK_ONLINE,
                )
                StatusPillModel(
                    tone = if (slow) SettingsTone.Warn else SettingsTone.Ok,
                    label = "$word · $value",
                )
            }
            NetProbeHealth.Error ->
                StatusPillModel(SettingsTone.Error, strings.t(I18nKeys.SettingsUi.NETWORK_OFFLINE))
        }

    /**
     * A service endpoint's pill — every `NetServiceHealth` worded (the web's
     * `servicePill`): quiet while checking, the latency once it answered,
     * and why not when it did not.
     */
    internal fun servicePill(health: NetServiceHealth, strings: VelaStrings): StatusPillModel? =
        when (health) {
            NetServiceHealth.Checking -> null
            is NetServiceHealth.Ok -> healthPill(NetProbeHealth.Ok(health.latency_ms), strings)
            NetServiceHealth.NotHttps ->
                StatusPillModel(SettingsTone.Error, strings.t(I18nKeys.SettingsUi.HEALTH_HTTPS_REQUIRED))
            is NetServiceHealth.Unreachable ->
                StatusPillModel(SettingsTone.Error, strings.t(I18nKeys.SettingsUi.NETWORK_OFFLINE))
            is NetServiceHealth.InvalidResponse ->
                StatusPillModel(SettingsTone.Error, strings.t(I18nKeys.SettingsUi.HEALTH_INVALID))
        }

    /** A second is where the drawn design calls an endpoint slow. */
    private const val SLOW_MS = 1_000L

    /**
     * The add-network wizard: what the chain index answered for what was typed.
     *
     * **An empty query shows nothing.** The fixture drew three results under an
     * empty search box, which reads as "these are your options" — and two of
     * them were invented chains. A search nobody has performed has no results.
     */
    fun withWizard(
        model: SettingsScreenModel,
        view: NetView,
        strings: VelaStrings,
    ): SettingsScreenModel {
        val wizard = view.wizard
        val info = wizard.chain_info
        val compat = wizard.compat
        val checked = wizard.phase == NetWizardPhase.Checked
        val stopped = wizard.phase == NetWizardPhase.Error
        val unverified = checked && (compat == null || compat.rpc_failure != null)
        val compatible = checked && compat != null && compat.compatible && compat.rpc_failure == null
        // Why the wizard stopped, in the core's sentence (`error_key`): the
        // network is already added, was not found, lists no RPC endpoint,
        // could not be verified — or was refused by its check, in the check's
        // own words. This file words no stop itself.
        val stop = wizard.error_key?.takeIf { stopped }
        // The check answered and the chain is refused — in the wizard
        // (`checked`), or on the scan path, which has no confirm step and so
        // stops with the check kept beside it. The core's reason and — for
        // missing contracts only — its link, the same on both.
        val refusedStop = stopped && wizard.error is NetWizardErrorKind.NotCompatible
        val refusedCheck = checked && !compatible && !unverified
        val refusal = compat?.hint_key?.takeIf { refusedCheck }
        val setupUrl = compat?.setup_url?.takeIf { refusedStop || refusedCheck }
        // The RPC field and "Re-check with this RPC" are ONE rule, and it is
        // the core's (`rpc_field`, PR 3 final notes F4 / F14 / F22): the field
        // exactly when naming another endpoint is a way on — the check passed
        // or could not reach a verdict (optional), or the network lists no
        // endpoint (required: "Enter one, then re-check", so the box to enter
        // one in is there and is not called "(optional)") — and the re-check
        // exactly when the field is drawn, never one without the other. Not
        // under a refusal: another endpoint does not give a network the
        // P-256 verifier or the contracts it lacks, and a "Re-check with this
        // RPC" there had no RPC box for "this" to mean. This file used to
        // decide both, each by its own condition.
        val rpcField = wizard.rpc_field != NetRpcField.None
        return model.copy(
            addNetwork = model.addNetwork.copy(
                query = wizard.query,
                results = if (wizard.query.isBlank()) {
                    emptyList()
                } else {
                    wizard.suggestions.map { entry ->
                        NetworkRowModel(
                            // The chain id IS the identity: two chains can
                            // share a name, and the tap must reach the right
                            // one.
                            id = entry.chain_id.toString(),
                            mark = ChainMarkModel(
                                letter = entry.name.take(1).uppercase(),
                                colorArgb = markColour(entry.chain_id),
                                logoUrl = Marks.chainLogoUrl(entry.chain_id.toInt()),
                            ),
                            name = entry.name,
                            meta = strings.t(
                                I18nKeys.SettingsUi.CHAIN_ID,
                                mapOf("chainId" to entry.chain_id.toString()),
                            ) + " · " + entry.native_currency_symbol,
                        )
                    }
                },
                candidate = info?.let {
                    NetworkRowModel(
                        id = it.chain_id.toString(),
                        mark = ChainMarkModel(
                            letter = it.name.take(1).uppercase(),
                            colorArgb = markColour(it.chain_id),
                            logoUrl = Marks.chainLogoUrl(it.chain_id.toInt()),
                        ),
                        name = it.name,
                        meta = strings.t(
                            I18nKeys.SettingsUi.CHAIN_ID,
                            mapOf("chainId" to it.chain_id.toString()),
                        ) + " · " + it.native_symbol,
                        // **No verdict until one was reached.** While the
                        // checks are running there is no pill: a chain drawn as
                        // compatible before anything was checked is the same
                        // lie as a latency nobody measured. And "we could not
                        // check" is NEVER worded as "incompatible" (the core's
                        // invariant ③, the web's `wizardModel`).
                        badge = when {
                            compatible -> StatusPillModel(SettingsTone.Ok, strings.t(I18nKeys.SettingsUi.ADD_COMPATIBLE))
                            unverified -> StatusPillModel(SettingsTone.Warn, strings.t("settingsModals.addNetwork.unableToVerify"))
                            checked -> StatusPillModel(SettingsTone.Error, strings.t(I18nKeys.SettingsUi.ADD_INCOMPATIBLE))
                            else -> null
                        },
                        tag = if (it.is_testnet) {
                            strings.t(I18nKeys.SettingsUi.ADD_TESTNET)
                        } else {
                            null
                        },
                    )
                },
                // Each contract the core looked for, and the signer precompile:
                // found or not. The names are the core's.
                checks = compat?.let { result ->
                    result.contracts.map { contract -> CheckItemModel(label = contract.name, ok = contract.deployed) } +
                        CheckItemModel(label = strings.t(I18nKeys.SettingsUi.ADD_CHECK_SIGNER), ok = result.p256_available == true)
                }.orEmpty().takeIf { checked && !unverified }.orEmpty(),
                checksTitle = if (checked && !unverified && compat != null) {
                    strings.t(I18nKeys.SettingsUi.ADD_COMPATIBILITY_CHECK)
                } else {
                    null
                },
                // The person's own RPC for this chain, as the core holds it —
                // so a keystroke round-trips. Drawn where the core says
                // (`rpc_field`), under the core's label: "Custom RPC
                // (optional)", or "RPC URL" where it is the one thing asked for.
                customRpc = if (rpcField) {
                    UrlFieldModel(
                        id = "custom-rpc",
                        label = strings.t(wizard.rpc_field_label_key ?: I18nKeys.SettingsUi.ADD_CUSTOM_RPC_TITLE),
                        value = wizard.custom_rpc,
                        placeholder = strings.t(I18nKeys.SettingsUi.ADD_CUSTOM_RPC_PLACEHOLDER),
                        // Spec 098 §5.1: the relay is sent this RPC, key and all.
                        hint = strings.t(I18nKeys.SettingsUi.NETWORK_RELAY_NOTICE),
                    )
                } else {
                    null
                },
                // Why the wizard stopped, why this chain cannot be added, or —
                // when it can — what adding it still will not do. Said, not
                // just badged, and only ONE of the three: a callout is the
                // sentence under the pill, and two would contradict.
                //
                // The stop comes first because it is why nothing more happened.
                // The last arm is spec 081 FR-009: two of the contracts above
                // matter only to a wallet holding more than one passkey, so
                // when those are the only ones missing the core says
                // "compatible" — truthfully — and this is the rest of that
                // sentence. Without it the pill says Compatible while two rows
                // carry a red cross. It cannot collide with the arm above it:
                // that one needs `!compatible`, this one needs `compatible`.
                //
                // A refusal says WHY, in the core's words (`compat.hint_key`):
                // no P-256 verifier — Vela wallets cannot work here and money
                // sent here would be stuck — or contracts that are missing and
                // can be deployed. The two used to share one sentence about
                // contracts, which is false of the first.
                //
                // A stop is said in the core's sentence, whichever stop it is.
                // It used to be one line for all of them ("Some contracts Vela
                // needs aren't on this network yet") — over a network that was
                // already added, one nobody could find and one with no RPC
                // listed, none of which is about contracts.
                // Why, then where: a stop the field is the way on from says
                // so OVER the box.
                calloutAsksForRpc = stop != null && rpcField,
                callout = when {
                    stop != null -> CalloutModel(CalloutTone.Warning, strings.t(stop))
                    refusal != null -> CalloutModel(CalloutTone.Warning, strings.t(refusal))
                    compat?.let { it.compatible && !it.multi_key_ready } == true -> CalloutModel(
                        tone = CalloutTone.Warning,
                        text = strings.t(I18nKeys.SettingsUi.ADD_SINGLE_KEY_ONLY),
                    )
                    else -> null
                },
                // Only offered when the core says this chain can be added — or,
                // for a chain that could not be checked, Retry. The button that
                // writes a network somebody's money will be read from must not
                // be reachable on a chain whose contracts are not deployed.
                primary = when {
                    wizard.can_add -> strings.t(I18nKeys.SettingsUi.ADD_BUTTON)
                    unverified -> strings.t("settingsModals.addNetwork.retry")
                    else -> null
                },
                // The chain setup tool is for a chain that is really missing
                // Vela's contracts — never for one that could not be reached,
                // and never for one with no P-256 verifier (nothing can be
                // deployed to add it). The core says which by sending the
                // link, opened on this chain (`setup_url`), or not.
                secondary = setupUrl?.let { strings.t(I18nKeys.SettingsUi.ADD_CHAIN_TOOL) },
                secondaryUrl = setupUrl,
                // "Re-check with this RPC" reads the field, so it is drawn
                // exactly where the field is (the core's one rule, above).
                recheck = strings.t(I18nKeys.SettingsUi.ADD_RECHECK_WITH_RPC).takeIf { rpcField },
            ),
        )
    }

    fun withNetworks(
        model: SettingsScreenModel,
        view: NetView,
        strings: VelaStrings,
    ): SettingsScreenModel {
        if (!view.loaded) return model
        return model.copy(
            // The row that opens the list said "12 个网络" from the fixture
            // while the list beside it was live — so adding a custom network
            // left the count untouched. Found on a device (spec 040 phase 11).
            sections = model.sections.map { section ->
                section.copy(
                    rows = section.rows.map { row ->
                        if (row.id == "networks") {
                            row.copy(
                                value = row.value.orEmpty()
                                    .replaceFirst(Regex("\\d+"), view.networks.size.toString()),
                            )
                        } else {
                            row
                        }
                    },
                )
            },
            networks = view.networks.map { row ->
                NetworkRowModel(
                    id = row.id,
                    mark = ChainMarkModel(
                        letter = row.display_name.take(1).uppercase(),
                        colorArgb = markColour(row.chain_id),
                        logoUrl = Marks.chainLogoUrl(row.chain_id.toInt()),
                    ),
                    name = row.display_name,
                    meta = strings.t(
                        I18nKeys.SettingsUi.CHAIN_ID,
                        mapOf("chainId" to row.chain_id.toString()),
                    ),
                    // A custom network's row carries its "custom" tag and no health
                    // pill, as on the web (`liveNetworkRows`): the probe is shown on
                    // its detail page.
                    badge = if (row.is_custom) null else healthPill(row.rpc_health, strings),
                    tag = if (row.is_custom) {
                        strings.t(I18nKeys.SettingsUi.NETWORK_CUSTOM)
                    } else {
                        null
                    },
                    removable = row.is_custom,
                )
            },
            endpoints = model.endpoints.copy(
                fields = view.endpoints.map { endpoint ->
                    UrlFieldModel(
                        id = endpoint.field.name,
                        label = endpointLabel(endpoint.field, model),
                        value = endpoint.value,
                        // The default is the placeholder: an unset endpoint
                        // shows what it WOULD use, greyed, not an empty box.
                        placeholder = endpoint.default_value,
                        badge = servicePill(endpoint.health, strings),
                    )
                },
            ),
            rpcProviders = model.rpcProviders.copy(
                providers = view.providers.mapIndexed { index, provider ->
                    val card = model.rpcProviders.providers.getOrNull(index)
                        ?: model.rpcProviders.providers.firstOrNull()
                    val notSet = strings.t(I18nKeys.SettingsUi.PROVIDER_NOT_SET)
                    val test = provider.test
                    card?.copy(
                        id = provider.provider.name,
                        name = providerName(provider.provider),
                        // Whether a key is set is the core's `has_key`, not the
                        // card's position: the fixture drew Alchemy connected
                        // and "supports 12" on every device (the web's
                        // `liveRpcProviders`).
                        badge = if (provider.has_key) {
                            StatusPillModel(SettingsTone.Ok, strings.t(I18nKeys.SettingsUi.PROVIDER_CONNECTED))
                        } else {
                            StatusPillModel(SettingsTone.Neutral, notSet)
                        },
                        // The field's id is what the host maps back to a
                        // machine event, so it must be the core's own name for
                        // this provider rather than the fixture's label.
                        field = card.field.copy(
                            id = provider.provider.name,
                            value = provider.key,
                            placeholder = if (provider.has_key) null else notSet,
                        ),
                        action = strings.t(
                            if (provider.has_key) I18nKeys.SettingsUi.PROVIDER_CHECK_KEY else I18nKeys.SettingsUi.PROVIDER_GET_KEY,
                        ),
                        // The test's own count, once it has finished — never a drawn one.
                        support = if (test != null && test.done) {
                            strings.t(
                                I18nKeys.SettingsUi.PROVIDER_SUPPORTS,
                                mapOf("count" to test.ok_count.toString(), "total" to test.total.toString()),
                            )
                        } else {
                            null
                        },
                        link = if (provider.has_key) null else "${strings.t(I18nKeys.SettingsUi.PROVIDER_GET_KEY)} →",
                        // The link used to hand its own LABEL to the opener
                        // ("https://Get key →"); each provider's key comes from its own site.
                        linkUrl = if (provider.has_key) null else providerKeyUrl(provider.provider),
                    )
                }.filterNotNull(),
            ),
        )
    }

    /**
     * A stable colour per chain, so a network keeps its mark between launches.
     *
     * The fixtures hand-picked a colour per chain because they knew all eleven.
     * A live list does not, and a random colour that changes on every read
     * would make the list flicker.
     */
    private fun markColour(chainId: Long): Long = MARKS[(chainId % MARKS.size).toInt()]

    private val MARKS = listOf(
        0xFF6C7BFF, 0xFF2E9E7E, 0xFFE0A03A, 0xFF8C8C8C,
        0xFFCF5C7A, 0xFF4A9BD1, 0xFF9B6CD1, 0xFF3FA37A,
    )

    /** The label the drawn endpoints page already uses, in field order. */
    private fun endpointLabel(field: NetEndpointField, model: SettingsScreenModel): String {
        val drawn = model.endpoints.fields
        val index = NetEndpointField.entries.indexOf(field)
        return drawn.getOrNull(index)?.label ?: field.name
    }

    private fun providerName(provider: NetProviderId): String = when (provider) {
        NetProviderId.Alchemy -> "Alchemy"
        NetProviderId.Drpc -> "dRPC"
        NetProviderId.Ankr -> "Ankr"
    }

    /**
     * The row's right-hand value: the code and its sign, and nothing more —
     * a sample amount implies a rate, and while the choice is still on its
     * way (`pending`) there is not one yet.
     */
    private fun currencyRowValue(code: String): String =
        "$code · ${CurrencyCatalog.glyph(code)}"

    // -- Spec 047 US1: the rows that read the device, not a fixture -----------------

    /**
     * A format sheet with LIVE examples (spec 049 — the web's `formatSheet`
     * over `numberFormatOptions()`): each row is its preset's rendering of
     * the same sample, its id the wire key, and 自动 shows what this device's
     * locale resolves to. Before this the rows were the mocks' strings, so
     * 自动 said `1,234,567.89` on a German phone. The drawn rows' notes
     * (自动 · 系统, 印度计数, 24 小时制…) are kept by position — the enums are
     * in the fixture's order.
     */
    private fun <K> formatSheet(
        sheet: SelectSheetModel,
        keys: List<K>,
        wire: (K) -> String,
        chosen: String,
        example: (K) -> String,
    ): SelectSheetModel = sheet.copy(
        rows = keys.mapIndexed { i, key ->
            SelectRowModel(
                id = wire(key),
                label = example(key),
                note = sheet.rows.getOrNull(i)?.note,
                selected = wire(key) == chosen,
                mono = true,
            )
        },
    )

    /**
     * Language, the three formats and the text scale, from the person's
     * preferences: the rows say the choice, the sheets tick it,
     * the controls sit on it. `system` shows the resolved language beside it.
     */
    fun withPreferences(
        model: SettingsScreenModel,
        prefs: app.getvela.wallet.core.data.PrefsView,
        activeLanguage: String,
        strings: VelaStrings,
        theme: String,
    ): SettingsScreenModel {
        val endonym = { tag: String -> SettingsFixtures.LOCALE_ENDONYMS.firstOrNull { it.first == tag }?.second ?: tag }
        val languageValue = if (prefs.language == app.getvela.wallet.core.data.Preferences.AUTO_LANGUAGE) "${endonym(activeLanguage)} · ${strings.t(I18nKeys.SettingsUi.COMMON_SYSTEM)}" else endonym(prefs.language)
        val formats = Formats.current
        val numberSheet = formatSheet(model.numberSheet, NumberFormatKey.entries, { it.wire }, prefs.numberFormat.wire) {
            Formats(number = it, locale = formats.locale).example()
        }
        val dateSheet = formatSheet(model.dateSheet, DateFormatKey.entries, { it.wire }, prefs.dateFormat.wire) {
            Formats(date = it, locale = formats.locale).dateExample()
        }
        val timeSheet = formatSheet(model.timeSheet, TimeFormatKey.entries, { it.wire }, prefs.timeFormat.wire) {
            Formats(time = it, locale = formats.locale).timeExample()
        }
        return model.copy(
            sections = model.sections.map { section ->
                section.copy(
                    rows = section.rows.map { row ->
                        when (row.id) {
                            "language" -> row.copy(value = languageValue)
                            // The row shows the CURRENT rendering (the web's `currentExamples`).
                            "number-format" -> row.copy(value = formats.example())
                            "date-format" -> row.copy(value = formats.dateExample())
                            "time-format" -> row.copy(value = formats.timeExample())
                            else -> row
                        }
                    },
                )
            },
            languageSheet = model.languageSheet.copy(
                rows = model.languageSheet.rows.map { row -> row.copy(selected = row.id == prefs.language) },
            ),
            numberSheet = numberSheet,
            dateSheet = dateSheet,
            timeSheet = timeSheet,
            theme = model.theme.copy(selected = theme),
            textScale = model.textScale.copy(steps = TextScaleLevel.entries.size, index = prefs.textScale.ordinal),
        ).let { withDebugMode(it, prefs.debugMode) }
    }

    /** Spec 091: About's debug-mode switch as the preferences hold it — not drawn until revealed. */
    fun withDebugMode(model: SettingsScreenModel, mode: app.getvela.wallet.core.data.DebugMode): SettingsScreenModel =
        model.copy(about = model.about.copy(debugMode = model.about.debugMode.copy(mode = mode)))

    /** The storage page from the device's own keys (spec 047 D4). */
    fun withStorage(model: SettingsScreenModel, report: DeviceStorage.Report, strings: VelaStrings): SettingsScreenModel {
        val total = report.totalBytes
        val (amount, unit) = bytesText(total)
        fun size(bytes: Long) = bytesText(bytes).let { "${it.first} ${it.second}" }
        val groups = model.storage.groups.map { group ->
            group.copy(
                items = group.items.map { item ->
                    val measured = report.items.firstOrNull { it.id == item.id } ?: return@map item
                    val count = measured.records?.let { n ->
                        when (item.id) {
                            "contacts" -> strings.t(I18nKeys.SettingsUi.COUNT_CONTACTS, mapOf("count" to n.toString()))
                            "custom", "dapps" -> strings.t(I18nKeys.SettingsUi.COUNT_ITEMS, mapOf("count" to n.toString()))
                            else -> strings.t(I18nKeys.SettingsUi.COUNT_RECORDS, mapOf("count" to n.toString()))
                        }
                    }
                    item.copy(meta = listOfNotNull(count, size(measured.bytes)).joinToString(" · "))
                },
            )
        }
        val user = report.bytesOf(DeviceStorage.Group.User); val cache = report.bytesOf(DeviceStorage.Group.Cache); val sessions = report.bytesOf(DeviceStorage.Group.Sessions)
        val denom = (user + cache + sessions).coerceAtLeast(1)
        return model.copy(
            storage = model.storage.copy(
                amount = amount,
                unit = unit,
                summary = strings.t(I18nKeys.SettingsUi.STORAGE_SUMMARY, mapOf("count" to report.totalRecords.toString())),
                segments = model.storage.segments.map { seg ->
                    seg.copy(fraction = when (seg.id) { "user" -> user; "cache" -> cache; else -> sessions }.toFloat() / denom)
                },
                groups = groups,
            ),
        )
    }

    /**
     * The Connections group, one row per connected site (spec 070 — the web's
     * `withLiveConnections`, row for row). A grant is a standing permission, so
     * a person has to see WHICH sites hold one, not only how many. `id` is the
     * origin, so the row's clear names exactly the grant to revoke; the words
     * say "Disconnect" (one site), never "Disconnect all". With no site
     * connected the drawn "dApp permissions · 0" row stands.
     */
    fun withConnections(
        model: SettingsScreenModel,
        sites: List<app.getvela.wallet.feature.browser.core.DbrSiteView>,
        strings: VelaStrings,
    ): SettingsScreenModel {
        if (sites.isEmpty()) return model
        val label = strings.t(I18nKeys.SettingsUi.STORAGE_CONNECTIONS)
        return model.copy(
            storage = model.storage.copy(
                groups = model.storage.groups.map { group ->
                    if (group.label != label) return@map group
                    group.copy(
                        items = sites.map { site ->
                            StorageItemModel(
                                id = site.origin,
                                label = site.origin.substringAfter("://"),
                                meta = app.getvela.wallet.feature.browser.ExploreLive.shortAddress(site.address),
                                action = strings.t("explore.disconnect"),
                                destructive = true,
                            )
                        },
                    )
                },
            ),
        )
    }

    /** A byte count in 1024s — the core's, as every Vela writes it (this shell used 1000s) — in the person's number format. */
    internal fun bytesText(bytes: Long): Pair<String, String> {
        val display = uniffi.vela_core_uniffi.storageBytesDisplay(bytes.coerceAtLeast(0).toULong())
        val digits = when (display.unit) { "B" -> 0; "KB" -> 0; else -> 1 }
        return Formats.current.number(java.math.BigDecimal(display.value), digits, digits) to display.unit
    }

    /** About: the build's version and commit, the wallet's network count. */
    fun withAbout(model: SettingsScreenModel, version: String, commit: String, networkCount: Int, strings: VelaStrings): SettingsScreenModel = model.copy(
        about = model.about.copy(
            version = strings.t(I18nKeys.SettingsUi.ABOUT_VERSION, mapOf("version" to version, "commit" to commit)),
            rows = model.about.rows.map { row ->
                if (row.label == strings.t(I18nKeys.SettingsUi.ABOUT_NETWORKS_LABEL)) row.copy(value = strings.t(I18nKeys.SettingsUi.ABOUT_NETWORKS_VALUE, mapOf("count" to networkCount.toString()))) else row
            },
        ),
        sections = model.sections.map { section ->
            section.copy(rows = section.rows.map { row -> if (row.id == "about") row.copy(value = strings.t(I18nKeys.SettingsUi.ABOUT_VERSION, mapOf("version" to version, "commit" to commit))) else row })
        },
    )

    /** The corpus labels the report's environment lines wear — the preview's and the payload's alike. */
    fun feedbackLabels(strings: VelaStrings): BugReport.EnvironmentLabels = BugReport.EnvironmentLabels(
        version = strings.t(I18nKeys.SettingsUi.BUG_PREVIEW_VERSION),
        platform = strings.t(I18nKeys.SettingsUi.BUG_PREVIEW_PLATFORM),
        language = strings.t(I18nKeys.SettingsUi.BUG_PREVIEW_LANGUAGE),
        rpc = strings.t(I18nKeys.SettingsUi.BUG_PREVIEW_RPC),
        failures = strings.t(I18nKeys.SettingsUi.BUG_PREVIEW_FAILURES),
        none = strings.t(I18nKeys.SettingsUi.BUG_PREVIEW_NONE),
    )

    /**
     * Feedback: the preview lines are the device's — built by the same
     * function the payload's `environment` is ([BugReport.environmentLines]),
     * so "only what you see is sent" is literal, redaction included.
     */
    fun withFeedback(model: SettingsScreenModel, facts: BugReport.DeviceFacts, strings: VelaStrings): SettingsScreenModel = model.copy(
        feedback = model.feedback.copy(previewLines = BugReport.environmentLines(feedbackLabels(strings), facts)),
    )

    /**
     * The notice for an answer that arrived with the sheet closed (the founder:
     * a report always ends in something the person sees): filed → the success
     * title and 在 GitHub 查看; not filed → the fallback title and 打开 GitHub 表单.
     */
    fun withFeedbackNotice(model: SettingsScreenModel, notice: BugReport.Outcome?): SettingsScreenModel {
        val f = model.feedback
        val shown = when (notice) {
            is BugReport.Outcome.Filed -> FeedbackNoticeModel(f.successTitle, f.viewIssue, notice.url)
            is BugReport.Outcome.Fallback -> FeedbackNoticeModel(f.fallbackTitle, f.openGithub, notice.fallbackUrl)
            null -> null
        }
        return model.copy(feedback = f.copy(notice = shown))
    }

    /** Where the last 发送 stands: in flight, filed, or handed to the form. */
    fun withFeedbackStatus(model: SettingsScreenModel, sending: Boolean, outcome: BugReport.Outcome?): SettingsScreenModel = model.copy(
        feedback = model.feedback.copy(
            status = when {
                sending -> FeedbackStatus.Sending
                outcome is BugReport.Outcome.Filed -> FeedbackStatus.Filed(outcome.number, outcome.url, outcome.deduped, outcome.screenshotsDropped)
                outcome is BugReport.Outcome.Fallback -> FeedbackStatus.Fallback(outcome.fallbackUrl)
                else -> FeedbackStatus.Idle
            },
        ),
    )

    /** The relayer panel from the treasury probe: the address to fund and the shortfall, in the chain's coin. */
    fun withRelayer(model: SettingsScreenModel, chainName: String, chainId: Int, symbol: String, status: SendTreasuryStatus?, strings: VelaStrings): SettingsScreenModel {
        if (status == null) return model
        val decimals = if (status.asset == SendTreasuryAsset.PathUsd) 6 else 18
        val short = (status.floor.toBigDecimalOrNull() ?: java.math.BigDecimal.ZERO) - (status.balance.toBigDecimalOrNull() ?: java.math.BigDecimal.ZERO)
        val amount = SendLive.fromBase(short.max(java.math.BigDecimal.ZERO).toPlainString(), decimals)
        return model.copy(
            relayer = model.relayer.copy(
                mark = ChainMarkModel(tokenGlyph(symbol), WalletLive.badge(chainId.toLong()).value.toLong() and 0xFFFFFFFFL, Marks.chainLogoUrl(chainId.toInt())),
                name = chainName,
                amountHint = strings.t(I18nKeys.SettingsUi.RELAYER_AMOUNT_HINT, mapOf("amount" to amount, "symbol" to (if (status.asset == SendTreasuryAsset.PathUsd) "pathUSD" else symbol))),
                addressDisplay = ExploreLive.shortAddress(status.address),
            ),
        )
    }

    /** Where a provider's API key is made — the "Get key →" link's target. */
    internal fun providerKeyUrl(provider: NetProviderId): String = when (provider) {
        NetProviderId.Alchemy -> "https://dashboard.alchemy.com/"
        NetProviderId.Drpc -> "https://drpc.org/"
        NetProviderId.Ankr -> "https://www.ankr.com/rpc/"
    }

    /**
     * The saved RPC answered its probe: a save went out from the sheet, nothing
     * new is being typed, and the row's health is measured `ok` (the web's
     * `rpcRestored`). Only then does the sheet say Done — and Done is what
     * tells the balance core the chain is fixed.
     */
    fun rpcFixRestored(row: NetNetworkRow, draft: String?, saved: Boolean): Boolean =
        saved && draft == null && row.rpc_health is NetProbeHealth.Ok

    /**
     * SR2: the RPC fix for the chain the home's status line named — THIS
     * chain, its URL as the core holds it (or as it is being typed), and the
     * probe's word after a save (the web's `liveRpcFix`). The fixture drew
     * Polygon whichever chain had failed.
     */
    fun rpcFix(fallback: RpcFixModel, row: NetNetworkRow, draft: String?, saved: Boolean, strings: VelaStrings): RpcFixModel {
        val health = row.rpc_health
        val restored = rpcFixRestored(row, draft, saved)
        val checking = saved && draft == null && health == NetProbeHealth.Checking
        val badge = when {
            restored -> healthPill(health, strings) ?: StatusPillModel(SettingsTone.Ok, strings.t(I18nKeys.SettingsUi.NETWORK_ONLINE))
            checking -> StatusPillModel(SettingsTone.Neutral, strings.t(I18nKeys.SettingsUi.ADD_CHECKING_COMPATIBILITY))
            else -> StatusPillModel(SettingsTone.Error, strings.t(I18nKeys.SettingsUi.NETWORK_OFFLINE))
        }
        val mismatch = row.rpc_chain_mismatch
        return fallback.copy(
            title = strings.t(I18nKeys.SettingsUi.RPC_FIX_TITLE),
            mark = ChainMarkModel(row.display_name.take(1).uppercase(), markColour(row.chain_id), Marks.chainLogoUrl(row.chain_id.toInt())),
            name = row.display_name,
            meta = strings.t(I18nKeys.SettingsUi.CHAIN_ID, mapOf("chainId" to row.chain_id.toString())) + " · " + row.native_symbol,
            badge = badge,
            callout = when {
                mismatch != null -> CalloutModel(
                    CalloutTone.Danger,
                    strings.t(
                        I18nKeys.SettingsUi.RPC_CHAIN_MISMATCH,
                        mapOf("reported" to mismatch.reported_chain_id.toString(), "expected" to mismatch.expected_chain_id.toString()),
                    ),
                )
                restored -> CalloutModel(CalloutTone.Success, strings.t(I18nKeys.SettingsUi.RPC_FIX_RESTORED))
                else -> CalloutModel(CalloutTone.Warning, strings.t(I18nKeys.SettingsUi.RPC_FIX_WARNING))
            },
            field = fallback.field.copy(
                value = draft ?: row.rpc_url,
                badge = if (restored) badge else null,
                tone = if (restored) SettingsTone.Ok else SettingsTone.Error,
            ),
            primary = strings.t(if (restored) I18nKeys.SettingsUi.COMMON_DONE else I18nKeys.SettingsUi.RPC_FIX_SAVE),
            providersLabel = if (restored) null else fallback.providersLabel,
            providers = if (restored) emptyList() else fallback.providers,
            report = if (restored) null else fallback.report,
            restored = restored,
        )
    }

    /**
     * Spec 048: the network detail page for THIS network — name, chain, the
     * RPC and explorer overrides as the core holds them. Before this the page
     * showed the fixture's Ethereum whichever row was tapped.
     */
    fun networkDetail(fallback: NetworkDetailModel, row: NetNetworkRow, strings: VelaStrings): NetworkDetailModel = fallback.copy(
        title = row.display_name,
        subtitle = strings.t(I18nKeys.SettingsUi.CHAIN_ID, mapOf("chainId" to row.chain_id.toString())) + " · " + row.native_symbol,
        mark = ChainMarkModel(row.display_name.take(1).uppercase(), markColour(row.chain_id), Marks.chainLogoUrl(row.chain_id.toInt())),
        name = row.display_name,
        note = strings.t(if (row.is_custom) I18nKeys.SettingsUi.NETWORK_CUSTOM else I18nKeys.SettingsUi.NETWORK_BUILTIN_NOTE),
        // What the probes measured (the web's `liveNetworkDetail`): the header
        // pill is the RPC's, quiet "online" until one answered — the fixture
        // drew a latency nobody measured.
        badge = healthPill(row.rpc_health, strings)
            ?: StatusPillModel(SettingsTone.Neutral, strings.t(I18nKeys.SettingsUi.NETWORK_ONLINE)),
        rpc = fallback.rpc.copy(
            value = row.rpc_url,
            badge = healthPill(row.rpc_health, strings),
            tone = if (row.rpc_chain_mismatch != null) SettingsTone.Error else null,
        ),
        explorer = fallback.explorer.copy(value = row.explorer_url, badge = healthPill(row.explorer_health, strings)),
        // An RPC that answers for another chain is the one fault a latency
        // pill cannot show: it is fast, and it is wrong.
        callout = row.rpc_chain_mismatch?.let { mismatch ->
            CalloutModel(
                CalloutTone.Danger,
                strings.t(
                    I18nKeys.SettingsUi.RPC_CHAIN_MISMATCH,
                    mapOf("reported" to mismatch.reported_chain_id.toString(), "expected" to mismatch.expected_chain_id.toString()),
                ),
            )
        },
        chainId = row.chain_id,
    )

    /**
     * SR6 (spec 092): the list the home's "can't reach" line opens — every
     * network the core lists, in its order, each with what was last read there
     * (its worth in the display currency, masked while hidden) and its RPC fix
     * — where the core says an RPC fix belongs (`rpc_fixable`): a network
     * whose RPC answers and whose token list could not be loaded gets none,
     * since "Fix RPC" there sends a person to repair what is working.
     * Built from the live view on every composition, so a network that comes
     * back leaves the open sheet; the title is the home's own line.
     */
    fun unreachable(
        view: BalanceView,
        currency: CurrencyView,
        chainNames: Map<Int, String>,
        strings: VelaStrings,
    ): UnreachableModel {
        val money = WalletLive.Money.of(currency)
        val rows = view.unreachable_networks.map { network ->
            val id = network.chain_id
            val name = chainNames[id] ?: id.toString()
            // "Last seen {{amount}}" carries a worth in the display currency:
            // masked while hidden (the core withholds the figure and this
            // writes the mask), and WITHHELD while that currency is not the
            // person's yet — the line is not said then, its room kept, since
            // the sentence is built around the figure. Every other line
            // ("Not read yet", "Held nothing when last read") has no figure.
            val worth = network.last_seen_usd?.takeIf { !view.hidden }
            val withheld = worth != null && !money.settled
            val amount = worth?.let { money.fiat(it) } ?: MASK
            UnreachableRowModel(
                chainId = id,
                mark = ChainMarkModel(name.take(1).uppercase(), markColour(id.toLong()), Marks.chainLogoUrl(id)),
                name = name,
                line = if (withheld) "" else strings.t(network.line_key, mapOf("amount" to amount)),
                lineWithheld = withheld,
                action = if (network.rpc_fixable) strings.t(I18nKeys.SettingsUi.RPC_FIX) else null,
            )
        }
        return UnreachableModel(
            title = WalletLive.unreachableLine(view, strings, chainNames)
                ?: strings.t(I18nKeys.SettingsUi.UNREACHABLE_NONE),
            summary = if (rows.isEmpty()) null else strings.t(I18nKeys.SettingsUi.UNREACHABLE_BODY),
            rows = rows,
        )
    }

    /**
     * SR3: the balance by network (the web's `liveBalanceDetail`) — the chains
     * still being read and the chains that settled, from the same view the hero
     * sums. A rate-limited chain is grey with no button, because it heals by
     * itself; a chain the core lists in `unreachable_networks` (failed MINUS
     * rate-limited) is red with 立即重试. The fixture drew Polygon and Gnosis
     * and two invented amounts on every device.
     */
    fun balanceDetail(
        fallback: BalanceDetailModel,
        view: BalanceView,
        currency: CurrencyView,
        chainNames: Map<Int, String>,
        strings: VelaStrings,
    ): BalanceDetailModel {
        val money = WalletLive.Money.of(currency)
        fun name(id: Int) = chainNames[id] ?: id.toString()
        fun mark(id: Int) = name(id).let { ChainMarkModel(it.take(1).uppercase(), markColour(id.toLong()), Marks.chainLogoUrl(id)) }
        val pending = view.rate_limited_chain_ids.distinct().map { id ->
            BalanceDetailRowModel(
                id = id.toString(),
                mark = mark(id),
                name = name(id),
                status = strings.t(I18nKeys.SettingsUi.BALANCE_DETAIL_RETRYING),
                tone = SettingsTone.Neutral,
            )
        }.toMutableList()
        view.unreachable_networks.distinctBy { it.chain_id }.filter { network -> pending.none { it.id == network.chain_id.toString() } }.forEach { network ->
            val id = network.chain_id
            pending += BalanceDetailRowModel(
                id = id.toString(),
                mark = mark(id),
                name = name(id),
                // The row's short status is the core's to name (`status_key`,
                // PR 3 final note F21): "RPC unavailable" only where the RPC
                // is what failed, "Token list unavailable" where the RPC
                // answers and the list could not be loaded — a status as
                // short as its neighbours', where this shell had borrowed the
                // home line's whole sentence. A key this build has no words
                // for reads as the one status there was, never a dotted path.
                status = strings.t(network.status_key).takeIf { it.isNotBlank() && it != network.status_key }
                    ?: strings.t(I18nKeys.SettingsUi.BALANCE_DETAIL_FAILED),
                tone = SettingsTone.Error,
                // Reading again is right for both: it asks for the list again too.
                action = strings.t(I18nKeys.SettingsUi.BALANCE_DETAIL_RETRY),
            )
        }
        // Per chain, what the hero counts: an unpriced holding adds nothing
        // (the chain still settled, so it is listed).
        val perChain = linkedMapOf<Int, Double>()
        view.tokens.forEach { token ->
            val usd = (token.balance.toDoubleOrNull() ?: Double.NaN) * (token.price_usd ?: 0.0)
            if (usd.isFinite()) perChain[token.chain_id] = (perChain[token.chain_id] ?: 0.0) + usd
        }
        val done = perChain.entries
            .filter { (id, _) -> pending.none { it.id == id.toString() } }
            .sortedByDescending { it.value }
            .map { (id, usd) ->
                BalanceDetailRowModel(
                    id = id.toString(),
                    mark = mark(id),
                    name = name(id),
                    // A network's worth is a fiat figure: the mask while
                    // hidden, and withheld (`null` from the one helper) while
                    // the display currency is not the person's yet.
                    amount = if (view.hidden) MASK else money.fiat(usd),
                    amountWithheld = !view.hidden && !money.settled,
                )
            }
        val total = view.display_total_usd ?: view.cached_total_usd
        // "Total {{amount}}": the mask while hidden or unknown; while the
        // currency is on its way the line is not said, its room kept.
        val totalWithheld = !view.hidden && total != null && !money.settled
        return fallback.copy(
            title = strings.t(I18nKeys.SettingsUi.BALANCE_DETAIL_TITLE),
            summary = if (totalWithheld) {
                ""
            } else {
                strings.t(
                    I18nKeys.SettingsUi.BALANCE_DETAIL_TOTAL,
                    mapOf("amount" to (total?.takeIf { !view.hidden }?.let { money.fiat(it) } ?: MASK)),
                )
            },
            summaryWithheld = totalWithheld,
            pending = pending,
            done = done,
            // The hero's "some tokens couldn't be priced" is answered here by name.
            sectionUnpriced = strings.t(I18nKeys.Wallet.BALANCE_UNPRICED),
            unpriced = view.unpriced_tokens.map { token ->
                BalanceDetailRowModel(
                    id = "${token.chain_id}:${token.token_address ?: token.symbol}",
                    mark = mark(token.chain_id),
                    name = token.symbol,
                    status = "${name(token.chain_id)} · ${if (view.hidden) MASK else tokenAmountText(token.balance)}",
                )
            },
        )
    }

    private const val MASK = "••••"

    /**
     * The accounts sheet: the home's own switcher (spec 072) — this device's
     * accounts, the active one ticked, each with the total the balance core
     * keeps for it once the sheet asked (`SwitcherOpened`), in the display
     * currency. One builder for both places a person switches accounts.
     */
    fun withAccounts(
        model: SettingsScreenModel,
        accounts: List<Pair<String, String>>,
        activeIndex: Int,
        switcher: app.getvela.wallet.feature.wallet.core.BalanceSwitcherView,
        currency: app.getvela.wallet.feature.settings.core.CurrencyView,
        strings: VelaStrings,
    ): SettingsScreenModel = model.copy(
        accountsSheet = app.getvela.wallet.feature.wallet.WalletLive.accountSwitcher(accounts, activeIndex, switcher, currency, strings),
    )

    /**
     * The keys that control this wallet, with their Ethereum backup beneath
     * them (spec 062) — ONE block, under the account it belongs to. A person
     * offered "back up your keys" is owed the sight of them first.
     *
     * `keys == null` is "still asking": a title and no guessed count. A registry
     * that did not answer leaves the device's own memory on screen, labelled,
     * without sync badges; one that answered with nothing is NOT called
     * unreachable. The backup row is a button only while there is something to
     * do, and absent where there is no registry on Ethereum or no registration.
     */
    fun withWalletKeys(
        model: SettingsScreenModel,
        keys: app.getvela.wallet.feature.settings.core.WalletKeys.Result?,
        backup: app.getvela.wallet.feature.settings.core.RegistryBackup.Check?,
        strings: VelaStrings,
        /** Spec 102: the account's signing domain — said only when it is not `getvela.app`. */
        signingDomain: String = "getvela.app",
    ): SettingsScreenModel {
        val k = I18nKeys.SettingsUi
        val rows = keys?.rows.orEmpty().mapIndexed { index, row ->
            val body = row.publicKeyHex.removePrefix("0x").let { if (it.length == 130 && it.startsWith("04")) it.drop(2) else it }
            WalletKeyRowModel(
                name = row.key.name.ifEmpty { strings.t(k.KEYS_KEY_N).replace("{{n}}", (index + 1).toString()) },
                // Spec 102: captioned by where the key LIVES — the vault when the
                // catalog knows it, else its place — never by a page it was made on.
                holder = row.key.providerName.ifEmpty {
                    strings.t(
                        when (row.key.method) {
                            app.getvela.wallet.feature.onboarding.core.KeyMethod.SecurityKey -> k.KEYS_PROVIDER_SECURITY_KEY
                            app.getvela.wallet.feature.onboarding.core.KeyMethod.Hybrid -> k.KEYS_PROVIDER_GENERIC
                            app.getvela.wallet.feature.onboarding.core.KeyMethod.Platform -> k.KEYS_PROVIDER_PLATFORM
                        },
                    )
                },
                fingerprint = if (body.length >= 8) "${body.take(4)}…${body.takeLast(4)}".lowercase() else "",
                pills = listOfNotNull(
                    // First: the key this device signs with (founder, 2026-09-26).
                    if (row.signsHere) KeyPillModel(strings.t(k.KEYS_SIGNS_HERE), KeyPillTone.SignsHere) else null,
                    if (row.userVerified == true) KeyPillModel(strings.t(k.KEYS_USER_VERIFIED), KeyPillTone.Verified) else null,
                    row.synced?.let { KeyPillModel(strings.t(if (it) k.KEYS_SYNCED else k.KEYS_NOT_SYNCED), if (it) KeyPillTone.Synced else KeyPillTone.Local) },
                ),
                // The registry explorer's facts, in its order; what is absent is left out.
                details = listOf(
                    KeyDetailModel(strings.t(k.KEYS_PUBLIC_KEY), if (row.publicKeyHex.isEmpty()) "" else "0x${row.publicKeyHex.removePrefix("0x")}", mono = true, copy = true),
                    KeyDetailModel(strings.t(k.KEYS_CREDENTIAL), row.credentialId, mono = true, copy = true),
                    KeyDetailModel("AAGUID", row.key.aaguid, mono = true, copy = false),
                    KeyDetailModel(strings.t(k.KEYS_TRANSPORT), listOf(row.key.authenticatorAttachment, row.key.transports).filter { it.isNotEmpty() }.joinToString(" · "), mono = false, copy = false),
                    KeyDetailModel(strings.t(k.KEYS_ATTESTATION), row.attestationHex, mono = true, copy = false),
                ).filter { it.value.isNotEmpty() },
                key = row.key,
            )
        }
        return model.copy(
            keys = WalletKeysModel(
                title = strings.t(k.KEYS_TITLE),
                subtitle = strings.t(k.KEYS_SUBTITLE),
                count = if (keys == null) "" else rows.size.toString(),
                loading = keys == null,
                note = if (keys?.source == app.getvela.wallet.feature.settings.core.WalletKeys.Source.Device) strings.t(k.KEYS_FROM_DEVICE) else null,
                rows = rows,
                // Spec 102: a wallet on its own domain says so — its keys
                // belong to that site, and only a page there can use them.
                domain = signingDomain.takeIf { !it.equals("getvela.app", ignoreCase = true) && it.isNotBlank() }
                    ?.let { strings.t("settings.signing.keysOn", mapOf("domain" to it)) },
                backup = ethereumBackupRow(backup, strings),
                // While the walk is still running the paragraph is the
                // explanation itself; once it has answered, the core says
                // whether this state has one (`explain_key`) — a wallet that
                // can never be copied is not told how a copy is made.
                backupExplain = if (backup == null) strings.t(k.BACKUP_EXPLAIN) else backup.row?.explainKey?.let { strings.t(it) },
                copyLabel = strings.t(k.KEYS_COPY),
                copiedLabel = strings.t(k.KEYS_COPIED),
            ),
        )
    }

    /**
     * The backup as a row — the core's words, tone and tap (`BackupState::row`),
     * drawn and not re-mapped: every shell used to map the state itself, and
     * they disagreed.
     *
     * - still asking ([check] `null`): the title and "Checking…", inert;
     * - "Copied to Ethereum": said in the positive tone, inert;
     * - "Not copied yet (optional)": a plain state, never a caution — a copy
     *   is optional and costs a fee — and a tap opens the sheet (the chevron);
     * - "Couldn't check. Tap to try again.": a tap asks again, with the glyph
     *   that says so rather than a chevron that would promise a page (the
     *   founder's ruling, 2026-09-23);
     * - "This older wallet can't be copied": a calm end, nothing to tap;
     * - no row from the core (no registry on Ethereum, no record): not drawn.
     *
     * A row with nothing to do takes no taps at all (dead-controls #8): no
     * chevron and no ripple.
     */
    fun ethereumBackupRow(
        check: app.getvela.wallet.feature.settings.core.RegistryBackup.Check?,
        strings: VelaStrings,
    ): SettingsRowModel? {
        val k = I18nKeys.SettingsUi
        val row = if (check == null) null else check.row ?: return null
        val trailing = when (row?.action) {
            app.getvela.wallet.feature.settings.core.RegistryBackup.Action.Copy -> RowTrailing.Chevron
            app.getvela.wallet.feature.settings.core.RegistryBackup.Action.Retry -> RowTrailing.Retry
            app.getvela.wallet.feature.settings.core.RegistryBackup.Action.None, null -> RowTrailing.None
        }
        return SettingsRowModel(
            id = ETHEREUM_BACKUP_ROW,
            title = strings.t(row?.titleKey ?: k.BACKUP_TITLE),
            icon = SettingsIcon.Upload,
            subtitle = strings.t(row?.subtitleKey ?: k.BACKUP_CHECKING),
            subtitlePositive = row?.tone == app.getvela.wallet.feature.settings.core.RegistryBackup.Tone.Positive,
            trailing = trailing,
            actionable = trailing != RowTrailing.None,
        )
    }

    const val ETHEREUM_BACKUP_ROW = "ethereum-backup"
}
