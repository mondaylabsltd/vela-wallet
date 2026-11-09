package app.getvela.wallet

import app.getvela.wallet.feature.signing.core.SigningController

/**
 * R22: a simulation outcome as the sheet DRAWS it — without the core's
 * verdict JSON it carries for the confirm gate (`SimOutcome.verdictJson`,
 * pinned on its own in `R22SigningTruthTest`). Two outcomes that draw the
 * same line compare equal here, whichever node answer they came from.
 */
fun SigningController.SimOutcome?.plain(): SigningController.SimOutcome? = when (this) {
    is SigningController.SimOutcome.Ready -> copy(verdict = null)
    is SigningController.SimOutcome.Notice -> copy(verdict = null)
    else -> this
}
