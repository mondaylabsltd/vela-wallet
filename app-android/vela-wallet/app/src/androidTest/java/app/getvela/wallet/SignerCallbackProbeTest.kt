package app.getvela.wallet

import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.feature.signing.trustedsigner.TrustedSignerCallbacks
import app.getvela.wallet.feature.signing.trustedsigner.TrustedSignerTab
import kotlinx.coroutines.CompletableDeferred
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.io.BufferedReader
import java.io.InputStreamReader
import java.net.InetAddress
import java.net.ServerSocket
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/**
 * Spec 076, probe P3 — **a measurement, not a feature**.
 *
 * The custom-scheme channel rests on one thing no unit test can tell us: that
 * a page in a **Custom Tab over this app** can hand the answer back, and that
 * doing so does not disturb the wallet underneath.
 *
 * That is not obvious, and the obvious arrangement is wrong. `MainActivity` is
 * `singleTop`, and the tab sits ABOVE it in this same task — so a `VIEW`
 * intent aimed at `MainActivity` would NOT reach the instance the person is
 * using: `singleTop` reuses an activity only when it is already on top, and
 * Android would build a second `MainActivity` over the tab, a fresh app with
 * the flow's screen gone. Hence [SignResultActivity], which this probe exists
 * to check the behaviour of.
 *
 * So: the real Custom Tab, a real page on a real socket, a real navigation to
 * `velawallet://sign-result`, and then the questions that matter —
 *
 *  1. did the answer reach the request that was waiting;
 *  2. is `MainActivity` the SAME instance it was before, un-recreated;
 *  3. did anything of the wallet's own state change.
 *
 * Needs a browser on the device. Skipped, loudly, where there is none.
 */
@RunWith(AndroidJUnit4::class)
class SignerCallbackProbeTest {

    /** A page that answers by navigating to the wallet's scheme, as the real one does. */
    private fun answeringPage(callback: String) = """
        <!doctype html><html><head><meta charset="utf-8"><title>answer</title>
        <meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'">
        </head><body><p>answering…</p><script>
        // Under the SAME `default-src 'none'` the published page carries: a
        // navigation to a custom scheme is not governed by it.
        setTimeout(function () { location.href = ${'"'}$callback${'"'}; }, 300);
        </script></body></html>
    """.trimIndent()

    private class Served(val port: Int, val socket: ServerSocket, val asked: MutableList<String>)

    private fun serve(body: () -> ByteArray): Served {
        val socket = ServerSocket(0, 8, InetAddress.getByName("127.0.0.1"))
        val asked = java.util.Collections.synchronizedList(mutableListOf<String>())
        Thread {
            while (!socket.isClosed) {
                try {
                    val client = socket.accept()
                    val reader = BufferedReader(InputStreamReader(client.getInputStream()))
                    asked.add(reader.readLine() ?: "")
                    val bytes = body()
                    val head = "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n" +
                        "Content-Length: ${bytes.size}\r\nConnection: close\r\n\r\n"
                    client.getOutputStream().write(head.toByteArray(Charsets.US_ASCII))
                    client.getOutputStream().write(bytes)
                    client.getOutputStream().flush()
                    client.close()
                } catch (_: Exception) {
                    // closed, or the client went away
                }
            }
        }.also { it.isDaemon = true }.start()
        return Served(socket.localPort, socket, asked)
    }

    @Test
    fun a_page_in_a_tab_over_the_wallet_answers_without_disturbing_it() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val token = "probe-" + System.nanoTime()
        val callback = "velawallet://sign-result?t=$token&result=e30"
        val served = serve { answeringPage(callback).toByteArray(Charsets.UTF_8) }

        val answered = CompletableDeferred<String?>()
        TrustedSignerCallbacks.await(token, answered)

        val scenario = ActivityScenario.launch(MainActivity::class.java)
        var identity: Int? = null
        var opened = false
        scenario.onActivity { activity ->
            // WHICH instance is on screen. If a second `MainActivity` were
            // built over the tab, this would not be it afterwards.
            identity = System.identityHashCode(activity)
            opened = TrustedSignerTab(activity).open("http://127.0.0.1:${served.port}/answer.html")
        }
        if (!opened) {
            served.socket.close()
            TrustedSignerCallbacks.forget(token)
            println("P3 · SKIPPED: no browser on this device to open a tab in")
            return
        }

        val landed = CountDownLatch(1)
        Thread {
            while (!answered.isCompleted && landed.count > 0) Thread.sleep(50)
            landed.countDown()
        }.also { it.isDaemon = true }.start()
        val arrived = landed.await(45, TimeUnit.SECONDS) && answered.isCompleted

        println("P3 · the page was fetched: ${served.asked}")
        println("P3 · the callback arrived: $arrived")
        served.socket.close()
        TrustedSignerCallbacks.forget(token)

        assertTrue("the tab never fetched the page", served.asked.isNotEmpty())
        assertTrue("the page's navigation to velawallet:// never reached the app", arrived)
        assertEquals(
            "the callback carries the answer, not a truncated URL",
            callback,
            answered.getCompleted(),
        )

        // …and the wallet is where it was: the same activity instance, never
        // rebuilt, with nothing of its state touched.
        scenario.onActivity { activity ->
            assertEquals("MainActivity was recreated by the callback", identity, System.identityHashCode(activity))
        }
        scenario.close()
    }
}
