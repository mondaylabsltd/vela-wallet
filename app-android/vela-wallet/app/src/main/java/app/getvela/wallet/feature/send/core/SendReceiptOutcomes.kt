package app.getvela.wallet.feature.send.core

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.diagnostics.VelaLog
import uniffi.vela_core_uniffi.sendReceiptOutcomeOf

/**
 * What a tracker entry means for the send receipt on screen — the core's one
 * mapping (`send::receipt_outcome_of`, spec 082), where this app used to keep
 * its own `when` over four statuses.
 *
 * Only a definitive drop, rejection or never-sent is a failure; a slow,
 * unreachable, may-have-been-sent or day-old op says nothing (invariant ⑤);
 * a relay that acknowledges an op whose reply was lost turns the receipt
 * back to "submitted" ([SendReceiptOutcome.Acknowledged]).
 */
object SendReceiptOutcomes {
    fun of(entry: TrackEntryView): SendReceiptOutcome? {
        val entryJson = Wire.json.encodeToString(TrackEntryView.serializer(), entry)
        val outcome = runCatching { sendReceiptOutcomeOf(entryJson) }
            .onFailure { VelaLog.failure("send.receipt", "the core could not read a tracker entry", it) }
            .getOrNull() ?: return null
        return runCatching { Wire.json.decodeFromString(SendReceiptOutcome.serializer(), outcome) }
            .onFailure { VelaLog.failure("send.receipt", "an outcome this app cannot decode", it) }
            .getOrNull()
    }
}
