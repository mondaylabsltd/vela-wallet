package app.getvela.wallet.feature.agent

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.data.ThemePreference
import app.getvela.wallet.core.format.DateFormatKey
import app.getvela.wallet.core.format.NumberFormatKey
import app.getvela.wallet.core.format.TextScaleLevel
import app.getvela.wallet.core.format.TimeFormatKey
import app.getvela.wallet.feature.contacts.core.ContactGroupInput
import app.getvela.wallet.feature.contacts.core.ContactSaveInput
import app.getvela.wallet.feature.send.core.FeeTier
import app.getvela.wallet.feature.settings.core.NetEndpointField
import app.getvela.wallet.feature.settings.core.NetOverrideField
import app.getvela.wallet.feature.settings.core.NetProviderId
import kotlinx.coroutines.delay
import kotlinx.serialization.KSerializer
import kotlinx.serialization.json.JsonPrimitive
import org.json.JSONArray
import org.json.JSONObject

/**
 * Doing an intent (spec 103): the calls this app's own controls make — the
 * same controller, the same method — so a person's tap and an agent's request
 * reach the rule that owns the change by one road. The desktop's `perform.rs`,
 * the iPhone's `AgentPerform.swift` and the web's `perform.ts`, action for
 * action, each over its own app's way of doing it: the token sheet searches
 * every network, as its screen does here.
 *
 * Transfers are not here: drafting one onto the review and following it is
 * the core's (`vela-agent` transfers, over [AgentShellBridge]'s send). Seeing
 * the change land is the runner's.
 *
 * Every answer is `{"done":true}` or `{"refused":{"code":…,"detail":…}}`. Runs
 * on the main thread.
 */
internal class AgentPerform(private val wallet: AgentWallet, private val agent: AgentStore) {

