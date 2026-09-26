package app.getvela.wallet

import app.getvela.wallet.core.diagnostics.BugReport
import app.getvela.wallet.core.diagnostics.BugReport.Answer
import app.getvela.wallet.core.diagnostics.BugReport.Outcome
import app.getvela.wallet.core.diagnostics.BugReport.Reason
import app.getvela.wallet.core.diagnostics.BugReportUrl
import java.util.Base64
import kotlinx.coroutines.runBlocking
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The in-app report (spec 078 round 3): the web's `bug-report.test.ts` and the
 * desktop's `bug_report.rs` vectors, on Android's copy — and every send path
 * through a STUB transport. Nothing here reaches getvela.app.
 */
class BugReportTest {

    private val labels = BugReport.EnvironmentLabels(
        version = "App version",
        platform = "Platform",
        language = "Language",
        rpc = "Unreachable RPC",
        failures = "Recent failures",
        none = "None",
    )

    private val facts = BugReport.DeviceFacts(
        version = "1.0.0",
        commit = "abc1234",
        platform = "Android 14",
        language = "en",
        unreachable = listOf("Gnosis"),
        failures = listOf("rpc:final_failure ×2"),
    )

    private fun payload(what: String = "Send froze", steps: String = "1. tap send", shots: List<ByteArray> = emptyList()) =
        BugReport.build(what, steps, BugReport.AREA_OTHER, labels, facts, shots)

    /** A transport that records what it was handed and answers [answer]. */
    private class Stub(private val answer: Answer) {
        var url: String? = null
        var body: String? = null
        var timeoutMs: Long? = null
        var calls = 0
        suspend fun post(url: String, body: String, timeoutMs: Long): Answer {
            calls++
            this.url = url
            this.body = body
            this.timeoutMs = timeoutMs
            return answer
        }
    }

    private fun send(payload: BugReport.Payload, answer: Answer): Pair<Outcome, Stub> {
        val stub = Stub(answer)
        val outcome = runBlocking { BugReport.send(payload, endpoint = "https://stub.invalid/api/bug-report", post = stub::post) }
        return outcome to stub
    }

    // --- What a report may carry ------------------------------------------

    @Test
    fun `redact replaces addresses and urls, the desktop's vectors`() {
        assertEquals("to [address] now", BugReport.redact("to 0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c now"))
        assertEquals("rpc [url] down", BugReport.redact("rpc https://eth.example/v2/KEY123 down"))
        // One hex digit too many is not an address the web would match.
        assertEquals("0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5cA", BugReport.redact("0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5cA"))
        assertEquals("My net [url]", BugReport.redact("My net wss://node:8546/x"))
        assertEquals("nothing here", BugReport.redact("nothing here"))
        assertEquals("Recent failures: rpc:final_failure ×2", BugReport.redact("Recent failures: rpc:final_failure ×2"))
    }

    @Test
    fun `the preview is the payload's environment, line for line`() {
        val lines = BugReport.environmentLines(labels, facts)
        assertEquals(
            listOf(
                "App version: v1.0.0 (abc1234)",
                "Platform: Android 14",
                "Language: en",
                "Unreachable RPC: Gnosis",
                "Recent failures: rpc:final_failure ×2",
            ),
            lines,
        )
        assertEquals(lines.joinToString("\n"), payload().environment)
        val empty = BugReport.environmentLines(labels, facts.copy(unreachable = emptyList(), failures = emptyList()))
        assertEquals("Unreachable RPC: None", empty[3])
        assertEquals("Recent failures: None", empty[4])
    }

    @Test
    fun `a network named after its own rpc url never reaches the wire`() {
        val leaky = facts.copy(unreachable = listOf("https://eth-mainnet.g.alchemy.com/v2/SUPER-SECRET-KEY", "0x44EEC06897ff7ab8C7f16819511A64bA168A6D33"))
        val wire = BugReport.body(BugReport.build("x", "", BugReport.AREA_OTHER, labels, leaky))
        assertFalse(wire.contains("SUPER-SECRET-KEY"))
        assertFalse(wire.contains("0x44EEC06897ff7ab8C7f16819511A64bA168A6D33"))
        assertTrue(wire.contains("[url]"))
        assertTrue(wire.contains("[address]"))
    }

