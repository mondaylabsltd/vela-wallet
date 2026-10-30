package app.getvela.wallet.feature.signing

import androidx.compose.runtime.Immutable
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.core.i18n.VelaStrings
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import uniffi.vela_core_uniffi.intentAdvice
import uniffi.vela_core_uniffi.intentAdviceStanding
import uniffi.vela_core_uniffi.intentReview

/**
 * The person's AI agent's opinion on the signing request on screen (spec
 * 103, `vela_advise`) — the web's `agent/advisor.ts` + `advice.svelte.ts`.
 *
 * Whether an opinion is accepted, and what of it is kept (plain text,
 * capped, no bidi or zero-width character, source hosts only), is the
 * core's (`intent_advice`, `agent_review::advice`). How it stands against
 * the sheet now — the review it was given for, the fee it was given at — is
 * the core's too (`intent_advice_standing`). What the sheet draws of it is
 * here: its own card, never one of Vela's words, greyed once the review it
 * was given for is not the one on screen or the fee moved past its band.
 */
object SigningAdvice {

    /** A verdict the agent may give. */
    @Serializable
    enum class Verdict {
        @SerialName("looks_fine") LooksFine,
        @SerialName("caution") Caution,
        @SerialName("danger") Danger,
        @SerialName("unsure") Unsure,
    }

    /** An opinion as the core accepted it (`agent_review::Advice`). */
    @Serializable
    data class Accepted(
        val review_digest: String,
        val verdict: Verdict = Verdict.Unsure,
        val summary: String = "",
        val points: List<String> = emptyList(),
        val source_hosts: List<String> = emptyList(),
        /** The fee the opinion was given for, as the core recorded it; `null`: the sheet showed none. */
        val fee: JsonElement? = null,
    )

    /** How an opinion stands against the sheet now (`agent_review::standing_of`). */
    @Serializable
    enum class Standing {
        @SerialName("current") Current,
        @SerialName("earlier_version") EarlierVersion,
        @SerialName("fee_changed") FeeChanged,
    }

    /** The fee the opinion was given for, as the card says it. */
    @Serializable
    data class FeeWhenGiven(val amount: String? = null, val symbol: String? = null)

    /** What `intent_advice_standing` answers. */
    @Serializable
    data class StandingAnswer(
        val standing: Standing = Standing.EarlierVersion,
        val fee_when_given: FeeWhenGiven? = null,
    )

    /** What `intent_advice` answers: the accepted opinion, or why not. */
    sealed interface Answer {
        data class Ok(val advice: Accepted) : Answer
        data class Refused(val code: String, val detail: String) : Answer
    }

    /**
     * `vela_advise`'s arguments ([argsJson]), judged by the core against
     * [worldJson] (who may use the wallet in front) and the review on screen
     * ([currentDigest], its [currentFeeJson]): the opinion as the core kept
     * it, or its refusal.
     */
    fun accept(worldJson: String, currentDigest: String?, currentFeeJson: String?, argsJson: String): Answer {
        val input = buildJsonObject {
            put("world", Wire.json.parseToJsonElement(worldJson))
            put("current_digest", currentDigest?.let { kotlinx.serialization.json.JsonPrimitive(it) } ?: JsonNull)
            put("current_fee", currentFeeJson?.let(Wire.json::parseToJsonElement) ?: JsonNull)
            put("args", Wire.json.parseToJsonElement(argsJson))
        }
        val answer = runCatching { Wire.json.parseToJsonElement(intentAdvice(input.toString())) as JsonObject }
            .getOrElse { return Answer.Refused("internal", it.message.orEmpty()) }
        val ok = (answer["ok"] as? kotlinx.serialization.json.JsonPrimitive)?.content == "true"
        if (!ok) {
            fun text(key: String) = (answer[key] as? kotlinx.serialization.json.JsonPrimitive)?.content.orEmpty()
            return Answer.Refused(text("code"), text("detail"))
        }
        val advice = answer["advice"]?.let { runCatching { Wire.json.decodeFromJsonElement(Accepted.serializer(), it) }.getOrNull() }
            ?: return Answer.Refused("not_found", "no signing review is open")
        return Answer.Ok(advice)
    }

