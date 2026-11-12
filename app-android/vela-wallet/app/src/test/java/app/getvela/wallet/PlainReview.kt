package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.feature.signing.core.ClearSigningView
import app.getvela.wallet.feature.signing.core.ClearSurface
import app.getvela.wallet.feature.signing.core.GuardView
import app.getvela.wallet.feature.signing.core.SignApproveOpts
import app.getvela.wallet.feature.signing.core.SignApproveReview

/**
 * §41 (P2-5): the review the signing machine admits on a plain press — the
 * core's own test `plain_review`: a request read in full (no blind surface,
 * nothing resolving), no approval-guard concern, and a check two independent
 * nodes agreed on. A test about the posture builds its own ([signAnyway]).
 */
internal fun plainReview(signAnyway: Boolean = false): SignApproveReview = SignApproveReview(
    clear = Wire.json.encodeToJsonElement(
        ClearSigningView.serializer(),
        ClearSigningView(surface = ClearSurface.ClearSign, resolving = false, sign_anyway = signAnyway),
    ),
    guard = Wire.json.encodeToJsonElement(GuardView.serializer(), GuardView()),
    sim = Wire.json.parseToJsonElement("""{"kind":"deltas","deltas":[],"source":"corroborated"}"""),
)

/** Approve options carrying [plainReview] — the core's `reviewed_opts`. */
internal fun reviewedOpts(): SignApproveOpts = SignApproveOpts(review = plainReview())