    suspend fun perform(actionJson: String): String {
        val action = runCatching { JSONObject(actionJson) }.getOrNull() ?: return refused("invalid_input", "not an action")
        val text = { key: String -> action.optString(key).trim() }
        val chainId = action.optLong("chain_id")
        return when (val kind = action.optString("type")) {
            "show" -> {
                val page = agent.page ?: return NOT_OPEN
                page.show(action.optJSONObject("screen") ?: JSONObject())?.let { refused(it) } ?: DONE
            }
            "switch_account" ->
                if (wallet.switchAccount(text("address"))) DONE else refused("no wallet on this device has that address")
            "remove_wallet" -> {
                val address = text("address")
                val row = wallet.session.view.value.accounts.firstOrNull { it.address.equals(address, ignoreCase = true) }
                    ?: return refused("no wallet on this device has that address")
                wallet.session.removeAccount(row.index)
                DONE
            }
            // Confirmed on the session's own sheet, as Settings' row asks it.
            "sign_out" -> DONE.also { wallet.session.signOut() }
            "set_preference" -> setPreference(action.optJSONObject("preference") ?: JSONObject())
            "save_contact" -> DONE.also {
                wallet.contacts.save(ContactSaveInput(address = text("address"), name = text("name")))
            }
            "delete_contact" -> DONE.also { wallet.contacts.delete(action.optString("address")) }
            "save_group" -> DONE.also {
                val id = action.optString("id").takeIf { !action.isNull("id") && it.isNotEmpty() }
                wallet.contacts.saveGroup(ContactGroupInput(id = id, name = text("name")))
            }
            "delete_group" -> DONE.also { wallet.contacts.deleteGroup(action.optString("id")) }
            "set_group_members" -> DONE.also {
                wallet.contacts.setGroupMembers(action.optString("id"), strings(action.optJSONArray("members")))
            }
            "set_contact_groups" -> DONE.also {
                wallet.contacts.setContactGroups(action.optString("address"), strings(action.optJSONArray("group_ids")))
            }
            // The star is a toggle: the check already said it is not so yet.
            "set_contact_favorite" -> DONE.also { wallet.contacts.toggleFavourite(action.optString("address")) }
            "erase_device" -> {
                val left = wallet.erase()
                agent.eraseLeft.value = left.takeIf { it.isNotEmpty() }
                if (left.isEmpty()) DONE else refused("${left.size} stored item(s) could not be erased")
            }
            "clear_storage_item" -> DONE.also {
                wallet.clearStorageItem(action.optString("item"))
                agent.storageCleared.value += 1
            }
            "clear_caches" -> DONE.also {
                wallet.clearCaches()
                agent.storageCleared.value += 1
            }
            "configure_agent" -> DONE.also {
                agent.apply(
                    enabled = if (action.has("enabled") && !action.isNull("enabled")) action.optBoolean("enabled") else null,
                    url = action.optString("bridge_url").takeIf { action.has("bridge_url") && !action.isNull("bridge_url") },
                    token = action.optString("client_token").takeIf { action.has("client_token") && !action.isNull("client_token") },
                )
            }
            "add_token" -> addToken(chainId.toInt(), text("address"))
            // History's "Delete record": the feed tombstones it and drops the row.
            "delete_activity" -> DONE.also { wallet.wallet.deleteActivity(action.optString("id")) }
            "set_balance_privacy" -> DONE.also {
                // The figure is a toggle: pressed only when it is not already so.
                if (wallet.wallet.balances.value.hidden != action.optBoolean("hidden")) wallet.wallet.togglePrivacy()
            }
            "refresh_balances" -> DONE.also { wallet.wallet.pullRefresh() }
            "add_network" -> addNetwork(chainId, text("rpc_url").takeIf { !action.isNull("rpc_url") && it.isNotEmpty() })
            "set_network_endpoint" -> setNetworkEndpoint(chainId, action.optString("field"), text("url"))
            "remove_network" -> DONE.also {
                val row = networks().optJSONArray("networks").objects()
                    .firstOrNull { it.optLong("chain_id") == chainId && it.optBoolean("is_custom") }
                wallet.settings.deleteNetwork(row?.optString("id") ?: "custom-$chainId")
            }
            "set_provider_key" -> setProviderKey(action.optString("provider"), text("key"))
            "set_service_endpoint" -> setServiceEndpoint(action.optString("field"), text("url"))
            "reset_service_endpoints" -> DONE.also { wallet.settings.resetEndpoints() }
            else -> refused("this wallet cannot do $kind here")
        }
    }

    // -- preferences -------------------------------------------------------------

    /** As Settings' own controls set them: stored, then applied now. */
    private suspend fun setPreference(preference: JSONObject): String {
        val word = preference.optString("value")
        val prefs = wallet.preferences
        when (preference.optString("kind")) {
            "theme" -> wallet.setTheme(
                ThemePreference.entries.firstOrNull { it.storageValue == word } ?: return refused("invalid_input", "not a theme"),
            )
            "language" -> {
                prefs.setLanguage(word)
                wallet.applyLanguage(word)
            }
            "text_scale" -> prefs.setTextScale(TextScaleLevel.entries.firstOrNull { it.wire == word } ?: return invalid())
            "number_format" -> prefs.setNumberFormat(NumberFormatKey.entries.firstOrNull { it.wire == word } ?: return invalid())
            "date_format" -> prefs.setDateFormat(DateFormatKey.entries.firstOrNull { it.wire == word } ?: return invalid())
            "time_format" -> prefs.setTimeFormat(TimeFormatKey.entries.firstOrNull { it.wire == word } ?: return invalid())
            "currency" -> wallet.settings.chooseCurrency(word)
            // Rapid is a one-shot pick on the send form, never the default.
            "fee_speed" -> named(FeeTier.serializer(), word)?.takeIf { it != FeeTier.Rapid }?.let { wallet.settings.chooseFeeTier(it) }
            "analytics" -> return refused(NO_ANALYTICS)
            else -> return invalid()
        }
        return DONE
    }

    private fun invalid() = refused("invalid_input", "not a value this wallet offers")

    // -- networks and services ---------------------------------------------------