    /**
     * How [advice] stands against the review on the sheet now — its digest
     * ([currentDigest], `null`: none open) and the fee on screen
     * ([currentFeeJson]) — the core's call. A standing that does not read is
     * "given for an earlier version": never drawn as current on a guess.
     */
    fun standing(advice: Accepted, currentDigest: String?, currentFeeJson: String?): StandingAnswer {
        val input = buildJsonObject {
            put("advice", Wire.json.encodeToJsonElement(Accepted.serializer(), advice))
            put("current_digest", currentDigest?.let { kotlinx.serialization.json.JsonPrimitive(it) } ?: JsonNull)
            put("current_fee", currentFeeJson?.let(Wire.json::parseToJsonElement) ?: JsonNull)
        }
        return runCatching { Wire.json.decodeFromString(StandingAnswer.serializer(), intentAdviceStanding(input.toString())) }
            .onFailure { VelaLog.failure("signing.advice", "the standing could not be read", it) }
            .getOrDefault(StandingAnswer())
    }

    /**
     * The dApp signing review on the sheet now, as the agent reads it
     * (`vela_review`: `intent_rules::review` over the sign view, as the
     * native agent builds it) — JSON, `null` when no request is open.
     */
    fun currentReview(signingJson: String?, account: String?, networksJson: String? = null): String? {
        signingJson ?: return null
        val input = runCatching {
            buildJsonObject {
                put("signing", Wire.json.parseToJsonElement(signingJson))
                put("account", account?.takeIf { it.isNotBlank() }?.let { kotlinx.serialization.json.JsonPrimitive(it) } ?: JsonNull)
                put("networks", networksJson?.let(Wire.json::parseToJsonElement) ?: JsonNull)
            }
        }.getOrNull() ?: return null
        return runCatching { intentReview(input.toString()) }.getOrNull()
    }

    /** A review's `signing.review_digest` — `null` when it names no signing request. */
    fun digestOf(reviewJson: String?): String? {
        val review = reviewJson?.let { runCatching { Wire.json.parseToJsonElement(it) as JsonObject }.getOrNull() } ?: return null
        val signing = review["signing"] as? JsonObject ?: return null
        return (signing["review_digest"] as? kotlinx.serialization.json.JsonPrimitive)?.content
    }

    /** The fee a review shows (its `signing.fee`), as JSON — what an opinion is bound to. */
    fun reviewFee(reviewJson: String?): String? {
        val review = reviewJson?.let { runCatching { Wire.json.parseToJsonElement(it) as JsonObject }.getOrNull() } ?: return null
        val fee = (review["signing"] as? JsonObject)?.get("fee") ?: return null
        return fee.takeIf { it !is JsonNull }?.toString()
    }

    /**
     * The card for [advice] as it stands ([standing]) — the web's
     * `adviceCard`, in the corpus's words (`componentsUi.signing.agentAdvice`).
     * An opinion for another version of the request, or another fee, is never
     * drawn as current.
     */
    fun card(advice: Accepted, standing: StandingAnswer?, s: VelaStrings): AdviceModel {
        fun w(key: String) = s.t("componentsUi.signing.agentAdvice.$key")
        val kind = standing?.standing ?: Standing.EarlierVersion
        val given = standing?.fee_when_given
        val fee = given?.amount?.takeIf { it.isNotBlank() }?.let { listOfNotNull(it, given.symbol?.takeIf(String::isNotBlank)).joinToString(" ") }
        return AdviceModel(
            title = w("title"),
            verdict = advice.verdict,
            verdictWord = when (advice.verdict) {
                Verdict.LooksFine -> w("looksFine")
                Verdict.Caution -> w("caution")
                Verdict.Danger -> w("danger")
                Verdict.Unsure -> w("unsure")
            },
            summary = advice.summary,
            points = advice.points,
            hosts = advice.source_hosts,
            sourcesLabel = w("sources"),
            footer = w("footer"),
            stale = kind != Standing.Current,
            staleLine = if (kind == Standing.FeeChanged) w("feeChanged") else w("stale"),
            feeLine = fee?.let { s.t("componentsUi.signing.agentAdvice.feeWhenGiven", mapOf("fee" to it)) },
        )
    }
}

/**
 * The AI agent's card on the signing sheet (the web's `AgentAdviceCardModel`):
 * its own card under Vela's lines and over the fee, labelled as the agent's.
 * It colours nothing but its chip — the confirm and every line above stay as
 * Vela drew them, and nothing waits for it.
 */
@Immutable
data class AdviceModel(
    val title: String,
    val verdict: SigningAdvice.Verdict,
    val verdictWord: String,
    val summary: String,
    val points: List<String>,
    /** Hosts only, as plain text — nothing on the card is a link. */
    val hosts: List<String>,
    val sourcesLabel: String,
    val footer: String,
    /** Given for an earlier version of this request, or at a fee since changed: greyed, [staleLine] says which. */
    val stale: Boolean,
    val staleLine: String,
    /** "Fee when given: 0.0012 xDAI" — `null` when the opinion saw no fee. */
    val feeLine: String?,
)
