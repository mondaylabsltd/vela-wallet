package app.getvela.wallet.feature.signing

import androidx.compose.runtime.Immutable
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.core.i18n.VelaStrings
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.buildJsonObject
import uniffi.vela_core_uniffi.intentAdviceStanding

/**
 * The person's AI agent's opinion on the signing request on screen (spec
 * 103, `vela_advise`) — the web's `agent/advisor.ts` + `advice.svelte.ts`.
 *
 * The phone agent is the core's (`vela-agent`): it registers `vela_advise`
 * and `vela_watch_reviews`, judges an opinion (plain text, capped, no bidi or
 * zero-width character, source hosts only — `agent_review::advice`), keeps
 * it with the request it was given on, and hands the card its input
 * (`AgentCore.advice`: the opinion, its standing against the sheet now — the
 * review it was given for, the fee it was given at — and that fee). What the
 * sheet draws of it is here: its own card, never one of Vela's words, greyed
 * once the review it was given for is not the one on screen or the fee
 * moved past its band.
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

    /**
     * What the phone agent hands the card (`AgentCore.advice`): the opinion
     * as the core accepted it, how it stands against the sheet now, and the
     * fee it was given for.
     */
    @Serializable
    data class Held(
        val advice: Accepted,
        val standing: Standing = Standing.EarlierVersion,
        val fee_when_given: FeeWhenGiven? = null,
    ) {
        val standingAnswer: StandingAnswer get() = StandingAnswer(standing, fee_when_given)
    }

    /**
     * The agent core's card input ([json], `AgentCore.advice`), read — `null`
     * for none (no request on the sheet, no opinion on this one) or one that
     * does not read: never a card drawn on a guess.
     */
    fun held(json: String?): Held? = json?.let {
        runCatching { Wire.json.decodeFromString(Held.serializer(), it) }
            .onFailure { error -> VelaLog.failure("signing.advice", "the agent's card could not be read", error) }
            .getOrNull()
    }

    /** The card the agent core's input ([json]) draws, as it stands now — `null` for none. */
    fun cardOf(json: String?, s: VelaStrings): AdviceModel? = held(json)?.let { card(it.advice, it.standingAnswer, s) }

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
