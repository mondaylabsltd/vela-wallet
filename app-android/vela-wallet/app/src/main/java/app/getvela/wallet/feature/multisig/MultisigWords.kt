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

    /**
     * A list inside a translated sentence (members, networks, numbers), in
     * the UI language's own way (`ListFormatter`: "A, B and C", "A、B 和 C") —
     * never an English ", " (r10, the core's rule). A JVM without ICU (a unit
     * test) joins with ", ".
     */
    fun list(items: List<String>): String {
        if (items.size < 2) return items.firstOrNull().orEmpty()
        return runCatching {
            android.icu.text.ListFormatter.getInstance(java.util.Locale.forLanguageTag(strings.language)).format(items)
        }.getOrNull()?.takeIf { it.isNotEmpty() } ?: items.joinToString(", ")
    }

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
        val name = label(address)
        // r11-6: the core's one rule for a title naming a member (`names.who`).
        return MsRules.string("names.who", mapOf("address" to address, "label" to name))
            ?: short(address).let { short -> if (name == short) short else "$name ($short)" }
    }

    /**
     * A member's label — the core's rule (`names.label`, r8-25: the name the
     * multisigs keep, else the passkey index's, else this device's own
     * account name, else the short address), so a member reads the same on
     * every device. This shell only hands it what it knows.
     */
    fun label(address: String): String =
        // r9-12 (core §8): the accounts machine's one name per member — the
        // passkey index's name included — before anything asked here.
        accounts.memberLabels[address.lowercase()]?.takeIf { it.isNotBlank() }
            ?: MsRules.string("names.label", mapOf("address" to address, "own" to localName(address), "kept" to keptName(address), "registered" to null))
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

    /**
     * r13-6: a network this device lacks, NAMED — for a sentence that says
     * the rest itself (`add.networkMissing`'s `{{network}}`): the name given,
     * else the core's catalog's, else the device's index, else "Chain N" in
     * the page's words (`assets.chainFallback`).
     */
    fun missingName(chainId: Int, name: String? = null): String =
        name?.takeIf { it.isNotBlank() } ?: MsRules.networkName(chainId) ?: networkName(chainId)
            ?: raw("assets.chainFallback", "chainId" to "$chainId")

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
    fun rowRuleChip(row: app.getvela.wallet.feature.multisig.core.MsAccountRow, proposals: app.getvela.wallet.feature.multisig.core.MsProposalsView): String =
        (if (proposals.followed.equals(row.account.address, ignoreCase = true)) ruleChip(proposals.ruleChip) else null)
            // r9-14 (core §9.2): one not followed draws the accounts machine's
            // `rule.chip` (the chains' last word), never the founding record's.
            ?: ruleChip(row.rule["chip"])
            ?: ruleChip(row.account.threshold, row.account.owners.size)

    /**
     * r9-14 (core §9.2): "Last known — read again when you open it", beside a
     * chip of a multisig not followed whose rule a chain said in an earlier
     * session; `null` otherwise.
     */
    fun ruleLastKnown(row: app.getvela.wallet.feature.multisig.core.MsAccountRow, proposals: app.getvela.wallet.feature.multisig.core.MsProposalsView): String? {
        if (proposals.followed.equals(row.account.address, ignoreCase = true)) return null
        val rule = row.rule ?: return null
        return if (rule["from_chain"].bool == true && rule["last_known"].bool == true) t("presence.ruleLastKnown") else null
    }

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
    fun actionTitle(chainId: Int, action: JsonElement?, nonce: String, token: Pair<String, Int?>? = null): String {
        if (action.isNullish) return t("action.unsupported")
        // r12-8 (core §13): a member change is titled by the core's ONE rule
        // (`names.change`) — the sheet and Activity alike — never worded here.
        memberChangeTitle(action)?.let { return it }
        return when (action.kind) {
            "transfer" -> {
                val raw = action["amount"].str ?: "0"
                val address = action["token"].str
                if (address == null) return t("action.send", "amount" to MsRules.formatUnits(raw, MsRules.COIN_DECIMALS), "symbol" to symbol(chainId)).trim()
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
            // r9 words: "Require 1 approval" — the core's plural family (`_one`, `_other`, …).
            "changeThreshold" -> action["threshold"].int?.let { plural("action.changeThreshold", it) }
                ?: t("action.changeThreshold", "threshold" to (action["threshold"].str ?: ""))
            "rename" -> t("action.rename", "name" to (action["name"].str ?: ""))
            "call" -> {
                val value = action["value"].str ?: "0"
                if (value == "0" || value.isEmpty()) t("action.call")
                else t("action.callWithValue", "amount" to MsRules.formatUnits(value, MsRules.COIN_DECIMALS), "symbol" to symbol(chainId))
            }
            else -> t("action.unsupported")
        }
    }

    /**
     * r12-8: a member change's title as the core words it (`names.change
     * {action, labels}` → `{key, params, count?}`), each member named by
     * `names.who` with the accounts machine's labels; `null` for anything
     * else, or from a core without the rule.
     */
    internal fun memberChangeTitle(action: JsonElement?): String? {
        if (action.kind !in setOf("addOwner", "removeOwner", "swapOwner", "changeThreshold")) return null
        val labels = localNames + accounts.memberLabels
        val said = MsRules.call("names.change", mapOf("action" to action, "labels" to labels)) ?: return null
        val key = said["key"].str ?: return null
        val params = (said["params"] as? kotlinx.serialization.json.JsonObject)?.mapNotNull { (k, v) -> v.str?.let { k to it } }.orEmpty()
        val count = said["count"].str?.toIntOrNull() ?: said["count"].int
        return if (count != null) strings.t(key, count) else raw(key, *params.toTypedArray())
    }

    // -- a row's one word (`standings`) ----------------------------------------------

    fun standing(standing: JsonElement?, item: MsProposalItem): String = when (standing.kind) {
        "unsent" -> t("status.unsent")
        "executing" -> t("status.executing")
        "queued" -> t("status.queued")
        "ready" -> t("status.ready")
        "yourTurn" -> t("status.yourTurn")
        "blocked" -> t("status.cannotRun", "reason" to blockReason(standing["block"], item.chainId))
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
    fun blockReason(block: JsonElement?, chainId: Int): String {
        if (block == null) return ""
        if (block.kind == "nobodyPays") return t("status.nobodyPays")
        return whyFails(block["why"], chainId)
    }

    /**
     * `short` (token, have, need) or `reverts` (reason). r12-1: a shortfall's
     * amounts are worded in the token's OWN decimals — the core's facts on the
     * shortfall (`token` as `OutsideTokenFacts`), else the facts the core read
     * for the review (`tokens`), else, for the network's coin, its wei — and
     * never with 18 assumed for a token: one whose decimals nobody said is
     * said in base units, "decimals unknown" (0.01 USDC read "<0.000001").
     */
    fun whyFails(why: JsonElement?, chainId: Int): String {
        if (why == null) return t("proposal.uncheckedReason")
        return when (why.kind) {
            "short" -> {
                val (symbol, decimals) = shortfallUnit(why["token"], chainId)
                fun amount(key: String): String {
                    // r12-1 (core §13): the core's own amount (`MultisigAmount`
                    // `{base_units, token, said}`) — drawn as it says it: the
                    // figure with its unit, or the base units, "decimals unknown".
                    said(why[key]["said"])?.let { return it }
                    val raw = why[key].str ?: "0"
                    val figure = if (decimals == null) t("review.reading.decimalsUnknown", "raw" to raw) else MsRules.formatUnits(raw, decimals)
                    return "$figure $symbol".trim()
                }
                t("proposal.whyShort", "have" to amount("have"), "need" to amount("need"))
            }
            "reverts" -> why["reason"].str?.takeIf { it.isNotEmpty() }?.let { t("proposal.whyReverts", "reason" to it) }
                ?: t("proposal.whyRevertsPlain")
            else -> t("proposal.uncheckedReason")
        }
    }

    /**
     * r12-1 (core §13): an amount as the core said it (`MultisigAmountSaid`):
     * `known` → its `text` ("0.01 USDC"); `decimals_unknown` → its key with the
     * base units. `null` for anything else (a core before it).
     */
    internal fun said(said: JsonElement?): String? = when (said.kind) {
        "known" -> said["text"].str?.takeIf { it.isNotBlank() }
        "decimals_unknown" -> said["raw"].str?.let { raw -> raw(said["key"].str ?: "home.multisig.review.reading.decimalsUnknown", "raw" to raw) }
        else -> null
    }

    /**
     * r12-1: the symbol and decimals a shortfall is said in. [token] is the
     * core's facts (an object: token, symbol, decimals), or — from a core
     * before them — the token's address (`null`: the network's coin).
     * `null` decimals: nobody said them.
     */
    internal fun shortfallUnit(token: JsonElement?, chainId: Int): Pair<String, Int?> {
        val given = token as? kotlinx.serialization.json.JsonObject
        val address = if (given != null) given["token"].str else token.str
        if (address.isNullOrBlank()) {
            val symbol = given?.get("symbol").str?.takeIf { it.isNotBlank() } ?: symbol(chainId)
            return symbol to (given?.get("decimals").int ?: MsRules.COIN_DECIMALS)
        }
        val facts = given ?: tokenFacts(chainId, address)
        val symbol = facts?.get("symbol").str?.takeIf { it.isNotBlank() } ?: short(address)
        return symbol to facts?.get("decimals").int
    }

    // -- why an action did not go (`MultisigActionState`) ----------------------------

    /**
     * r13-3: a key the core named for a read that failed — the full catalog
     * key, `{{network}}` its name: `home.multisig.proposal.unread.*`, and
     * (round 16 leftover 3) any other the core names, such as the service's
     * busy words (`home.multisig.service.busyNote`). Worded whole, never
     * prefixed again.
     */
    fun unreadText(key: String?, chainId: Int): String =
        raw(key ?: "home.multisig.proposal.unread.chain", "network" to network(chainId))

    /** The action's failure in words; `null` for one that says nothing (working, done, the person's own cancel). */
    fun actionFailure(state: JsonElement?, chainId: Int): String? {
        state ?: return null
        return when (state.kind) {
            "working", "done", "cancelled", "outsideAsked", "riskDeclined" -> null
            "mailboxUnreadable" -> t("proposal.mailboxDown")
            // r13-3: the read the core names (the network, or both).
            "unread" -> unreadText(state["reason_key"].str, chainId)
            "noMemberHere" -> t("proposal.notMember")
            "signingRefused" -> t("authority.refusedOn", "network" to network(chainId))
            "signingBlocked" -> t("review.blocked")
            "signingUnchecked" -> t("authority.signUnchecked", "network" to network(chainId))
            "refused" -> refusedText(state["reason"].str)
            "mailboxRefused" -> t("proposal.reason.serviceRefused")
            // r11-9: what stopped it is its look's own kind — "would fail" only
            // for a look that says it fails, never for one that could not price
            // or check it ("This would fail…: it couldn't be checked").
            "stopped" -> stoppedLine(state["preview"], chainId)
            "noOutsideSigner" -> t("proposal.failed", "reason" to t("proposal.noOutsideSigner"))
            "failed" -> failureText(state["detail"].str, chainId)
            else -> t("proposal.reason.unknown")
        }
    }

    /**
     * §31: a site's request to a multisig the core refused because it would
     * have made the multisig act for a Safe it owns (`refusal.data.reason ==
     * "parent_safe"`) — said in the core's sentence for it (`data.key`:
     * `componentsUi.signing.parentSafeBlocked` / `parentSafeUnchecked`) with
     * the function and that Safe. `null` for any other refusal.
     */
    fun dappRefusal(refusal: JsonElement?): String? {
        val data = refusal["data"]
        if (data["reason"].str != "parent_safe") return null
        val key = data["key"].str?.takeIf { it == PARENT_BLOCKED_KEY || it == PARENT_UNCHECKED_KEY }
            ?: if (data["why"].str == "unchecked") PARENT_UNCHECKED_KEY else PARENT_BLOCKED_KEY
        return raw(key, "function" to data["function"].str.orEmpty(), "safe" to data["safe"].str?.let(::short).orEmpty())
    }

    /**
     * Round 16 leftover 4: a draft the core refused to build (`refused`'s
     * `reason`). A member change's refusal is the core's JSON
     * (`MemberRefusal`, `{kind, …}`) and is worded by its kind; anything
     * else is the core's English — never drawn: "Vela won't sign this
     * transaction: it isn't a kind Vela signs" (iOS's words).
     */
    fun refusedText(reason: String?): String {
        val refusal = MultisigShellFailure.read(reason)
        return if (refusal.kind in MEMBER_REFUSALS) memberRefusal(this, refusal) else t("review.refusedWhy.other")
    }

    /**
     * Round 16 leftover 4: the create sheet's draft the core could not make
     * (`create.draft_error` is the core's English, never drawn).
     */
    val createDraftFailed: String get() = t("proposal.reason.unknown")

    /**
     * The created multisig the service did not record (`create.refused` is
     * the service's text, never drawn): it is kept here, and Settings offers
     * "Share it again" — the catalog's words for exactly that.
     */
    val createRefused: String get() = t("settings.shareAgainNote")

    /**
     * r18-8 (§28, §32): this test build refused to write to the live service —
     * the catalog's sentence (`common.testBuild.productionWriteRefused`), never
     * "can't be reached", "didn't answer" or "offline".
     */
    val productionWriteRefused: String get() = raw(uniffi.vela_core_uniffi.productionWriteRefusedKey())

    /**
     * The created multisig's `create.refused`, in words: the test build's refusal
     * ([PRODUCTION_WRITE_REFUSED]) its own sentence; any other — the service
     * answered and did not take it — "Share it again".
     */
    fun createRefused(refused: String?): String =
        if (refused == PRODUCTION_WRITE_REFUSED) productionWriteRefused else createRefused

    /**
     * Round 16 leftover 4: why Vela's side is not on the signing relay
     * (`RelayStatus.why`), as the reason its line fills in — the relay's
     * refusal and a pass not got in the catalog's words, the relay unreached
     * by its HOST (the core's `detail`, only when it is one) — never a
     * transport error's text. `null`: nothing sayable as a reason (the line
     * says Vela is reconnecting instead, [relayLine]).
     */
    fun relayReason(why: String?, detail: String?): String? = when (why) {
        "refused" -> t("external.relayRefused")
        "unreachable" -> detail?.takeIf { it.isNotEmpty() && RELAY_HOST.matches(it) }
        else -> t("external.relayNoToken")
    }

    /** The outside sheet's relay line (`retrying` / `failed`), in words; `null` for any other stand. */
    fun relayLine(relay: JsonElement?): String? {
        val kind = relay.kind
        if (kind != "retrying" && kind != "failed") return null
        val reason = relayReason(relay["why"].str, relay["detail"].str) ?: return t("external.velaOffline")
        return if (kind == "failed") t("external.relayFailed", "reason" to reason) else t("external.relayUnreachable", "reason" to reason)
    }

    /**
     * The outside signing session's failure, in words. A room that could not
     * be opened (`noRoom`) carries a transport error's text as its `reason`
     * — round 16 leftover 4: never drawn, "Something went wrong" with the
     * sheet's Ask again (iOS's `outsideFailure`).
     */
    fun outsideFailure(failure: JsonElement, chain: String, wallet: String): String = when (failure.kind) {
        "connectDeclined" -> t("external.connectDeclined")
        "walletDeclined" -> t("external.declined")
        "otherChain" -> t("external.otherChain", "chain" to chain)
        "badSignature" -> t("external.badSignature")
        "stoppedWaiting" -> t("external.stoppedWaiting")
        "noRoom" -> t("proposal.reason.unknown")
        else -> t("external.stillWaiting", "wallet" to wallet)
    }

    /**
     * r17-17: a link whose lookup failed (`MultisigLinkStand::Failed`'s
     * `reason`, an `AddOutcome`), each kind mapped to a sentence the catalog
     * has — never `add.<kind>` built from the kind (`add.refused`,
     * `add.highRisk`, `add.unverified` are no keys: their key was drawn).
     * iOS's mapping.
     */
    fun linkFailed(reason: JsonElement?): String = when (reason.kind) {
        null, "notFound" -> t("proposal.notFound")
        "busy" -> t("add.busy", "network" to network(reason["chain_id"].int ?: 0))
        "unverified" -> t("authority.unverified", "network" to network(reason["chain_id"].int ?: 0))
        "refused" -> t("authority.refusedTitle")
        "highRisk" -> t("authority.riskTitle")
        "unavailable", "unsupported", "notMultisig", "unchecked", "chooseNetwork" -> t("add.${reason.kind}")
        else -> t("add.unavailable")
    }

    /**
     * r17-17: a damaged link (`MultisigLinkStand::Broken`) in words — an
     * invite by what is wrong with it (its checksum, else damaged), a
     * transaction's link as damaged. The link's own text is never drawn.
     */
    fun linkBroken(stand: app.getvela.wallet.feature.multisig.core.MsLinkStand): String = when {
        stand.link.str == "add" && stand.fault == "checksum" -> t("add.badChecksum")
        stand.link.str == "add" -> t("add.badInvite")
        else -> t("proposal.badLink")
    }

    /** r11-9: an execution its look stopped, in the look's own words (`MultisigPreview` kind). */
    fun stoppedLine(preview: JsonElement?, chainId: Int): String = when (preview.kind) {
        "unpriced" -> t("proposal.feeUnpriced")
        "unchecked" -> t("proposal.feeUnchecked")
        // r14-4: a setup not checked is said as that, never as a finding.
        "unverified" -> t("authority.deployUnverified", "network" to network(chainId))
        "refused" -> deployRefused(preview["authority"], chainId)
        else -> t("proposal.wouldFail", "reason" to whyFails(preview["why"] ?: preview?.takeIf { it.kind == "short" || it.kind == "reverts" }, chainId))
    }

    /** r11-9: a look that only could not price or check it is a warning, never a failure. */
    fun actionFailureTone(state: JsonElement?): MsTone =
        if (state.kind == "stopped" && state["preview"].kind in setOf("unpriced", "unchecked", "unverified")) MsTone.Warning else MsTone.Danger

    /** The core's sentence for a passkey error's own words; `null` when it names nothing more than "it failed". */
    private fun passkeyWords(message: String?): String? {
        if (message.isNullOrBlank()) return null
        val words = runCatching { uniffi.vela_core_uniffi.passkeyErrorWords("android", "", null, message) }.getOrNull() ?: return null
        return if (words.kind == uniffi.vela_core_uniffi.PasskeyErrorKind.OTHER) null else raw(words.key)
    }

    /**
     * A failed action's `detail` in words — round 16 leftover 4: the core
     * reads it into a stable KIND (`review.failure`, [MsRules.failureSaid])
     * and names the catalog key that says it; this maps kinds, never a
     * detail's or a message's text. Plain text, a kind the core does not
     * know, the core's own refusal: "Something went wrong, so nothing was
     * sent." The kinds whose words are their own facts — the fee that moved,
     * what the executor cannot pay, the signing venue's block — are worded
     * from those facts. [chainId]: the action's network, for a
     * `chain_unreachable` that does not name its own.
     */
    fun failureText(detail: String?, chainId: Int? = null): String {
        val said = MsRules.failureSaid(detail)
        val failure = MultisigShellFailure.read(detail)
        val unknown = t("proposal.reason.unknown")
        return when (said.kind) {
            // Real device S3 (core §9.7): a platform's passkey error refined by
            // the core's catalog (`passkey_error_words`) — a key, never its English.
            "passkey_failed" -> passkeyWords(failure?.get("message").str) ?: said.key?.let { raw(it) } ?: unknown
            "venue_blocked" -> failure?.get("block")?.let { block ->
                VenueWords.block(org.json.JSONObject(block.toString())) { key, vars -> strings.t(key, vars) }.ifEmpty { null }
            } ?: unknown
            "cannot_pay" -> t("proposal.failed", "reason" to t("status.nobodyPays"))
            // r14-9: nothing signed — the fee moved since the press; said with the
            // new figure (its own fee line's words), so the next press is on it.
            "fee_moved" -> {
                val chain = failure?.get("chainId").int ?: chainId ?: 0
                val fee = "${MsRules.formatUnits(failure?.get("amount").str ?: "0", MsRules.COIN_DECIMALS)} ${symbol(chain)}"
                t("proposal.failed", "reason" to t("proposal.feeLine", "fee" to fee, "name" to label(failure?.get("executor").str.orEmpty())))
            }
            "chain_unreachable" -> {
                val chain = failure?.get("chain_id").int ?: failure?.get("chainId").int ?: chainId
                val key = said.key ?: return unknown
                if (chain == null) unknown else raw(key, "network" to network(chain))
            }
            else -> said.key?.let { raw(it) } ?: unknown
        }
    }

    // -- authority findings (§3.5) -----------------------------------------------------

    /**
     * r14-4: a deploy the core read and refused on [chainId] (the look's `refused`, its
     * `authority {kind: refused, reasons}`): "Vela won't deploy this multisig on X:" and each
     * reason once.
     */
    fun deployRefused(authority: JsonElement?, chainId: Int): String {
        val lines = mutableListOf(t("authority.deployRefused", "network" to network(chainId)))
        for (reason in authority["reasons"].arr) {
            val line = finding(
                kotlinx.serialization.json.JsonObject(
                    mapOf("chainId" to kotlinx.serialization.json.JsonPrimitive(chainId), "reason" to reason),
                ),
            )
            if (line !in lines) lines += line
        }
        return lines.joinToString(" ")
    }

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
        /** r18-8: the core's word for a write this test build refused (§28): `create.refused`, an outbox `why`. */
        const val PRODUCTION_WRITE_REFUSED = "production_write_refused"

        /** "0xd9c79C…a20432": short, in its EIP-55 case. */
        fun short(address: String): String =
            // r11-6: THE short form is the core's (`names.short`, 6+6 EIP-55).
            app.getvela.wallet.core.format.ShortAddress.of(address)

        /** §31: the core's two sentences for a parent-Safe refusal (`multisig_dapp::PARENT_*_KEY`). */
        private const val PARENT_BLOCKED_KEY = "componentsUi.signing.parentSafeBlocked"
        private const val PARENT_UNCHECKED_KEY = "componentsUi.signing.parentSafeUnchecked"

        /** The core's member-change refusals (`MemberRefusal`'s kinds) worded by [memberRefusal]. */
        private val MEMBER_REFUSALS = setOf(
            "noOne", "theMultisig", "alreadyMember", "notMember", "lastMember", "thresholdTooHigh",
            "thresholdZero", "belowThreshold", "unchanged", "tooMany", "changeWaiting",
        )

        /** A relay's host (and port) — the only `detail` of an unreached relay ever drawn. */
        private val RELAY_HOST = Regex("^[A-Za-z0-9.-]+(:[0-9]+)?$")

        /** Keep [Wire] reachable for callers that decode a view part here. */
        internal val json get() = Wire.json
    }
}