    @Test
    fun `the fingerprint hashes as the web's does`() {
        assertEquals("c7e1a5b2", BugReport.fingerprintOf("  Send FAILED on Base ", BugReport.AREA_OTHER, "1.0.0"))
        assertEquals("f7bb6d3d", BugReport.fingerprintOf("转账失败", BugReport.AREA_OTHER, "1.0.0"))
        val a = BugReport.fingerprintOf("Send froze", BugReport.AREA_OTHER, "1.0.0")
        assertEquals(a, BugReport.fingerprintOf("  SEND FROZE ", BugReport.AREA_OTHER, "1.0.0"))
        assertFalse(a == BugReport.fingerprintOf("Receive froze", BugReport.AREA_OTHER, "1.0.0"))
    }

    @Test
    fun `a text-only body is the five fields and no sixth`() {
        val json = JSONObject(BugReport.body(payload()))
        assertEquals(setOf("what", "steps", "area", "environment", "fingerprint"), json.keys().asSequence().toSet())
        assertEquals("Send froze", json.getString("what"))
        assertEquals("1. tap send", json.getString("steps"))
        assertEquals(BugReport.AREA_OTHER, json.getString("area"))
        // In the web's key order.
        assertTrue(BugReport.body(payload()).startsWith("{\"what\":\"Send froze\",\"steps\":\"1. tap send\",\"area\":\"Other (explain above)\",\"environment\":"))
        assertNull("no screenshots → no field, not []", payload(shots = emptyList()).screenshots)
    }

    @Test
    fun `the fallback url prefills by field id, the desktop's vector`() {
        val url = BugReportUrl.prefilled(BugReport.build("It broke & stayed broken", "1. open\n2. send", BugReport.AREA_OTHER, labels, facts))
        assertTrue(url, url.startsWith("https://github.com/mondaylabsltd/vela-wallet/issues/new?template=bug.yml&title=%5Bbug%5D+It+broke+%26+stayed+broken&what="))
        assertTrue(url.contains("&steps=1.+open%0A2.+send&"))
        assertTrue(url.contains("&area=Other+%28explain+above%29"))
        assertFalse(url.contains("body="))
        // Steps always present — the form marks it required.
        assertTrue(BugReportUrl.prefilled(payload(steps = "")).contains("&steps=&"))
    }

    // --- Screenshots on the wire ------------------------------------------

    @Test
    fun `screenshots go as plain base64, in tile order, at most five, never into the form`() {
        val shots = (1..6).map { byteArrayOf(0xFF.toByte(), 0xD8.toByte(), 0xFF.toByte(), it.toByte(), 0xFF.toByte(), 0xD9.toByte()) }
        val p = payload(shots = shots)
        val sent = p.screenshots!!
        assertEquals(BugReport.MAX_SCREENSHOTS, sent.size)
        sent.forEachIndexed { i, b64 ->
            assertFalse("no data: prefix", b64.startsWith("data:"))
            assertFalse("no line breaks", b64.contains('\n'))
            assertTrue(shots[i].contentEquals(Base64.getDecoder().decode(b64)))
        }
        val json = JSONObject(BugReport.body(p))
        assertEquals(5, json.getJSONArray("screenshots").length())
        assertEquals(sent[0], json.getJSONArray("screenshots").getString(0))
        assertFalse(BugReportUrl.prefilled(p).contains(sent[0].take(8)))
    }

    @Test
    fun `screenshots get the 30 s budget, text alone keeps 10 s`() {
        val (_, text) = send(payload(), Answer.Status(200, """{"number":1,"url":"https://x/1"}"""))
        assertEquals(BugReport.TIMEOUT_MS, text.timeoutMs)
        val (_, shots) = send(payload(shots = listOf(byteArrayOf(-1, -40, -1, -39))), Answer.Status(200, """{"number":1,"url":"https://x/1"}"""))
        assertEquals(BugReport.TIMEOUT_WITH_SCREENSHOTS_MS, shots.timeoutMs)
        assertEquals(30_000L, shots.timeoutMs)
    }

