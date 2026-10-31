package app.getvela.wallet.feature.multisig

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.multisig.core.MsAccount
import app.getvela.wallet.feature.multisig.core.MsAccountsView
import app.getvela.wallet.feature.multisig.core.MsProposalItem
import app.getvela.wallet.feature.multisig.core.MsRules
import app.getvela.wallet.feature.multisig.core.MultisigShellFailure
import app.getvela.wallet.feature.multisig.core.arr
import app.getvela.wallet.feature.multisig.core.bool
import app.getvela.wallet.feature.multisig.core.get
import app.getvela.wallet.feature.multisig.core.int
import app.getvela.wallet.feature.multisig.core.isNullish
import app.getvela.wallet.feature.multisig.core.kind
import app.getvela.wallet.feature.multisig.core.str
import app.getvela.wallet.feature.signing.trustedsigner.VenueWords
import kotlinx.serialization.json.JsonElement

/**
 * How a multisig and its transactions read on screen — the web's
 * `core/display.ts` and `shell/proposal-failure.ts`, iOS's `MultisigWords`:
 * member names (this device's own, or the name a multisig keeps — never text
 * that came with a link or a proposal), a transaction's one line, a rule, why
 * an action did not go. Every verdict is the core's; this only puts the
 * corpus's words on it (`home.multisig.*`, the on-demand catalog the runtime
 * merges in, `I18nCatalogs.MULTISIG`).
 */
