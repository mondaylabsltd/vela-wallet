package app.getvela.wallet.feature.send

import androidx.compose.runtime.Composable
import app.getvela.wallet.core.diagnostics.BugReport
import app.getvela.wallet.feature.send.core.SendRelayReport
import app.getvela.wallet.feature.settings.FeedbackSeed
import app.getvela.wallet.feature.settings.SettingsOverlay
import app.getvela.wallet.feature.settings.SettingsScreenModel
import app.getvela.wallet.feature.settings.SettingsSheet

/**
 * Issue #466: a relay stop's "Report this" — the app's OWN report sheet (the
 * Settings feedback sheet, ST15), over the send flow, opened with the core's
 * report already in its boxes.
 *
 * One sheet: the settings sheet host with the feedback body, as the home's
 * rescue sheet is (spec 092) — never a second modal on another. The person
 * reads what will be filed (the treasury address, what it holds against its
 * floor, where they were), may edit it, and sends; the core's area and
 * fingerprint go with it ([app.getvela.wallet.core.diagnostics.BugReport.build]),
 * so every reporter of one outage lands on one issue. Every refusal or fault
 * still ends at the prefilled GitHub form, as any report does.
 *
 * [seed] is the core's `relay_report` as it stood at the tap: the stop closes
 * itself once the relayer is funded, and a report half-read must not vanish
 * under the person's finger when it does.
 */
@Composable
fun RelayReportSheet(
    model: SettingsScreenModel,
    seed: FeedbackSeed,
    onDismiss: () -> Unit,
    onSend: (what: String, steps: String, screenshots: List<ByteArray>) -> Unit,
    onGithub: () -> Unit,
    onOpenLink: (String) -> Unit,
    onOpened: () -> Unit,
    onClosed: () -> Unit,
) {
    SettingsSheet(
        model = model,
        overlay = SettingsOverlay.Feedback,
        onDismiss = onDismiss,
        onSignOut = {},
        onFeedbackSend = onSend,
        onFeedbackGithub = onGithub,
        onFeedbackOpened = onOpened,
        onFeedbackClosed = onClosed,
        feedbackSeed = seed,
        onOpenLink = onOpenLink,
    )
}

/**
 * Issue #466: the report a relay stop files, as the core built it. The words
 * are whatever the person left in the boxes — the core's, edited or not —
 * and the area and fingerprint are always the core's: `Send`, and
 * `relay-gas-<chain>` / `relay-unreachable-<chain>`, the same on every
 * platform and version, so one outage is one issue.
 */
object RelayReport {
    /** What the sheet opens with: the core's report at the tap. */
    fun seed(report: SendRelayReport): FeedbackSeed = FeedbackSeed(what = report.what, steps = report.steps)

    fun payload(
        report: SendRelayReport,
        what: String,
        steps: String,
        labels: BugReport.EnvironmentLabels,
        facts: BugReport.DeviceFacts,
        screenshots: List<ByteArray> = emptyList(),
    ): BugReport.Payload = BugReport.build(
        what = what,
        steps = steps,
        area = report.area,
        labels = labels,
        facts = facts,
        screenshots = screenshots,
        fingerprint = report.fingerprint,
    )
}