    /** The network ledger's view, in the core's own words. */
    private fun networks(): JSONObject = wallet.settings.networksJson.value?.let { runCatching { JSONObject(it) }.getOrNull() } ?: JSONObject()

    /**
     * Settings → Add network: look the chain up, try [rpcUrl] first if given,
     * check the wallet can serve it, and add it. A wizard already checked for
     * this chain (the person's own, at its Add) is not checked again.
     */
    private suspend fun addNetwork(chainId: Long, rpcUrl: String?): String {
        val settings = wallet.settings
        val wizard = networks().optJSONObject("wizard") ?: JSONObject()
        val checked = wizard.optString("phase") == "checked" && wizard.optBoolean("can_add") &&
            wizard.optJSONObject("chain_info")?.optLong("chain_id") == chainId &&
            (rpcUrl == null || wizard.optString("custom_rpc").trim() == rpcUrl)
        if (!checked) {
            settings.resetWizard()
            settings.selectChain(chainId)
            if (rpcUrl != null) {
                settings.editCustomRpc(rpcUrl)
                settings.selectChain(chainId, keepCustomRpc = true)
            }
        }
        // `""` once it can be added, else why not.
        val settled = until(CHECK_MS) {
            val now = networks().optJSONObject("wizard") ?: return@until null
            when {
                now.optString("phase") == "error" -> "add network: ${name(now.opt("error"))}"
                now.optString("phase") != "checked" -> null
                now.optBoolean("can_add") -> ""
                else -> {
                    val compat = now.optJSONObject("compat")
                    val why = listOf("blocker", "rpc_failure").firstNotNullOfOrNull { key ->
                        compat?.takeIf { !it.isNull(key) }?.opt(key)?.let(::name)
                    }
                    "add network: ${why ?: "not compatible"}"
                }
            }
        } ?: return refused("not_observed", "the network could not be checked in time")
        if (settled.isNotEmpty()) return refused(settled)
        settings.confirmAddNetwork()
        return DONE
    }

    /**
     * One network's card: open it, edit the field, leave it (which saves). An
     * RPC goes through the core's save gate: one answering for another chain
     * is not written, and the field is put back to what is in effect.
     */
    private suspend fun setNetworkEndpoint(chainId: Long, field: String, url: String): String {
        val wire = named(NetOverrideField.serializer(), field) ?: return refused("invalid_input", "not a network field")
        val settings = wallet.settings
        val row = { networks().optJSONArray("networks").objects().firstOrNull { it.optLong("chain_id") == chainId } }
        val before = row()?.optString(if (wire == NetOverrideField.Rpc) "rpc_url" else "explorer_url")
        if (before != url) {
            settings.expandOverride(chainId)
            settings.editOverride(chainId, wire, url)
        }
        settings.commitOverride(chainId)
        if (wire != NetOverrideField.Rpc) return DONE
        val gate = until(GATE_MS) {
            val now = row() ?: return@until null
            when {
                !now.isNull("rpc_chain_mismatch") && now.has("rpc_chain_mismatch") -> false
                now.optBoolean("rpc_save_deferred") -> null
                else -> true
            }
        }
        if (gate == false) {
            if (before != null) settings.editOverride(chainId, wire, before)
            return refused("that RPC answers for another chain; the network keeps the RPC it had")
        }
        return DONE
    }

    /**
     * RPC providers: open the page (it seeds the drafts), type the key, leave
     * it. A key already in its field is only left — opening the page again
     * would reseed every draft.
     */
    private fun setProviderKey(provider: String, key: String): String {
        val wire = named(NetProviderId.serializer(), provider) ?: return refused("invalid_input", "not a provider")
        val settings = wallet.settings
        val typed = networks().optJSONArray("providers").objects()
            .any { it.optString("provider") == provider && it.optString("key") == key }
        if (!typed) {
            settings.openProviders()
            settings.editProviderKey(wire, key)
        }
        settings.commitProviderKey(wire)
        return DONE
    }

