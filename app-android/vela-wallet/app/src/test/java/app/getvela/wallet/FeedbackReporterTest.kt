package app.getvela.wallet

import app.getvela.wallet.core.diagnostics.BugReport
import app.getvela.wallet.feature.settings.FeedbackModel
import app.getvela.wallet.feature.settings.FeedbackNoticeModel
import app.getvela.wallet.feature.settings.SettingsLive
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.feature.settings.SettingsFixtures
import app.getvela.wallet.feature.settings.SettingsScreenState
import java.io.File
import app.getvela.wallet.feature.settings.core.FeedbackReporter
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The founder (spec 078 round 3): 「反馈成功或失败都要有提示吧，而不是生硬的退出到
 * 设置页面吧」 — a report ALWAYS ends in something the person sees. In the open
 * sheet when it is open; as the settings page's notice when the person closed
 * the sheet mid-send, and the send carries on regardless.
 */
class FeedbackReporterTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
    private val answer = CompletableDeferred<BugReport.Outcome>()
    private var sent = 0
    private val reporter = FeedbackReporter(scope) { sent++; answer.await() }

    private val payload = BugReport.Payload("x", "", BugReport.AREA_OTHER, "env", "fp")
    private val filed = BugReport.Outcome.Filed(323, "https://github.com/mondaylabsltd/vela-wallet/issues/323", deduped = false)
    private val fallback = BugReport.Outcome.Fallback(BugReport.Reason.Unreachable, "https://github.com/mondaylabsltd/vela-wallet/issues/new?template=bug.yml")

    @After
    fun tearDown() = scope.cancel()

    @Test
    fun `an answer while the sheet is open is shown in the sheet, never as a notice`() {
        reporter.sheetOpened()
        assertTrue(reporter.submit(payload))
        assertTrue(reporter.state.value.sending)
        answer.complete(filed)
        val s = reporter.state.value
        assertFalse(s.sending)
        assertEquals(filed, s.outcome)
        assertNull(s.notice)
        assertTrue("the sheet was never asked to close", s.sheetOpen)
    }

    @Test
    fun `closing the sheet mid-send keeps the send going and ends in a notice, filed`() {
        reporter.sheetOpened()
        reporter.submit(payload)
        reporter.sheetClosed()
        assertTrue("still sending after the close", reporter.state.value.sending)
        answer.complete(filed)
        val s = reporter.state.value
        assertEquals(1, sent)
        assertEquals(filed, s.notice)
        assertNull("nothing left for a sheet opened later", s.outcome)
        reporter.noticeShown()
        assertNull(reporter.state.value.notice)
    }

    @Test
    fun `closing the sheet mid-send ends in a notice, not filed`() {
        reporter.sheetOpened()
        reporter.submit(payload)
        reporter.sheetClosed()
        answer.complete(fallback)
        assertEquals(fallback, reporter.state.value.notice)
    }

    @Test
    fun `reopened before the answer, the sheet shows it`() {
        reporter.sheetOpened()
        reporter.submit(payload)
        reporter.sheetClosed()
        reporter.sheetOpened()
        answer.complete(fallback)
        assertEquals(fallback, reporter.state.value.outcome)
        assertNull(reporter.state.value.notice)
    }

    @Test
    fun `a second send while one is in flight is refused`() {
        reporter.sheetOpened()
        assertTrue(reporter.submit(payload))
        assertFalse(reporter.submit(payload))
        answer.complete(filed)
        assertEquals(1, sent)
    }

    @Test
    fun `closing after the answer clears it, so the next open starts fresh`() {
        reporter.sheetOpened()
        reporter.submit(payload)
        answer.complete(filed)
        reporter.sheetClosed()
        assertNull(reporter.state.value.outcome)
        assertNull(reporter.state.value.notice)
    }

    @Test
    fun `the notice says the outcome and offers the way onward`() {
        val labels = FeedbackModel(
            title = "", subtitle = "", placeholder = "", addSteps = "", previewToggle = "", previewLines = emptyList(),
            consent = "", send = "", githubLink = "",
            successTitle = "感谢反馈，已收到", viewIssue = "在 GitHub 查看",
            fallbackTitle = "暂时无法在应用内发送", openGithub = "打开 GitHub 表单",
        )
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        val strings = I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
        val model = SettingsFixtures.buildState(SettingsScreenState.ST1, strings).copy(feedback = labels)
        assertEquals(
            FeedbackNoticeModel("感谢反馈，已收到", "在 GitHub 查看", filed.url),
            SettingsLive.withFeedbackNotice(model, filed).feedback.notice,
        )
        assertEquals(
            FeedbackNoticeModel("暂时无法在应用内发送", "打开 GitHub 表单", fallback.fallbackUrl),
            SettingsLive.withFeedbackNotice(model, fallback).feedback.notice,
        )
        assertNull(SettingsLive.withFeedbackNotice(model, null).feedback.notice)
    }
}
