package app.getvela.wallet

import app.getvela.wallet.core.diagnostics.BugReportUrl
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The gotcha, pinned so it cannot come back (spec 081 FR-016).
 *
 * `?template=bug.yml&body=…` looks right, opens, and throws the body away:
 * GitHub prefills an issue FORM by its field ids and ignores `body` entirely.
 * Three call sites had this bug at once, which is why the URL is built in one
 * place now and why the assertion below is about `body` being ABSENT rather
 * than about the fields being present.
 */
class BugReportUrlTest {

    @Test
    fun `prefills by field id and never sends a body`() {
        val url = BugReportUrl.build(
            what = "Send froze",
            steps = "1. tap send",
            environment = "App version: v1.0.0",
        )
        assertTrue(url.startsWith("https://github.com/mondaylabsltd/vela-wallet/issues/new?"))
        assertTrue("template=bug.yml" in url)
        assertTrue("what=Send+froze" in url)
        assertTrue("steps=1.+tap+send" in url)
        assertTrue("environment=App+version%3A+v1.0.0" in url)
        // The whole point.
        assertFalse("body=" in url)
    }

    @Test
    fun `the area matches an option the form actually offers`() {
        // A value the dropdown does not have leaves it unset, which is the
        // same as not prefilling it — so this string is a contract with
        // `.github/ISSUE_TEMPLATE/bug.yml`, not a label.
        assertEquals("Other (explain above)", BugReportUrl.AREA_OTHER)
        assertTrue("area=Other+%28explain+above%29" in BugReportUrl.build(what = "x"))
    }

    @Test
    fun `an empty optional field is left out rather than sent blank`() {
        val url = BugReportUrl.build(what = "x")
        assertFalse("steps=" in url)
        assertFalse("environment=" in url)
    }

    @Test
    fun `the title carries the prefix and only the first line`() {
        val url = BugReportUrl.build(what = "line one\nline two", titlePrefix = "[android] ")
        assertTrue("title=%5Bandroid%5D+line+one&" in url)
        assertFalse("line+two&" in url.substringBefore("&what="))
    }
}