    /** Service endpoints: open the page, type the URL, leave it — or only leave it. */
    private fun setServiceEndpoint(field: String, url: String): String {
        val wire = named(NetEndpointField.serializer(), field) ?: return refused("invalid_input", "not a service endpoint")
        val settings = wallet.settings
        val typed = networks().optJSONArray("endpoints").objects()
            .any { it.optString("field") == field && it.optString("value") == url }
        if (!typed) {
            settings.openEndpoints()
            settings.editEndpoint(wire, url)
        }
        settings.commitEndpoint(wire)
        return DONE
    }

    // -- tokens ------------------------------------------------------------------

    /**
     * The add-token sheet's own road: the address typed (the sheet searches
     * every network for it), then the card for the network named saved.
     */
    private suspend fun addToken(chainId: Int, address: String): String {
        val tokens = wallet.wallet
        tokens.openAddToken()
        tokens.addTokenInput(address)
        val typed = until(TOKEN_STEP_MS) {
            tokens.manageTokens.value.takeIf { it.input_address == address }?.address_valid
        }
        if (typed != true) return refused("invalid_input", "that is not a contract address")
        val searched = until(TOKEN_STEP_MS) {
            tokens.manageTokens.value.takeIf { !it.detecting && (it.found.isNotEmpty() || it.not_found || it.native_alias) }
        } ?: return refused("not_observed", "the token could not be looked up in time")
        if (searched.native_alias) return refused("that address is the network's own coin, already in the wallet")
        val card = searched.found.firstOrNull { it.chain_id == chainId }
            ?: return refused("not_found", "no token answers at that address on that network")
        if (card.added) return DONE
        tokens.addTokenSave(chainId)
        val saved = until(TOKEN_STEP_MS) {
            val view = tokens.manageTokens.value
            val stored = view.found.any { it.chain_id == chainId && it.added }
            if (!view.saving && (view.save_error || stored)) view.save_error else null
        }
        return when (saved) {
            null -> refused("not_observed", "the token was not seen stored in time")
            true -> refused("the token could not be stored")
            false -> DONE
        }
    }

    // -- -------------------------------------------------------------------------

    /** Read [read] until it answers, or [ms] pass. */
    private suspend fun <T> until(ms: Long, read: () -> T?): T? {
        var waited = 0L
        while (true) {
            read()?.let { return it }
            if (waited >= ms) return null
            delay(50)
            waited += 50
        }
    }

    /** An enum of the core's by its wire name. */
    private fun <T> named(serializer: KSerializer<T>, word: String): T? =
        runCatching { Wire.json.decodeFromJsonElement(serializer, JsonPrimitive(word)) }.getOrNull()

    companion object {
        const val DONE = """{"done":true}"""
        private val NOT_OPEN = refused("not_ready", "the wallet is not open")

        /** The phone shares no usage statistics: there is no switch to turn. */
        const val NO_ANALYTICS = "this wallet shares no usage statistics: there is nothing to switch"

        /** How long the add wizard may take to look a chain up and check it. */
        private const val CHECK_MS = 45_000L
        /** How long the RPC save gate may take to hear the endpoint's chain id. */
        private const val GATE_MS = 20_000L
        /** How long the token search and the save may each take. */
        private const val TOKEN_STEP_MS = 30_000L

        fun refused(code: String, detail: String): String =
            JSONObject().put("refused", JSONObject().put("code", code).put("detail", detail)).toString()

        /** Refused, in words. */
        fun refused(detail: String): String = refused("refused", detail)

        /** A core value by its own name, as a refusal reads it: an enum's word, or a tagged one's `type`. */
        private fun name(value: Any?): String = when (value) {
            is String -> value
            is JSONObject -> value.optString("type").ifEmpty { value.toString() }
            null, JSONObject.NULL -> ""
            else -> value.toString()
        }

        private fun strings(array: JSONArray?): List<String> =
            array?.let { list -> (0 until list.length()).map { list.optString(it) } }.orEmpty()

        private fun JSONArray?.objects(): List<JSONObject> =
            this?.let { list -> (0 until list.length()).mapNotNull { list.optJSONObject(it) } }.orEmpty()
    }
}