    @Test
    fun `screenshots do not count against the text cap`() {
        val big = ByteArray(150_000) { 0x41 }
        val (outcome, stub) = send(payload(shots = listOf(big)), Answer.Status(200, """{"number":7,"url":"https://github.com/x/y/issues/7"}"""))
        assertTrue(outcome is Outcome.Filed)
        assertEquals(1, stub.calls)
        assertEquals(Base64.getEncoder().encodeToString(big), JSONObject(stub.body!!).getJSONArray("screenshots").getString(0))
    }

    // --- Outcomes, through the stub ---------------------------------------

    @Test
    fun `filed, new and deduped, with the dropped count`() {
        val (filed, stub) = send(payload(), Answer.Status(200, """{"number":42,"url":"https://github.com/x/y/issues/42","deduped":false}"""))
        assertEquals(Outcome.Filed(42, "https://github.com/x/y/issues/42", deduped = false, screenshotsDropped = 0), filed)
        assertEquals("https://stub.invalid/api/bug-report", stub.url)
        assertEquals(BugReport.body(payload()), stub.body)

        val (deduped, _) = send(payload(), Answer.Status(200, """{"number":7,"url":"https://github.com/x/y/issues/7","deduped":true}"""))
        assertEquals(Outcome.Filed(7, "https://github.com/x/y/issues/7", deduped = true), deduped)

        val (dropped, _) = send(
            payload(shots = listOf(byteArrayOf(-1, -40, -1, -39), byteArrayOf(-1, -40, -1, -39))),
            Answer.Status(200, """{"number":9,"url":"https://github.com/x/y/issues/9","deduped":false,"screenshots":1,"screenshotsDropped":1}"""),
        )
        assertEquals(1, (dropped as Outcome.Filed).screenshotsDropped)
    }

    @Test
    fun `every refusal falls back to the prefilled form`() {
        val cases = listOf(
            503 to Reason.NotConfigured,
            429 to Reason.RateLimited,
            413 to Reason.TooLarge,
            415 to Reason.Rejected,
            400 to Reason.Rejected,
            500 to Reason.Rejected,
            502 to Reason.Rejected,
        )
        for ((code, reason) in cases) {
            val (outcome, _) = send(payload(what = "x", shots = listOf(byteArrayOf(-1, -40, -1, -39))), Answer.Status(code, "{}"))
            outcome as Outcome.Fallback
            assertEquals("status $code", reason, outcome.reason)
            assertTrue(outcome.fallbackUrl.contains("what=x"))
        }
    }

    @Test
    fun `a 200 this client cannot read is not a filed report`() {
        for (body in listOf("not json", "{}", """{"number":"42","url":"https://x"}""", """{"number":42}""")) {
            val (outcome, _) = send(payload(), Answer.Status(200, body))
            assertEquals(body, Reason.Rejected, (outcome as Outcome.Fallback).reason)
        }
    }

    @Test
    fun `offline and timed out fall back as unreachable`() {
        for (answer in listOf(Answer.Failed(timedOut = false), Answer.Failed(timedOut = true))) {
            val (outcome, _) = send(payload(), answer)
            outcome as Outcome.Fallback
            assertEquals(Reason.Unreachable, outcome.reason)
            assertEquals(BugReportUrl.prefilled(payload()), outcome.fallbackUrl)
        }
    }

    @Test
    fun `text over the cap never leaves the device`() {
        val (outcome, stub) = send(payload(what = "x".repeat(BugReport.MAX_REPORT_CHARS)), Answer.Status(200, "{}"))
        assertEquals(Reason.TooLarge, (outcome as Outcome.Fallback).reason)
        assertEquals("refused before the request", 0, stub.calls)
    }

    @Test
    fun `an image over the endpoint's cap never leaves the device`() {
        val (outcome, stub) = send(payload(shots = listOf(ByteArray(BugReport.MAX_SCREENSHOT_BYTES + 3))), Answer.Status(200, "{}"))
        assertEquals(Reason.TooLarge, (outcome as Outcome.Fallback).reason)
        assertEquals(0, stub.calls)
    }
}