class MultisigWords(
    val strings: VelaStrings,
    /** What the multisigs here keep of members (`known_members`). */
    private val accounts: MsAccountsView = MsAccountsView(),
    /** This device's Vela accounts (lowercased address → name). */
    private val localNames: Map<String, String> = emptyMap(),
    /** A network's name, from this device's list. */
    private val networkName: (Int) -> String? = { null },
    /** A network's coin symbol. */
    private val nativeSymbol: (Int) -> String? = { null },
    /** r8-2: the core's facts of a token (`tokens` in the proposals view): symbol and decimals, `null` decimals unknown. */
    private val tokenFacts: (Int, String) -> JsonElement? = { _, _ -> null },
) {
    fun t(key: String, vararg vars: Pair<String, String>): String =
        if (vars.isEmpty()) strings.t("home.multisig.$key") else strings.t("home.multisig.$key", vars.toMap())

    /** A plural key: [count] picks the form (the core's CLDR rule) and fills `{{count}}`. */
    fun plural(key: String, count: Int): String = strings.t("home.multisig.$key", count)

    /** Any corpus key, outside `home.multisig`. */
    fun raw(key: String, vararg vars: Pair<String, String>): String =
        if (vars.isEmpty()) strings.t(key) else strings.t(key, vars.toMap())

    // -- names ----------------------------------------------------------------

    fun localName(address: String): String? = localNames[address.lowercase()]?.ifBlank { null }

    /** What the multisigs here keep of a member (a label, never authority). */
    fun keptName(address: String): String? = accounts.lookup(accounts.knownMembers, address)?.name?.ifBlank { null }

    /** A member as a line names it: its name and short address — a name is a claim, the address is who gets the say. */
    fun who(address: String): String {
        val short = short(address)
        val name = label(address)
        return if (name == short) short else "$name ($short)"
    }

    /**
     * A member's label — the core's rule (`names.label`, r8-25: the name the
     * multisigs keep, else the passkey index's, else this device's own
     * account name, else the short address), so a member reads the same on
     * every device. This shell only hands it what it knows.
     */
    fun label(address: String): String =
        MsRules.string("names.label", mapOf("address" to address, "own" to localName(address), "kept" to keptName(address), "registered" to null))
            ?: localName(address) ?: keptName(address) ?: short(address)

    /** A multisig's name: its row's, its record's (when valid), its label, else its rule ("Multisig 2/3"). */
    fun name(account: MsAccount, rowName: String? = null): String {
        if (!rowName.isNullOrBlank()) return rowName
        account.genesisName?.takeIf { MsRules.validName(it) }?.let { return it }
        account.label?.takeIf { it.isNotBlank() }?.let { return it }
        return t("switcher.rowName", "m" to account.threshold.toString(), "n" to account.owners.size.toString())
    }

    /** "Needs 2 of 3 members" — "Needs its only member's approval" for one. */
    fun ruleTitle(m: Int, n: Int): String = if (n == 1) t("panel.titleSolo") else t("panel.title", "m" to "$m", "n" to "$n")

    /** The header's chip: "Needs 2 of 3", "One member". */
    fun ruleChip(m: Int, n: Int): String = if (n == 1) t("panel.chipSolo") else t("panel.chip", "m" to "$m", "n" to "$n")

    fun ruleText(m: Int, n: Int): String = t("create.thresholdValue", "m" to "$m", "n" to "$n")

    // -- networks ---------------------------------------------------------------

    fun network(chainId: Int): String = networkName(chainId) ?: "Chain $chainId"

    fun symbol(chainId: Int): String = nativeSymbol(chainId) ?: "ETH"

    /**
     * A network this device lacks, titled: the name given, else the core's
     * catalog's (`networks.name`), else the device's own index — else its id alone.
     */
    fun missingTitle(chainId: Int, name: String? = null): String {
        val named = name ?: MsRules.networkName(chainId) ?: networkName(chainId)
        return if (named != null) t("proposal.missingNetworkNamed", "name" to named, "chainId" to "$chainId")
        else t("proposal.missingNetwork", "chainId" to "$chainId")
    }

    /** The header's rule chip as the machine says it (`MultisigRuleChip`). */
    fun ruleChip(chip: kotlinx.serialization.json.JsonElement?): String? = when (chip.kind) {
        "needs" -> chip["rule"].let { r -> if (r["solo"].bool == true) t("panel.chipSolo") else t("panel.chip", "m" to (r["m"].str ?: ""), "n" to (r["n"].str ?: "")) }
        "differs" -> t("presence.ruleDiffers")
        else -> null
    }

    /**
     * r9-14: a multisig's rule chip wherever it is drawn (home, switcher):
     * the core's `rule_chip` for the multisig the proposals machine follows
     * (its members as the chains say them now), else what its record says —
     * the core reads no chain for one it does not follow.
     */
    fun rowRuleChip(account: app.getvela.wallet.feature.multisig.core.MsAccount, proposals: app.getvela.wallet.feature.multisig.core.MsProposalsView): String =
        (if (proposals.followed.equals(account.address, ignoreCase = true)) ruleChip(proposals.ruleChip) else null)
            ?: ruleChip(account.threshold, account.owners.size)

    /** The rule's title as the machine's chip says it ("Needs 2 of 4 members"); `null` when it says nothing, or differs by network. */
    fun ruleTitleOf(chip: kotlinx.serialization.json.JsonElement?): String? =
        if (chip.kind == "needs") chip["rule"].let { r -> r["m"].int?.let { m -> r["n"].int?.let { n -> ruleTitle(m, n) } } } else null

    /** The waiting line as the machine parts it (`MultisigWaitingPart[]`): "1 needs your approval · 1 ready to execute". */
    fun waitingParts(parts: List<kotlinx.serialization.json.JsonElement>): String? =
        parts.mapNotNull { p -> p["kind"].str?.let { plural("presence.$it", p["count"].int ?: 0) } }
            .takeIf { it.isNotEmpty() }?.joinToString(" · ")

    // -- a transaction's one line ------------------------------------------------

    /**
     * "Send 500 USDC", "Add Bob (0x12…34)" — the proposal's own title. A
     * transfer of a token says the symbol [token] gives (`null`: the chain's
     * coin when it is one, else the amount alone).
     */
    fun actionTitle(chainId: Int, action: JsonElement?, nonce: String, token: Pair<String, Int>? = null): String {
        if (action.isNullish) return t("action.unsupported")
        return when (action.kind) {
            "transfer" -> {
                val raw = action["amount"].str ?: "0"
                val address = action["token"].str
                if (address == null) return t("action.send", "amount" to MsRules.formatUnits(raw, 18), "symbol" to symbol(chainId)).trim()
                // r8-2: a token's amount is worded with ITS decimals — the core's
                // facts, else the caller's — and never with 18 assumed: unknown
                // decimals say the base units and that they are unknown.
                val facts = tokenFacts(chainId, address)
                val decimals = facts?.get("decimals").int ?: token?.second?.takeIf { facts == null }
                val sym = facts?.get("symbol").str?.takeIf { it.isNotBlank() } ?: token?.first?.takeIf { it.isNotBlank() } ?: short(address)
                if (decimals == null) t("action.send", "amount" to t("review.reading.decimalsUnknown", "raw" to raw), "symbol" to sym).trim()
                else t("action.send", "amount" to MsRules.formatUnits(raw, decimals), "symbol" to sym).trim()
            }
            "batch" -> {
                val calls = action["calls"].arr
                val allTransfers = calls.all { it["action"].kind == "transfer" }
                t(if (allTransfers) "action.batch" else "action.batchCalls", "total" to calls.size.toString())
            }
            "cancel" -> t("action.cancel", "nonce" to nonce)
            "addOwner" -> t("change.add", "member" to who(action["owner"].str.orEmpty()))
            "removeOwner" -> t("change.remove", "member" to who(action["owner"].str.orEmpty()))
            "swapOwner" -> t("change.replace", "old" to who(action["oldOwner"].str.orEmpty()), "new" to who(action["newOwner"].str.orEmpty()))
            "changeThreshold" -> t("action.changeThreshold", "threshold" to (action["threshold"].str ?: ""))
            "rename" -> t("action.rename", "name" to (action["name"].str ?: ""))
            "call" -> {
                val value = action["value"].str ?: "0"
                if (value == "0" || value.isEmpty()) t("action.call")
                else t("action.callWithValue", "amount" to MsRules.formatUnits(value, 18), "symbol" to symbol(chainId))
            }
            else -> t("action.unsupported")
        }
    }

    // -- a row's one word (`standings`) ----------------------------------------------

    fun standing(standing: JsonElement?, item: MsProposalItem): String = when (standing.kind) {
        "unsent" -> t("status.unsent")
        "executing" -> t("status.executing")
        "queued" -> t("status.queued")
        "ready" -> t("status.ready")
        "yourTurn" -> t("status.yourTurn")
        "blocked" -> t("status.cannotRun", "reason" to blockReason(standing["block"]))
        "progress" -> t(
            "status.progress",
            "approved" to (standing["approved"].str ?: item.approved.toString()),
            "threshold" to (standing["threshold"].str ?: item.threshold.toString()),
        )
        else -> t("status.progress", "approved" to item.approved.toString(), "threshold" to item.threshold.toString())
    }

    /**
     * r9-13 (the web's row, r4b-6): where it stands in the core's one word,
     * and — while others wait at its number (`sameNonce`: a cancellation, a
     * rival) — "Only one of these can run" beside it.
     */
    fun standingLine(standing: JsonElement?, item: MsProposalItem): String =
        listOfNotNull(this.standing(standing, item), t("status.conflict").takeIf { item.sameNonce.isNotEmpty() && item.status != "done" })
            .joinToString(" · ")

    /** Why a ready transaction cannot run now (`MultisigReadyBlock`). */
    fun blockReason(block: JsonElement?): String {
        if (block == null) return ""
        if (block.kind == "nobodyPays") return t("status.nobodyPays")
        return whyFails(block["why"])
    }

    /** `short` (token, have, need) or `reverts` (reason). */
    fun whyFails(why: JsonElement?): String {
        if (why == null) return t("proposal.uncheckedReason")
        return when (why.kind) {
            "short" -> {
                val decimals = why["token"]["decimals"].int ?: 18
                val symbol = why["token"]["symbol"].str.orEmpty()
                fun amount(key: String) = "${MsRules.formatUnits(why[key].str ?: "0", decimals)} $symbol".trim()
                t("proposal.whyShort", "have" to amount("have"), "need" to amount("need"))
            }
            "reverts" -> why["reason"].str?.takeIf { it.isNotEmpty() }?.let { t("proposal.whyReverts", "reason" to it) }
                ?: t("proposal.whyRevertsPlain")
            else -> t("proposal.uncheckedReason")
        }
    }

    // -- why an action did not go (`MultisigActionState`) ----------------------------

    /** The action's failure in words; `null` for one that says nothing (working, done, the person's own cancel). */
    fun actionFailure(state: JsonElement?, chainId: Int): String? {
        state ?: return null
        return when (state.kind) {
            "working", "done", "cancelled", "outsideAsked", "riskDeclined" -> null
            "mailboxUnreadable" -> t("proposal.mailboxDown")
            "noMemberHere" -> t("proposal.notMember")
            "signingRefused" -> t("authority.refusedOn", "network" to network(chainId))
            "signingBlocked" -> t("review.blocked")
            "signingUnchecked" -> t("authority.signUnchecked", "network" to network(chainId))
            "refused" -> t("proposal.refused", "reason" to (state["reason"].str ?: ""))
            "mailboxRefused" -> t("proposal.reason.serviceRefused")
            "stopped" -> t("proposal.wouldFail", "reason" to whyFails(state["preview"]["why"] ?: state["preview"]))
            "noOutsideSigner" -> t("proposal.failed", "reason" to t("proposal.noOutsideSigner"))
            "failed" -> failureText(state["detail"].str)
            else -> t("proposal.reason.unknown")
        }
    }

    /** A shell failure ([MultisigShellFailure]'s JSON), or the core's own words, said as the corpus says them. */
    fun failureText(detail: String?): String {
        val failure = MultisigShellFailure.read(detail)
            ?: return if (detail.isNullOrEmpty()) t("proposal.reason.unknown") else t("proposal.failed", "reason" to detail)
        return when (failure.kind) {
            "passkey" -> when {
                failure["timedOut"].bool == true -> t("proposal.reason.passkeyTimedOut")
                failure["failure"].str == "cancelled" -> t("proposal.reason.passkeyClosed")
                failure["failure"].str in setOf("not_discoverable", "not_supported") -> t("proposal.reason.passkeyMissing")
                else -> t("proposal.reason.passkeyFailed")
            }
            "passkey_check" -> t("proposal.reason.passkeyFailed")
            "venue_blocked" -> failure["block"]?.let { block ->
                VenueWords.block(org.json.JSONObject(block.toString())) { key, vars -> strings.t(key, vars) }.ifEmpty { null }
            } ?: t("proposal.reason.unknown")
            "page_declined" -> raw("componentsUi.signing.trustedSignerClosed")
            "page_sign_failed", "page_approval" -> when (failure["page"].str ?: failure["failure"].str) {
                "unanswered" -> raw("componentsUi.signing.trustedSignerTimeout")
                "mismatch" -> raw("componentsUi.signing.trustedSignerMismatch")
                "not_opened" -> raw("componentsUi.signing.signerDown")
                else -> raw("componentsUi.signing.trustedSignerRefused")
            }
            "not_sent" -> t("proposal.notSent")
            "cannot_pay" -> t("proposal.failed", "reason" to t("status.nobodyPays"))
            else -> failure["message"].str?.takeIf { it.isNotEmpty() }?.let { t("proposal.failed", "reason" to it) }
                ?: t("proposal.reason.unknown")
        }
    }

    // -- authority findings (§3.5) -----------------------------------------------------

    /** One finding (`{from: 'genesis', chainId, reason}` or `{from: 'chain', chainId, risk}`), in words. */
    fun finding(finding: JsonElement): String {
        val chain = finding["chainId"].int?.let(::network) ?: ""
        val risk = finding["risk"]
        if (finding["from"].str == "chain" && risk != null) {
            val address = risk["address"].str.orEmpty()
            return when (risk.kind) {
                "module" -> t("authority.riskModule", "network" to chain, "address" to address)
                "guard" -> t("authority.riskGuard", "network" to chain, "address" to address)
                "handler" -> t("authority.riskHandler", "network" to chain, "address" to address)
                else -> t("authority.riskSingleton", "network" to chain, "address" to address)
            }
        }
        val reason = finding["reason"] ?: finding
        val address = reason["address"].str ?: reason["receiver"].str ?: ""
        return when (reason.kind) {
            "module" -> t("authority.module", "address" to address)
            "handler" -> t("authority.handler", "address" to address)
            "missingCode" -> t("authority.missingCode", "network" to chain, "address" to address)
            "wrongCode" -> t("authority.wrongCode", "network" to chain, "address" to address)
            "unknownFactory" -> t("authority.unknownFactory", "address" to address)
            "unknownSingleton" -> t("authority.unknownSingleton", "address" to address)
            "setupCode" -> t("authority.setupCode", "address" to address)
            "payment" -> t("authority.payment", "address" to address)
            "wrongAddress" -> t("authority.wrongAddress")
            "notSetup" -> t("authority.notSetup")
            "badMembers" -> t("authority.badMembers")
            else -> t("authority.unverified", "network" to chain)
        }
    }

    companion object {
        /** "0xd9c79C…a20432": short, in its EIP-55 case. */
        fun short(address: String): String {
            val checksummed = MsRules.eip55(address)
            return if (checksummed.length > 14) "${checksummed.take(8)}…${checksummed.takeLast(6)}" else checksummed
        }

        /** Keep [Wire] reachable for callers that decode a view part here. */
        internal val json get() = Wire.json
    }
}
